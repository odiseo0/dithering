pub trait ProgressSink {
    fn report(&self, progress: f32);
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NoProgress;

impl ProgressSink for NoProgress {
    fn report(&self, _progress: f32) {}
}
