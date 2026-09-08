//! Image model and color and scale rules.

mod color;
mod model;
pub(crate) mod scale;

pub use color::{LinearRgb, Rgb8};
pub use model::{MAX_PIXELS, MAX_SIDE, Raster, RasterError};
pub use scale::{MAX_SCALE, MIN_SCALE, Scale, ScaleError};
