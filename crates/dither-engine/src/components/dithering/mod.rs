//! Dithering configuration, execution contracts, and algorithms.

mod algorithms;
mod cancellation;
mod config;
mod palette;
mod progress;
mod render;

pub use cancellation::{AtomicCancellationToken, CancellationCheck, NeverCancel};
pub use config::{
    BayerMatrix, ConfigError, DotShape, EffectConfig, EffectKind, ErrorDiffusionAlgorithm,
    ErrorDiffusionConfig, HalftoneConfig, OrderedConfig,
};
pub use palette::{MAX_PALETTE_COLORS, MIN_PALETTE_COLORS, Palette, PaletteError};
pub use progress::{NoProgress, ProgressSink};
pub use render::{RenderError, render};
