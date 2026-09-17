use std::cell::{Cell, RefCell};

use dither_engine::{
    BayerMatrix, CancellationCheck, EffectConfig, NeverCancel, NoProgress, OrderedConfig, Palette,
    ProgressSink, Raster, RenderError, Rgb8, Scale, render,
};

fn ordered_config(matrix: BayerMatrix, scale: Scale, palette: Palette) -> EffectConfig {
    EffectConfig::Ordered(OrderedConfig::new(matrix, scale, palette))
}

fn four_by_four_gradient(colors: [Rgb8; 4]) -> Raster {
    let mut rgba = Vec::with_capacity(4 * 4 * 4);
    for _ in 0..4 {
        for color in colors {
            rgba.extend_from_slice(&[color.r, color.g, color.b, 255]);
        }
    }
    Raster::new(4, 4, rgba).expect("the fixture is valid")
}

#[test]
fn bayer_four_matches_the_grayscale_reference() {
    let source = four_by_four_gradient([
        Rgb8::BLACK,
        Rgb8::new(137, 137, 137),
        Rgb8::new(188, 188, 188),
        Rgb8::new(225, 225, 225),
    ]);
    let output = render(
        &source,
        &ordered_config(BayerMatrix::Four, Scale::MIN, Palette::black_and_white()),
        &NeverCancel,
        &NoProgress,
    )
    .expect("rendering must succeed");
    let expected = [
        0, 0, 255, 255, // row 0
        0, 0, 0, 255, // row 1
        0, 0, 255, 255, // row 2
        0, 0, 0, 255, // row 3
    ];

    for (pixel, expected_white) in output.rgba().chunks_exact(4).zip(expected) {
        let value = if expected_white == 255 { 255 } else { 0 };
        assert_eq!(pixel, &[value, value, value, 255]);
    }
}

#[test]
fn bayer_four_matches_the_color_reference() {
    let red = Rgb8::new(255, 0, 0);
    let blue = Rgb8::new(0, 0, 255);
    let source = four_by_four_gradient([red, Rgb8::new(188, 0, 137), Rgb8::new(137, 0, 188), blue]);
    let output = render(
        &source,
        &ordered_config(
            BayerMatrix::Four,
            Scale::MIN,
            Palette::new(vec![red, blue]).expect("the palette is valid"),
        ),
        &NeverCancel,
        &NoProgress,
    )
    .expect("rendering must succeed");
    let expected_blue = [
        false, false, true, true, // row 0
        false, true, false, true, // row 1
        false, false, true, true, // row 2
        false, false, false, true, // row 3
    ];

    for (pixel, use_blue) in output.rgba().chunks_exact(4).zip(expected_blue) {
        let expected = if use_blue { blue } else { red };
        assert_eq!(pixel, &[expected.r, expected.g, expected.b, 255]);
    }
}

#[test]
fn every_matrix_supports_eight_colors() {
    let palette = Palette::new(vec![
        Rgb8::BLACK,
        Rgb8::WHITE,
        Rgb8::new(255, 0, 0),
        Rgb8::new(0, 255, 0),
        Rgb8::new(0, 0, 255),
        Rgb8::new(255, 255, 0),
        Rgb8::new(255, 0, 255),
        Rgb8::new(0, 255, 255),
    ])
    .expect("the palette is valid");
    let source = four_by_four_gradient([
        Rgb8::new(30, 80, 140),
        Rgb8::new(90, 120, 180),
        Rgb8::new(150, 160, 200),
        Rgb8::new(220, 210, 190),
    ]);

    for matrix in [BayerMatrix::Two, BayerMatrix::Four, BayerMatrix::Eight] {
        let output = render(
            &source,
            &ordered_config(matrix, Scale::MIN, palette.clone()),
            &NeverCancel,
            &NoProgress,
        )
        .expect("rendering must succeed");
        for pixel in output.rgba().chunks_exact(4) {
            assert!(
                palette
                    .colors()
                    .contains(&Rgb8::new(pixel[0], pixel[1], pixel[2]))
            );
        }
    }
}

#[test]
fn scale_handles_partial_blocks_and_preserves_alpha() {
    let source = Raster::new(
        3,
        1,
        vec![255, 255, 255, 0, 0, 0, 0, 128, 255, 255, 255, 64],
    )
    .expect("the fixture is valid");
    let output = render(
        &source,
        &ordered_config(
            BayerMatrix::Two,
            Scale::new(2).expect("the scale is valid"),
            Palette::black_and_white(),
        ),
        &NeverCancel,
        &NoProgress,
    )
    .expect("rendering must succeed");

    assert_eq!(
        output
            .rgba()
            .iter()
            .skip(3)
            .step_by(4)
            .copied()
            .collect::<Vec<_>>(),
        vec![0, 128, 64]
    );
    assert_eq!(&output.rgba()[0..8], &[0, 0, 0, 0, 0, 0, 0, 128]);
    assert_eq!(&output.rgba()[8..12], &[255, 255, 255, 64]);
}

struct CancelOnSixthCheck(Cell<u8>);

impl CancellationCheck for CancelOnSixthCheck {
    fn is_cancelled(&self) -> bool {
        let calls = self.0.get() + 1;
        self.0.set(calls);
        calls >= 6
    }
}

#[derive(Default)]
struct Trace(RefCell<Vec<f32>>);

impl ProgressSink for Trace {
    fn report(&self, value: f32) {
        self.0.borrow_mut().push(value);
    }
}

#[test]
fn reports_rows_and_can_stop_between_them() {
    let source = Raster::new(
        1,
        3,
        vec![128, 128, 128, 255, 128, 128, 128, 255, 128, 128, 128, 255],
    )
    .expect("the fixture is valid");
    let progress = Trace::default();
    let result = render(
        &source,
        &ordered_config(BayerMatrix::Two, Scale::MIN, Palette::black_and_white()),
        &CancelOnSixthCheck(Cell::new(0)),
        &progress,
    );

    assert_eq!(result, Err(RenderError::Cancelled));
    assert_eq!(*progress.0.borrow(), vec![0.0, 1.0 / 3.0]);
}
