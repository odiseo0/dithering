use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub trait CancellationCheck {
    fn is_cancelled(&self) -> bool;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NeverCancel;

impl CancellationCheck for NeverCancel {
    fn is_cancelled(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Default)]
pub struct AtomicCancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl AtomicCancellationToken {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

impl CancellationCheck for AtomicCancellationToken {
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::{AtomicCancellationToken, CancellationCheck};

    #[test]
    fn cloned_tokens_share_the_cancelled_state() {
        let first = AtomicCancellationToken::new();
        let second = first.clone();

        assert!(!second.is_cancelled());
        first.cancel();
        assert!(second.is_cancelled());
    }
}
