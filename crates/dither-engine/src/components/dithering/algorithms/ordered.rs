use crate::components::{
    dithering::{
        BayerMatrix, CancellationCheck, OrderedConfig, Palette, ProgressSink, RenderError,
    },
    raster::{LinearRgb, Raster, Rgb8, scale},
};

const BAYER_2: [u8; 4] = [0, 2, 3, 1];
const BAYER_4: [u8; 16] = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];
const BAYER_8: [u8; 64] = [
    0, 32, 8, 40, 2, 34, 10, 42, 48, 16, 56, 24, 50, 18, 58, 26, 12, 44, 4, 36, 14, 46, 6, 38, 60,
    28, 52, 20, 62, 30, 54, 22, 3, 35, 11, 43, 1, 33, 9, 41, 51, 19, 59, 27, 49, 17, 57, 25, 15,
    47, 7, 39, 13, 45, 5, 37, 63, 31, 55, 23, 61, 29, 53, 21,
];

#[derive(Clone, Copy)]
struct PaletteSegment {
    first_index: usize,
    second_index: usize,
    first: LinearRgb,
    direction: LinearRgb,
    inverse_length_squared: f32,
}

#[allow(clippy::cast_precision_loss)]
pub(crate) fn render_ordered(
    source: &Raster,
    config: &OrderedConfig,
    cancellation: &impl CancellationCheck,
    progress: &impl ProgressSink,
) -> Result<Raster, RenderError> {
    let logical = scale::reduce(source, config.scale());
    let linear_colors: Vec<_> = config
        .palette()
        .colors()
        .iter()
        .map(|color| color.to_linear())
        .collect();
    let segments = palette_segments(&linear_colors);
    let mut output = Vec::with_capacity(logical.samples.len());

    for y in 0..logical.height {
        if cancellation.is_cancelled() {
            return Err(RenderError::Cancelled);
        }

        for x in 0..logical.width {
            let sample = logical.samples[scale::sample_index(logical.width, x, y)];
            let color = if sample.visible {
                select_color(
                    config.palette(),
                    &linear_colors,
                    &segments,
                    sample.color,
                    threshold(config.matrix(), x, y),
                )
            } else {
                config.palette().nearest(sample.color).1
            };
            output.push(color);
        }
        progress.report((y + 1) as f32 / logical.height as f32);
    }

    scale::expand(source, &logical, &output).map_err(RenderError::InvalidOutput)
}

fn palette_segments(linear: &[LinearRgb]) -> Vec<PaletteSegment> {
    let mut segments = Vec::with_capacity(linear.len() * (linear.len() - 1) / 2);

    for first_index in 0..linear.len() - 1 {
        for second_index in first_index + 1..linear.len() {
            let first = linear[first_index];
            let second = linear[second_index];
            let direction =
                LinearRgb::new(second.r - first.r, second.g - first.g, second.b - first.b);
            let length_squared = direction.distance_squared(LinearRgb::new(0.0, 0.0, 0.0));
            segments.push(PaletteSegment {
                first_index,
                second_index,
                first,
                direction,
                inverse_length_squared: if length_squared == 0.0 {
                    0.0
                } else {
                    length_squared.recip()
                },
            });
        }
    }
    segments
}

fn select_color(
    palette: &Palette,
    linear_colors: &[LinearRgb],
    segments: &[PaletteSegment],
    target: LinearRgb,
    threshold: f32,
) -> Rgb8 {
    let target = target.clamped();
    let colors = palette.colors();
    if let Some(index) = linear_colors
        .iter()
        .position(|color| target.distance_squared(*color) == 0.0)
    {
        return colors[index];
    }
    let mut best = (f32::INFINITY, 0, 1, 0.0);

    for segment in segments {
        let offset = LinearRgb::new(
            target.r - segment.first.r,
            target.g - segment.first.g,
            target.b - segment.first.b,
        );
        let t = if segment.inverse_length_squared == 0.0 {
            0.0
        } else {
            (offset.r.mul_add(
                segment.direction.r,
                offset
                    .g
                    .mul_add(segment.direction.g, offset.b * segment.direction.b),
            ) * segment.inverse_length_squared)
                .clamp(0.0, 1.0)
        };
        let projected = LinearRgb::new(
            segment.direction.r.mul_add(t, segment.first.r),
            segment.direction.g.mul_add(t, segment.first.g),
            segment.direction.b.mul_add(t, segment.first.b),
        );
        let error = target.distance_squared(projected);
        if error < best.0 {
            best = (error, segment.first_index, segment.second_index, t);
        }
    }

    if best.3 > threshold {
        colors[best.2]
    } else {
        colors[best.1]
    }
}

fn threshold(matrix: BayerMatrix, x: u32, y: u32) -> f32 {
    let (side, levels, values): (u32, f32, &[u8]) = match matrix {
        BayerMatrix::Two => (2, 4.0, &BAYER_2),
        BayerMatrix::Four => (4, 16.0, &BAYER_4),
        BayerMatrix::Eight => (8, 64.0, &BAYER_8),
    };
    let index = scale::sample_index(side, x % side, y % side);
    (f32::from(values[index]) + 0.5) / levels
}

#[cfg(test)]
mod tests {
    use super::{BAYER_2, BAYER_4, BAYER_8, palette_segments, select_color, threshold};
    use crate::components::{
        dithering::{BayerMatrix, Palette},
        raster::{LinearRgb, Rgb8},
    };

    #[test]
    fn matrices_contain_each_expected_rank_once() {
        for values in [&BAYER_2[..], &BAYER_4[..], &BAYER_8[..]] {
            let mut sorted = values.to_vec();
            sorted.sort_unstable();
            let length = u8::try_from(values.len()).expect("the matrix size fits in u8");
            assert_eq!(sorted, (0..length).collect::<Vec<_>>());
        }
    }

    #[test]
    fn thresholds_are_centered_inside_zero_and_one() {
        assert!((threshold(BayerMatrix::Two, 0, 0) - 0.125).abs() < f32::EPSILON);
        assert!((threshold(BayerMatrix::Two, 1, 0) - 0.625).abs() < f32::EPSILON);
    }

    #[test]
    fn segment_projection_supports_color_palettes() {
        let red = Rgb8::new(255, 0, 0);
        let blue = Rgb8::new(0, 0, 255);
        let palette = Palette::new(vec![red, blue]).expect("the palette is valid");
        let linear = [red.to_linear(), blue.to_linear()];
        let segments = palette_segments(&linear);
        let midpoint = LinearRgb::new(0.5, 0.0, 0.5);

        assert_eq!(
            select_color(&palette, &linear, &segments, midpoint, 0.25),
            blue
        );
        assert_eq!(
            select_color(&palette, &linear, &segments, midpoint, 0.75),
            red
        );
    }

    #[test]
    fn exact_palette_colors_do_not_change() {
        let red = Rgb8::new(255, 0, 0);
        let blue = Rgb8::new(0, 0, 255);
        let gray = Rgb8::new(128, 128, 128);
        let palette = Palette::new(vec![red, blue, gray]).expect("the palette is valid");
        let linear = [red.to_linear(), blue.to_linear(), gray.to_linear()];
        let segments = palette_segments(&linear);

        assert_eq!(
            select_color(&palette, &linear, &segments, red.to_linear(), 0.01),
            red
        );
        assert_eq!(
            select_color(&palette, &linear, &segments, blue.to_linear(), 0.99),
            blue
        );
        assert_eq!(
            select_color(&palette, &linear, &segments, gray.to_linear(), 0.50),
            gray
        );
    }
}
