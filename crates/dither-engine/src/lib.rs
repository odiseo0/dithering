#![forbid(unsafe_code)]

pub mod components;

pub use components::{
    dithering::{
        AtomicCancellationToken, BayerMatrix, CancellationCheck, ConfigError, DotShape,
        EffectConfig, EffectKind, ErrorDiffusionAlgorithm, ErrorDiffusionConfig, HalftoneConfig,
        NeverCancel, NoProgress, OrderedConfig, Palette, PaletteError, ProgressSink, RenderError,
        render,
    },
    raster::{LinearRgb, Raster, RasterError, Rgb8, Scale, ScaleError},
};

pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
