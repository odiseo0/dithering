use std::cell::{Cell, RefCell};

use dither_engine::{
    CancellationCheck, EffectConfig, ErrorDiffusionAlgorithm, ErrorDiffusionConfig, NeverCancel,
    NoProgress, Palette, ProgressSink, Raster, RenderError, Rgb8, Scale, render,
};

fn render_gray_row(value: u8, algorithm: ErrorDiffusionAlgorithm) -> Raster {
    let source = Raster::new(
        3,
        1,
        vec![
            value, value, value, 255, value, value, value, 255, value, value, value, 255,
        ],
    )
    .expect("the fixture is valid");
    let config = EffectConfig::ErrorDiffusion(ErrorDiffusionConfig::new(
        algorithm,
        Scale::MIN,
        Palette::black_and_white(),
    ));

    render(&source, &config, &NeverCancel, &NoProgress).expect("rendering must succeed")
}

#[test]
fn floyd_steinberg_matches_a_reference_row() {
    let output = render_gray_row(160, ErrorDiffusionAlgorithm::FloydSteinberg);

    assert_eq!(
        output.rgba(),
        &[0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 255]
    );
}

#[test]
fn atkinson_matches_a_reference_row() {
    let output = render_gray_row(160, ErrorDiffusionAlgorithm::Atkinson);

    assert_eq!(output.rgba(), &[0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255]);
}

#[test]
fn scale_expands_partial_blocks_and_preserves_alpha() {
    let source = Raster::new(
        3,
        1,
        vec![255, 255, 255, 0, 0, 0, 0, 128, 255, 255, 255, 64],
    )
    .expect("the fixture is valid");
    let config = EffectConfig::ErrorDiffusion(ErrorDiffusionConfig::new(
        ErrorDiffusionAlgorithm::FloydSteinberg,
        Scale::new(2).expect("the scale is valid"),
        Palette::black_and_white(),
    ));

    let output = render(&source, &config, &NeverCancel, &NoProgress)
        .expect("rendering must succeed")
        .into_rgba();

    assert_eq!(&output[0..8], &[0, 0, 0, 0, 0, 0, 0, 128]);
    assert_eq!(&output[8..12], &[255, 255, 255, 64]);
    assert_eq!(
        output
            .iter()
            .skip(3)
            .step_by(4)
            .copied()
            .collect::<Vec<_>>(),
        vec![0, 128, 64]
    );
}

#[test]
fn transparent_samples_do_not_receive_or_send_error() {
    let source = Raster::new(
        3,
        1,
        vec![160, 160, 160, 255, 255, 255, 255, 0, 160, 160, 160, 255],
    )
    .expect("the fixture is valid");

    let output = render(&source, &EffectConfig::default(), &NeverCancel, &NoProgress)
        .expect("rendering must succeed");

    assert_eq!(output.rgba(), &[0, 0, 0, 255, 0, 0, 0, 0, 0, 0, 0, 255]);
}

#[derive(Default)]
struct Trace {
    values: RefCell<Vec<f32>>,
}

impl ProgressSink for Trace {
    fn report(&self, value: f32) {
        self.values.borrow_mut().push(value);
    }
}

struct CancelOnThirdCheck(Cell<u8>);

impl CancellationCheck for CancelOnThirdCheck {
    fn is_cancelled(&self) -> bool {
        let calls = self.0.get() + 1;
        self.0.set(calls);
        calls >= 3
    }
}

#[test]
fn reports_each_logical_row_and_stops_when_cancelled() {
    let source = Raster::new(
        1,
        3,
        vec![128, 128, 128, 255, 128, 128, 128, 255, 128, 128, 128, 255],
    )
    .expect("the fixture is valid");
    let progress = Trace::default();

    let result = render(
        &source,
        &EffectConfig::default(),
        &CancelOnThirdCheck(Cell::new(0)),
        &progress,
    );

    assert_eq!(result, Err(RenderError::Cancelled));
    assert_eq!(*progress.values.borrow(), vec![0.0, 1.0 / 3.0]);
}

#[test]
fn output_is_deterministic_and_uses_only_palette_colors() {
    let palette = Palette::new(vec![Rgb8::new(10, 20, 30), Rgb8::new(200, 210, 220)])
        .expect("the palette is valid");
    let config = EffectConfig::ErrorDiffusion(ErrorDiffusionConfig::new(
        ErrorDiffusionAlgorithm::Atkinson,
        Scale::MIN,
        palette.clone(),
    ));
    let source = Raster::new(
        2,
        2,
        vec![
            90, 100, 110, 1, 90, 100, 110, 1, 90, 100, 110, 1, 90, 100, 110, 1,
        ],
    )
    .expect("the fixture is valid");

    let first =
        render(&source, &config, &NeverCancel, &NoProgress).expect("rendering must succeed");
    let second =
        render(&source, &config, &NeverCancel, &NoProgress).expect("rendering must succeed");

    assert_eq!(first, second);
    for pixel in first.rgba().chunks_exact(4) {
        let color = Rgb8::new(pixel[0], pixel[1], pixel[2]);
        assert!(palette.colors().contains(&color));
    }
}
