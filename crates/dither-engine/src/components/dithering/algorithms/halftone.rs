use crate::components::{
    dithering::{CancellationCheck, DotShape, HalftoneConfig, ProgressSink, RenderError},
    raster::{Raster, Rgb8},
};

#[allow(clippy::cast_precision_loss)]
pub(crate) fn render_halftone(
    source: &Raster,
    config: HalftoneConfig,
    cancellation: &impl CancellationCheck,
    progress: &impl ProgressSink,
) -> Result<Raster, RenderError> {
    let cell_size = u32::from(config.cell_size());
    let cell_rows = source.height().div_ceil(cell_size);
    let cell_columns = source.width().div_ceil(cell_size);
    let mut rgba = source.rgba().to_vec();

    for cell_y in 0..cell_rows {
        if cancellation.is_cancelled() {
            return Err(RenderError::Cancelled);
        }

        for cell_x in 0..cell_columns {
            let bounds = CellBounds::new(source, cell_size, cell_x, cell_y);
            let coverage = cell_luminance(source, bounds).map_or(0.0, |luminance| {
                if config.inverted() {
                    luminance
                } else {
                    1.0 - luminance
                }
            });
            draw_cell(source.width(), &mut rgba, bounds, config, coverage);
        }

        progress.report((cell_y + 1) as f32 / cell_rows as f32);
    }

    Raster::new(source.width(), source.height(), rgba).map_err(RenderError::InvalidOutput)
}

#[derive(Clone, Copy)]
struct CellBounds {
    start_x: u32,
    start_y: u32,
    end_x: u32,
    end_y: u32,
}

impl CellBounds {
    fn new(source: &Raster, cell_size: u32, cell_x: u32, cell_y: u32) -> Self {
        let start_x = cell_x * cell_size;
        let start_y = cell_y * cell_size;
        Self {
            start_x,
            start_y,
            end_x: (start_x + cell_size).min(source.width()),
            end_y: (start_y + cell_size).min(source.height()),
        }
    }
}

fn cell_luminance(source: &Raster, bounds: CellBounds) -> Option<f32> {
    let mut weighted_luminance = 0.0;
    let mut alpha_sum = 0.0;

    for y in bounds.start_y..bounds.end_y {
        for x in bounds.start_x..bounds.end_x {
            let offset = pixel_offset(source.width(), x, y);
            let pixel = &source.rgba()[offset..offset + 4];
            if pixel[3] == 0 {
                continue;
            }
            let alpha = f32::from(pixel[3]) / 255.0;
            let luminance = Rgb8::new(pixel[0], pixel[1], pixel[2])
                .to_linear()
                .luminance();
            weighted_luminance += luminance * alpha;
            alpha_sum += alpha;
        }
    }

    (alpha_sum > 0.0).then_some(weighted_luminance / alpha_sum)
}

#[allow(clippy::cast_precision_loss)]
fn draw_cell(
    width: u32,
    rgba: &mut [u8],
    bounds: CellBounds,
    config: HalftoneConfig,
    coverage: f32,
) {
    let size = f32::from(config.max_size()) * coverage.clamp(0.0, 1.0).sqrt();
    let half_size = size / 2.0;
    let center_x = (bounds.start_x + bounds.end_x) as f32 / 2.0;
    let center_y = (bounds.start_y + bounds.end_y) as f32 / 2.0;

    for y in bounds.start_y..bounds.end_y {
        for x in bounds.start_x..bounds.end_x {
            let dx = (x as f32 + 0.5) - center_x;
            let dy = (y as f32 + 0.5) - center_y;
            let inside = match config.shape() {
                DotShape::Circle => {
                    half_size > 0.0 && dx.mul_add(dx, dy * dy) <= half_size * half_size
                }
                DotShape::Square => half_size > 0.0 && dx.abs() < half_size && dy.abs() < half_size,
            };
            let color = if inside {
                config.dot()
            } else {
                config.background()
            };
            let offset = pixel_offset(width, x, y);
            rgba[offset..offset + 3].copy_from_slice(&[color.r, color.g, color.b]);
        }
    }
}

#[allow(clippy::cast_possible_truncation)]
fn pixel_offset(width: u32, x: u32, y: u32) -> usize {
    ((u64::from(y) * u64::from(width) + u64::from(x)) * 4) as usize
}
