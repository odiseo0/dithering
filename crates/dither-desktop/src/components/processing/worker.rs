use std::{
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{Sender, SyncSender},
    },
    time::{Duration, Instant},
};

use dither_engine::{CancellationCheck, EffectConfig, ProgressSink, Raster, RenderError};

use super::events::{ProcessingEvent, RenderRequest, RequestIdentity, WorkerEvent};

const PROGRESS_INTERVAL: Duration = Duration::from_millis(33);

pub trait RenderBackend: Send + Sync + 'static {
    /// Renders one request and observes its cancellation and progress control.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if rendering is cancelled or fails.
    fn render(
        &self,
        source: &Raster,
        config: &EffectConfig,
        control: &RenderControl,
    ) -> Result<Raster, RenderError>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct EngineRenderer;

impl RenderBackend for EngineRenderer {
    fn render(
        &self,
        source: &Raster,
        config: &EffectConfig,
        control: &RenderControl,
    ) -> Result<Raster, RenderError> {
        dither_engine::render(source, config, control, control)
    }
}

pub struct RenderControl {
    generation: u64,
    desired_generation: Arc<AtomicU64>,
    shutting_down: Arc<AtomicBool>,
    identity: RequestIdentity,
    events: SyncSender<WorkerEvent>,
    last_progress: Mutex<Option<(Instant, f32)>>,
}

impl RenderControl {
    pub(crate) fn new(
        generation: u64,
        desired_generation: Arc<AtomicU64>,
        shutting_down: Arc<AtomicBool>,
        identity: RequestIdentity,
        events: SyncSender<WorkerEvent>,
    ) -> Self {
        Self {
            generation,
            desired_generation,
            shutting_down,
            identity,
            events,
            last_progress: Mutex::new(None),
        }
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.shutting_down.load(Ordering::Acquire)
            || self.desired_generation.load(Ordering::Acquire) != self.generation
    }

    pub fn report_progress(&self, value: f32) {
        if self.is_cancelled() {
            return;
        }

        let value = value.clamp(0.0, 1.0);
        let now = Instant::now();
        let Ok(mut last) = self.last_progress.lock() else {
            return;
        };
        let should_send = match *last {
            None => true,
            Some((time, previous)) => {
                value >= 1.0 || (value >= previous && now.duration_since(time) >= PROGRESS_INTERVAL)
            }
        };

        if should_send {
            *last = Some((now, value));
            let _ignored = self.events.try_send(WorkerEvent {
                generation: self.generation,
                event: ProcessingEvent::Progress {
                    document_id: self.identity.document_id,
                    revision: self.identity.revision,
                    value,
                },
            });
        }
    }
}

impl CancellationCheck for RenderControl {
    fn is_cancelled(&self) -> bool {
        Self::is_cancelled(self)
    }
}

impl ProgressSink for RenderControl {
    fn report(&self, progress: f32) {
        self.report_progress(progress);
    }
}

pub(crate) struct PendingRequest {
    pub generation: u64,
    pub request: RenderRequest,
    pub ready_at: Instant,
}

#[derive(Default)]
pub(crate) struct WorkerState {
    pub pending: Option<PendingRequest>,
    pub shutdown: bool,
}

pub(crate) struct SharedWorker {
    pub state: Mutex<WorkerState>,
    pub wake: Condvar,
}

pub(crate) fn run_worker(
    shared: &SharedWorker,
    renderer: &dyn RenderBackend,
    desired_generation: &Arc<AtomicU64>,
    shutting_down: &Arc<AtomicBool>,
    events: &SyncSender<WorkerEvent>,
    stopped: &Sender<()>,
) {
    while let Some(pending) = take_next(shared) {
        let identity = pending.request.identity();
        let control = RenderControl::new(
            pending.generation,
            Arc::clone(desired_generation),
            Arc::clone(shutting_down),
            identity,
            events.clone(),
        );
        let result = renderer.render(&pending.request.source, &pending.request.config, &control);

        if control.is_cancelled() {
            continue;
        }

        let event = match result {
            Ok(raster) => ProcessingEvent::Completed {
                document_id: identity.document_id,
                revision: identity.revision,
                raster,
            },
            Err(RenderError::Cancelled) => continue,
            Err(error) => ProcessingEvent::Failed {
                document_id: identity.document_id,
                revision: identity.revision,
                error,
            },
        };
        let _ignored = events.send(WorkerEvent {
            generation: pending.generation,
            event,
        });
    }

    let _ignored = stopped.send(());
}

fn take_next(shared: &SharedWorker) -> Option<PendingRequest> {
    let mut state = shared
        .state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    loop {
        if state.shutdown {
            return None;
        }

        if let Some(pending) = state.pending.as_ref() {
            let now = Instant::now();

            if now >= pending.ready_at {
                return state.pending.take();
            }

            let wait = pending.ready_at.duration_since(now);
            let waited = shared
                .wake
                .wait_timeout(state, wait)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state = waited.0;
        } else {
            state = shared
                .wake
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }
}
