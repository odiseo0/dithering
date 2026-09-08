use super::{CancellationCheck, EffectConfig, EffectKind, ProgressSink, algorithms};
use crate::components::raster::{Raster, RasterError};
use thiserror::Error;

/// # Errors
///
/// Returns [`RenderError::Cancelled`] when cancellation is requested, or an effect-specific
/// error when rendering cannot finish.
pub fn render(
    source: &Raster,
    config: &EffectConfig,
    cancellation: &impl CancellationCheck,
    progress: &impl ProgressSink,
) -> Result<Raster, RenderError> {
    if cancellation.is_cancelled() {
        return Err(RenderError::Cancelled);
    }

    progress.report(0.0);

    match config {
        EffectConfig::ErrorDiffusion(config) => {
            algorithms::render_error_diffusion(source, config, cancellation, progress)
        }
        EffectConfig::Ordered(config) => {
            algorithms::render_ordered(source, config, cancellation, progress)
        }
        EffectConfig::Halftone(_) => unavailable(EffectKind::Halftone),
    }
}

fn unavailable(effect: EffectKind) -> Result<Raster, RenderError> {
    Err(RenderError::EffectNotImplemented { effect })
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RenderError {
    #[error("rendering was cancelled")]
    Cancelled,
    #[error("the selected effect is not implemented")]
    EffectNotImplemented { effect: EffectKind },
    #[error("the renderer produced an invalid image")]
    InvalidOutput(#[source] RasterError),
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::{RenderError, render};
    use crate::components::{
        dithering::{EffectConfig, EffectKind, HalftoneConfig, NeverCancel, ProgressSink},
        raster::Raster,
    };

    #[derive(Default)]
    struct RecordedProgress(RefCell<Vec<f32>>);

    impl ProgressSink for RecordedProgress {
        fn report(&self, progress: f32) {
            self.0.borrow_mut().push(progress);
        }
    }

    #[test]
    fn dispatches_the_unimplemented_effect_to_its_branch() {
        let source = Raster::new(1, 1, vec![0, 0, 0, 255]).expect("valid fixture");
        let config = EffectConfig::Halftone(HalftoneConfig::default());
        let expected = RenderError::EffectNotImplemented {
            effect: EffectKind::Halftone,
        };

        assert_eq!(
            render(&source, &config, &NeverCancel, &RecordedProgress::default()),
            Err(expected)
        );
    }

    #[test]
    fn cancellation_stops_before_progress() {
        use crate::components::dithering::AtomicCancellationToken;

        let source = Raster::new(1, 1, vec![0, 0, 0, 255]).expect("valid fixture");
        let cancellation = AtomicCancellationToken::new();
        let progress = RecordedProgress::default();
        cancellation.cancel();

        assert_eq!(
            render(&source, &EffectConfig::default(), &cancellation, &progress),
            Err(RenderError::Cancelled)
        );
        assert!(progress.0.borrow().is_empty());
    }

    #[test]
    fn reports_zero_when_work_starts() {
        let source = Raster::new(1, 1, vec![0, 0, 0, 255]).expect("valid fixture");
        let progress = RecordedProgress::default();

        let _result = render(&source, &EffectConfig::default(), &NeverCancel, &progress);

        assert_eq!(*progress.0.borrow(), vec![0.0, 1.0]);
    }
}
