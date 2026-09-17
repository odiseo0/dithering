use crate::components::{
    dithering::{
        CancellationCheck, ErrorDiffusionAlgorithm, ErrorDiffusionConfig, ProgressSink, RenderError,
    },
    raster::{LinearRgb, Raster, Rgb8, scale},
};

#[allow(clippy::cast_precision_loss)]
pub(crate) fn render_error_diffusion(
    source: &Raster,
    config: &ErrorDiffusionConfig,
    cancellation: &impl CancellationCheck,
    progress: &impl ProgressSink,
) -> Result<Raster, RenderError> {
    let logical = scale::reduce_cancellable(source, config.scale(), || cancellation.is_cancelled())
        .ok_or(RenderError::Cancelled)?;
    let mut work: Vec<_> = logical.samples.iter().map(|sample| sample.color).collect();
    let mut output = vec![Rgb8::BLACK; logical.samples.len()];

    for y in 0..logical.height {
        if cancellation.is_cancelled() {
            return Err(RenderError::Cancelled);
        }

        for x in 0..logical.width {
            let index = scale::sample_index(logical.width, x, y);
            let sample = logical.samples[index];
            let (_, chosen) = config.palette().nearest(work[index]);
            output[index] = chosen;

            if !sample.visible {
                continue;
            }

            let current = work[index].clamped();
            let quantized = chosen.to_linear();
            let error = LinearRgb::new(
                current.r - quantized.r,
                current.g - quantized.g,
                current.b - quantized.b,
            );
            diffuse(&logical, &mut work, x, y, error, config.algorithm());
        }

        progress.report((y + 1) as f32 / logical.height as f32);
    }

    scale::expand(source, &logical, &output).map_err(RenderError::InvalidOutput)
}

fn diffuse(
    logical: &scale::LogicalRaster,
    work: &mut [LinearRgb],
    x: u32,
    y: u32,
    error: LinearRgb,
    algorithm: ErrorDiffusionAlgorithm,
) {
    match algorithm {
        ErrorDiffusionAlgorithm::FloydSteinberg => {
            add_error(
                logical,
                work,
                i64::from(x) + 1,
                i64::from(y),
                error,
                7.0 / 16.0,
            );
            add_error(
                logical,
                work,
                i64::from(x) - 1,
                i64::from(y) + 1,
                error,
                3.0 / 16.0,
            );
            add_error(
                logical,
                work,
                i64::from(x),
                i64::from(y) + 1,
                error,
                5.0 / 16.0,
            );
            add_error(
                logical,
                work,
                i64::from(x) + 1,
                i64::from(y) + 1,
                error,
                1.0 / 16.0,
            );
        }
        ErrorDiffusionAlgorithm::Atkinson => {
            for (dx, dy) in [(1, 0), (2, 0), (-1, 1), (0, 1), (1, 1), (0, 2)] {
                add_error(
                    logical,
                    work,
                    i64::from(x) + dx,
                    i64::from(y) + dy,
                    error,
                    1.0 / 8.0,
                );
            }
        }
    }
}

fn add_error(
    logical: &scale::LogicalRaster,
    work: &mut [LinearRgb],
    x: i64,
    y: i64,
    error: LinearRgb,
    weight: f32,
) {
    let Ok(x) = u32::try_from(x) else { return };
    let Ok(y) = u32::try_from(y) else { return };
    if x >= logical.width || y >= logical.height {
        return;
    }

    let index = scale::sample_index(logical.width, x, y);
    if !logical.samples[index].visible {
        return;
    }

    work[index].r += error.r * weight;
    work[index].g += error.g * weight;
    work[index].b += error.b * weight;
}

#[cfg(test)]
mod tests {
    use super::diffuse;
    use crate::components::{
        dithering::ErrorDiffusionAlgorithm,
        raster::{LinearRgb, Scale, scale},
    };

    fn visible_grid(width: u32, height: u32) -> scale::LogicalRaster {
        scale::LogicalRaster {
            width,
            height,
            scale: Scale::MIN,
            samples: vec![
                scale::LogicalSample {
                    color: LinearRgb::new(0.0, 0.0, 0.0),
                    visible: true,
                };
                usize::try_from(width * height).expect("the fixture size fits in usize")
            ],
        }
    }

    #[test]
    fn atkinson_writes_one_eighth_to_each_of_six_neighbors() {
        let logical = visible_grid(5, 5);
        let mut work = vec![LinearRgb::new(0.0, 0.0, 0.0); logical.samples.len()];
        diffuse(
            &logical,
            &mut work,
            1,
            1,
            LinearRgb::new(0.8, 0.4, 0.2),
            ErrorDiffusionAlgorithm::Atkinson,
        );

        let expected_neighbors = [(2, 1), (3, 1), (0, 2), (1, 2), (2, 2), (1, 3)];
        for y in 0..logical.height {
            for x in 0..logical.width {
                let value = work[scale::sample_index(logical.width, x, y)];
                if expected_neighbors.contains(&(x, y)) {
                    assert_eq!(value, LinearRgb::new(0.1, 0.05, 0.025));
                } else {
                    assert_eq!(value, LinearRgb::new(0.0, 0.0, 0.0));
                }
            }
        }
    }

    #[test]
    fn diffusion_does_not_write_to_transparent_neighbors() {
        let mut logical = visible_grid(3, 2);
        let transparent = scale::sample_index(logical.width, 2, 0);
        logical.samples[transparent].visible = false;
        let mut work = vec![LinearRgb::new(0.0, 0.0, 0.0); logical.samples.len()];

        diffuse(
            &logical,
            &mut work,
            1,
            0,
            LinearRgb::new(1.0, 1.0, 1.0),
            ErrorDiffusionAlgorithm::FloydSteinberg,
        );

        assert_eq!(work[transparent], LinearRgb::new(0.0, 0.0, 0.0));
    }
}
