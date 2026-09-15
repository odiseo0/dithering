use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use dither_desktop::components::processing::{
    Dispatch, DocumentId, ProcessingCoordinator, ProcessingEvent, RenderBackend, RenderControl,
    RenderRequest, Revision,
};
use dither_engine::{EffectConfig, Raster, RenderError};

fn raster(marker: u8) -> Arc<Raster> {
    Arc::new(Raster::new(1, 1, vec![marker, 0, 0, 255]).expect("valid test raster"))
}

fn request(document: u64, revision: u64, marker: u8) -> RenderRequest {
    RenderRequest::new(
        DocumentId::new(document),
        Revision::new(revision),
        raster(marker),
        EffectConfig::default(),
    )
}

fn wait_for_completion(
    coordinator: &mut ProcessingCoordinator,
    timeout: Duration,
) -> ProcessingEvent {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(event @ ProcessingEvent::Completed { .. }) = coordinator.poll_event() {
            return event;
        }
        assert!(Instant::now() < deadline, "render did not complete in time");
        thread::sleep(Duration::from_millis(2));
    }
}

#[derive(Clone)]
struct RecordingRenderer {
    rendered: Arc<Mutex<Vec<u8>>>,
}

impl RenderBackend for RecordingRenderer {
    fn render(
        &self,
        source: &Raster,
        _config: &EffectConfig,
        control: &RenderControl,
    ) -> Result<Raster, RenderError> {
        self.rendered
            .lock()
            .expect("test recorder lock")
            .push(source.rgba()[0]);
        control.report_progress(0.0);
        control.report_progress(1.0);
        Ok(source.clone())
    }
}

#[test]
fn three_debounced_changes_keep_only_the_latest_request() {
    let rendered = Arc::new(Mutex::new(Vec::new()));
    let backend = RecordingRenderer {
        rendered: Arc::clone(&rendered),
    };
    let mut coordinator = ProcessingCoordinator::with_renderer(backend).expect("worker starts");

    coordinator.submit(request(1, 1, 1), Dispatch::Debounced);
    coordinator.submit(request(1, 2, 2), Dispatch::Debounced);
    coordinator.submit(request(1, 3, 3), Dispatch::Debounced);
    let event = wait_for_completion(&mut coordinator, Duration::from_secs(2));

    assert!(matches!(
        event,
        ProcessingEvent::Completed {
            document_id,
            revision,
            ..
        } if document_id == DocumentId::new(1) && revision == Revision::new(3)
    ));
    assert_eq!(*rendered.lock().expect("test recorder lock"), vec![3]);
}

struct SlowRenderer {
    cancellations: Arc<AtomicUsize>,
}

impl RenderBackend for SlowRenderer {
    fn render(
        &self,
        source: &Raster,
        _config: &EffectConfig,
        control: &RenderControl,
    ) -> Result<Raster, RenderError> {
        for row in 0_u16..100 {
            if control.is_cancelled() {
                self.cancellations.fetch_add(1, Ordering::Relaxed);
                return Err(RenderError::Cancelled);
            }
            control.report_progress(f32::from(row) / 100.0);
            thread::sleep(Duration::from_millis(1));
        }
        control.report_progress(1.0);
        Ok(source.clone())
    }
}

#[test]
fn a_new_revision_cancels_slow_work_and_only_returns_the_new_result() {
    let cancellations = Arc::new(AtomicUsize::new(0));
    let mut coordinator = ProcessingCoordinator::with_renderer(SlowRenderer {
        cancellations: Arc::clone(&cancellations),
    })
    .expect("worker starts");

    coordinator.submit(request(7, 1, 1), Dispatch::Immediate);
    thread::sleep(Duration::from_millis(10));
    coordinator.submit(request(7, 2, 2), Dispatch::Immediate);
    let event = wait_for_completion(&mut coordinator, Duration::from_secs(2));

    let ProcessingEvent::Completed {
        document_id,
        revision,
        raster,
    } = event
    else {
        unreachable!();
    };
    assert_eq!(document_id, DocumentId::new(7));
    assert_eq!(revision, Revision::new(2));
    assert_eq!(raster.rgba()[0], 2);
    assert_eq!(cancellations.load(Ordering::Relaxed), 1);
    assert!(coordinator.poll_event().is_none());
}

struct IgnoresCancellation;

impl RenderBackend for IgnoresCancellation {
    fn render(
        &self,
        source: &Raster,
        _config: &EffectConfig,
        _control: &RenderControl,
    ) -> Result<Raster, RenderError> {
        if source.rgba()[0] == 1 {
            thread::sleep(Duration::from_millis(40));
        }
        Ok(source.clone())
    }
}

#[test]
fn stale_results_are_filtered_even_if_a_backend_ignores_cancellation() {
    let mut coordinator =
        ProcessingCoordinator::with_renderer(IgnoresCancellation).expect("worker starts");
    coordinator.submit(request(1, 1, 1), Dispatch::Immediate);
    thread::sleep(Duration::from_millis(5));
    coordinator.submit(request(2, 1, 2), Dispatch::Immediate);

    let event = wait_for_completion(&mut coordinator, Duration::from_secs(2));
    assert!(matches!(
        event,
        ProcessingEvent::Completed {
            document_id,
            raster,
            ..
        } if document_id == DocumentId::new(2) && raster.rgba()[0] == 2
    ));
}

#[test]
fn internal_generation_filters_a_repeated_public_identity() {
    let mut coordinator =
        ProcessingCoordinator::with_renderer(IgnoresCancellation).expect("worker starts");
    coordinator.submit(request(1, 1, 1), Dispatch::Immediate);
    thread::sleep(Duration::from_millis(5));
    coordinator.submit(request(1, 1, 2), Dispatch::Immediate);

    let event = wait_for_completion(&mut coordinator, Duration::from_secs(2));
    assert!(matches!(
        event,
        ProcessingEvent::Completed { raster, .. } if raster.rgba()[0] == 2
    ));
}

#[test]
fn progress_is_monotonic_bounded_and_has_request_identity() {
    let rendered = Arc::new(Mutex::new(Vec::new()));
    let mut coordinator = ProcessingCoordinator::with_renderer(RecordingRenderer { rendered })
        .expect("worker starts");
    coordinator.submit(request(4, 9, 5), Dispatch::Immediate);

    let deadline = Instant::now() + Duration::from_secs(1);
    let mut progress = Vec::new();
    loop {
        match coordinator.poll_event() {
            Some(ProcessingEvent::Progress {
                document_id,
                revision,
                value,
            }) => {
                assert_eq!(document_id, DocumentId::new(4));
                assert_eq!(revision, Revision::new(9));
                progress.push(value);
            }
            Some(ProcessingEvent::Completed { .. }) => break,
            Some(ProcessingEvent::Failed { error, .. }) => panic!("unexpected failure: {error}"),
            None => thread::sleep(Duration::from_millis(1)),
        }
        assert!(Instant::now() < deadline, "render did not complete in time");
    }
    assert_eq!(progress, vec![0.0, 1.0]);
}

#[test]
fn cancellation_clears_pending_work() {
    let rendered = Arc::new(Mutex::new(Vec::new()));
    let mut coordinator = ProcessingCoordinator::with_renderer(RecordingRenderer {
        rendered: Arc::clone(&rendered),
    })
    .expect("worker starts");
    coordinator.submit(request(1, 1, 1), Dispatch::Debounced);
    coordinator.cancel_current();
    thread::sleep(Duration::from_millis(130));

    assert!(!coordinator.has_current_request());
    assert!(coordinator.poll_event().is_none());
    assert!(rendered.lock().expect("test recorder lock").is_empty());
}

#[test]
fn shutdown_cancels_the_worker_without_an_unbounded_wait() {
    let cancellations = Arc::new(AtomicUsize::new(0));
    let mut coordinator = ProcessingCoordinator::with_renderer(SlowRenderer {
        cancellations: Arc::clone(&cancellations),
    })
    .expect("worker starts");
    coordinator.submit(request(1, 1, 1), Dispatch::Immediate);
    thread::sleep(Duration::from_millis(10));

    let started = Instant::now();
    assert!(coordinator.shutdown(Duration::from_secs(1)));
    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(cancellations.load(Ordering::Relaxed), 1);
}

#[test]
fn revision_increments_without_wrapping() {
    assert_eq!(Revision::INITIAL.next(), Some(Revision::new(1)));
    assert_eq!(Revision::new(u64::MAX).next(), None);
}
