use std::{
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use super::{
    events::{ProcessingEvent, RenderRequest, RequestIdentity, WorkerEvent},
    worker::{
        EngineRenderer, PendingRequest, RenderBackend, SharedWorker, WorkerState, run_worker,
    },
};

const DEBOUNCE_DELAY: Duration = Duration::from_millis(100);
const DROP_SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(250);
const EVENT_CHANNEL_CAPACITY: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Dispatch {
    Debounced,
    Immediate,
}

pub struct ProcessingCoordinator {
    shared: Arc<SharedWorker>,
    desired_generation: Arc<AtomicU64>,
    shutting_down: Arc<AtomicBool>,
    desired_identity: Option<RequestIdentity>,
    events: Receiver<WorkerEvent>,
    stopped: Receiver<()>,
    worker: Option<JoinHandle<()>>,
}

impl ProcessingCoordinator {
    /// Starts the fixed render worker.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if the operating system cannot create the worker thread.
    pub fn new() -> std::io::Result<Self> {
        Self::with_renderer(EngineRenderer)
    }

    /// Starts the fixed worker with a custom rendering backend.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if the operating system cannot create the worker thread.
    pub fn with_renderer(renderer: impl RenderBackend) -> std::io::Result<Self> {
        let shared = Arc::new(SharedWorker {
            state: Mutex::new(WorkerState::default()),
            wake: Condvar::new(),
        });
        let desired_generation = Arc::new(AtomicU64::new(0));
        let shutting_down = Arc::new(AtomicBool::new(false));
        let (event_sender, events) = mpsc::sync_channel(EVENT_CHANNEL_CAPACITY);
        let (stopped_sender, stopped) = mpsc::channel();
        let worker_shared = Arc::clone(&shared);
        let worker_generation = Arc::clone(&desired_generation);
        let worker_shutdown = Arc::clone(&shutting_down);
        let renderer: Arc<dyn RenderBackend> = Arc::new(renderer);
        let worker = thread::Builder::new()
            .name("dither-render-worker".to_owned())
            .spawn(move || {
                run_worker(
                    &worker_shared,
                    renderer.as_ref(),
                    &worker_generation,
                    &worker_shutdown,
                    &event_sender,
                    &stopped_sender,
                );
            })?;

        Ok(Self {
            shared,
            desired_generation,
            shutting_down,
            desired_identity: None,
            events,
            stopped,
            worker: Some(worker),
        })
    }

    pub fn submit(&mut self, request: RenderRequest, dispatch: Dispatch) {
        let generation = self
            .desired_generation
            .fetch_add(1, Ordering::AcqRel)
            .saturating_add(1);
        self.desired_identity = Some(request.identity());
        let ready_at = match dispatch {
            Dispatch::Debounced => Instant::now() + DEBOUNCE_DELAY,
            Dispatch::Immediate => Instant::now(),
        };
        let mut state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.pending = Some(PendingRequest {
            generation,
            request,
            ready_at,
        });
        drop(state);
        self.shared.wake.notify_one();
    }

    pub fn cancel_current(&mut self) {
        self.desired_generation.fetch_add(1, Ordering::AcqRel);
        self.desired_identity = None;
        let mut state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.pending = None;
        drop(state);
        self.shared.wake.notify_one();
    }

    pub fn poll_event(&mut self) -> Option<ProcessingEvent> {
        loop {
            match self.events.try_recv() {
                Ok(worker_event)
                    if worker_event.generation
                        == self.desired_generation.load(Ordering::Acquire)
                        && Some(worker_event.event.identity()) == self.desired_identity =>
                {
                    return Some(worker_event.event);
                }
                Ok(_) => {}
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => return None,
            }
        }
    }

    #[must_use]
    pub const fn has_current_request(&self) -> bool {
        self.desired_identity.is_some()
    }

    pub fn shutdown(&mut self, timeout: Duration) -> bool {
        if self.worker.is_none() {
            return true;
        }

        self.shutting_down.store(true, Ordering::Release);
        self.desired_generation.fetch_add(1, Ordering::AcqRel);
        self.desired_identity = None;
        let mut state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.shutdown = true;
        state.pending = None;
        drop(state);
        self.shared.wake.notify_one();

        if self.stopped.recv_timeout(timeout).is_err() {
            self.worker.take();

            return false;
        }
        if let Some(worker) = self.worker.take() {
            return worker.join().is_ok();
        }
        true
    }
}

impl Drop for ProcessingCoordinator {
    fn drop(&mut self) {
        let _stopped = self.shutdown(DROP_SHUTDOWN_TIMEOUT);
    }
}
