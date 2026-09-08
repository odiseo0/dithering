use super::{LinearRgb, Raster, Rgb8};

pub const MIN_SCALE: u8 = 1;
pub const MAX_SCALE: u8 = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Scale(u8);

impl Scale {
    pub const MIN: Self = Self(MIN_SCALE);

    /// # Errors
    ///
    /// Returns [`ScaleError`] if `value` is outside the supported range.
    pub fn new(value: u8) -> Result<Self, ScaleError> {
        if (MIN_SCALE..=MAX_SCALE).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ScaleError { value })
        }
    }

    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("scale must be from {MIN_SCALE} through {MAX_SCALE}, got {value}")]
pub struct ScaleError {
    value: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LogicalSample {
    pub color: LinearRgb,
    pub visible: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LogicalRaster {
    pub width: u32,
    pub height: u32,
    pub scale: Scale,
    pub samples: Vec<LogicalSample>,
}

pub(crate) fn reduce(source: &Raster, scale: Scale) -> LogicalRaster {
    let scale_u32 = u32::from(scale.get());
    let width = source.width().div_ceil(scale_u32);
    let height = source.height().div_ceil(scale_u32);
    let mut samples = Vec::new();

    for logical_y in 0..height {
        for logical_x in 0..width {
            samples.push(average_block(source, scale_u32, logical_x, logical_y));
        }
    }

    LogicalRaster {
        width,
        height,
        scale,
        samples,
    }
}

pub(crate) fn expand(
    source: &Raster,
    logical: &LogicalRaster,
    colors: &[Rgb8],
) -> Result<Raster, super::RasterError> {
    debug_assert_eq!(colors.len(), logical.samples.len());
    let scale = u32::from(logical.scale.get());
    let mut rgba = Vec::with_capacity(source.rgba().len());

    for y in 0..source.height() {
        for x in 0..source.width() {
            let logical_x = x / scale;
            let logical_y = y / scale;
            let logical_index = sample_index(logical.width, logical_x, logical_y);
            let source_offset = pixel_offset(source.width(), x, y);
            let color = colors[logical_index];
            rgba.extend_from_slice(&[color.r, color.g, color.b, source.rgba()[source_offset + 3]]);
        }
    }

    Raster::new(source.width(), source.height(), rgba)
}

fn average_block(source: &Raster, scale: u32, logical_x: u32, logical_y: u32) -> LogicalSample {
    let start_x = logical_x * scale;
    let start_y = logical_y * scale;
    let end_x = (start_x + scale).min(source.width());
    let end_y = (start_y + scale).min(source.height());
    let mut red = 0.0;
    let mut green = 0.0;
    let mut blue = 0.0;
    let mut alpha_sum = 0.0;

    for y in start_y..end_y {
        for x in start_x..end_x {
            let offset = pixel_offset(source.width(), x, y);
            let rgba = &source.rgba()[offset..offset + 4];

            if rgba[3] == 0 {
                continue;
            }

            let alpha = f32::from(rgba[3]) / 255.0;
            let color = Rgb8::new(rgba[0], rgba[1], rgba[2]).to_linear();
            red += color.r * alpha;
            green += color.g * alpha;
            blue += color.b * alpha;
            alpha_sum += alpha;
        }
    }

    if alpha_sum == 0.0 {
        LogicalSample {
            color: LinearRgb::new(0.0, 0.0, 0.0),
            visible: false,
        }
    } else {
        LogicalSample {
            color: LinearRgb::new(red / alpha_sum, green / alpha_sum, blue / alpha_sum),
            visible: true,
        }
    }
}

#[allow(clippy::cast_possible_truncation)]
fn pixel_offset(width: u32, x: u32, y: u32) -> usize {
    ((u64::from(y) * u64::from(width) + u64::from(x)) * 4) as usize
}

#[allow(clippy::cast_possible_truncation)]
pub(crate) fn sample_index(width: u32, x: u32, y: u32) -> usize {
    (u64::from(y) * u64::from(width) + u64::from(x)) as usize
}

#[cfg(test)]
mod tests {
    use super::{LogicalSample, MAX_SCALE, Scale, reduce};
    use crate::components::raster::{LinearRgb, Raster};

    #[test]
    fn validates_scale_range() {
        assert!(Scale::new(0).is_err());
        assert_eq!(Scale::new(1), Ok(Scale::MIN));
        assert!(Scale::new(MAX_SCALE).is_ok());
        assert!(Scale::new(MAX_SCALE + 1).is_err());
    }

    #[test]
    fn keeps_partial_blocks_at_odd_edges() {
        let source = Raster::new(3, 1, vec![0, 0, 0, 255, 255, 255, 255, 255, 255, 0, 0, 255])
            .expect("the fixture is valid");
        let reduced = reduce(&source, Scale::new(2).expect("the scale is valid"));

        assert_eq!((reduced.width, reduced.height), (2, 1));
        assert_eq!(reduced.scale.get(), 2);
        assert_eq!(reduced.samples[1].color, LinearRgb::new(1.0, 0.0, 0.0));
    }

    #[test]
    fn weights_visible_pixels_by_alpha_and_ignores_zero_alpha() {
        let source = Raster::new(3, 1, vec![255, 0, 0, 0, 0, 0, 0, 255, 255, 255, 255, 128])
            .expect("the fixture is valid");
        let reduced = reduce(&source, Scale::new(3).expect("the scale is valid"));
        let expected = (128.0 / 255.0) / (1.0 + 128.0 / 255.0);

        assert!(reduced.samples[0].visible);
        assert!((reduced.samples[0].color.r - expected).abs() < 0.000_01);
        assert!((reduced.samples[0].color.g - reduced.samples[0].color.r).abs() < 0.000_01);
        assert!((reduced.samples[0].color.b - reduced.samples[0].color.r).abs() < 0.000_01);
    }

    #[test]
    fn marks_a_fully_transparent_block_as_invisible() {
        let source = Raster::new(1, 1, vec![255, 0, 0, 0]).expect("the fixture is valid");
        let reduced = reduce(&source, Scale::MIN);

        assert_eq!(
            reduced.samples,
            vec![LogicalSample {
                color: LinearRgb::new(0.0, 0.0, 0.0),
                visible: false,
            }]
        );
    }
}
