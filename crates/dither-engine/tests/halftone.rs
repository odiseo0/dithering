use std::cell::{Cell, RefCell};

use dither_engine::{
    CancellationCheck, DotShape, EffectConfig, HalftoneConfig, NeverCancel, NoProgress,
    ProgressSink, Raster, RenderError, Rgb8, render,
};

fn config(shape: DotShape, cell_size: u8, max_size: u8, inverted: bool) -> EffectConfig {
    EffectConfig::Halftone(
        HalftoneConfig::new(
            Rgb8::WHITE,
            Rgb8::BLACK,
            shape,
            cell_size,
            max_size,
            inverted,
        )
        .expect("the halftone settings are valid"),
    )
}

fn solid(width: u32, height: u32, value: u8) -> Raster {
    let pixels = usize::try_from(u64::from(width) * u64::from(height))
        .expect("the fixture size fits in usize");
    let mut rgba = Vec::with_capacity(pixels * 4);
    for _ in 0..pixels {
        rgba.extend_from_slice(&[value, value, value, 255]);
    }
    Raster::new(width, height, rgba).expect("the fixture is valid")
}

fn dot_count(raster: &Raster) -> usize {
    raster
        .rgba()
        .chunks_exact(4)
        .filter(|pixel| pixel[0..3] == [0, 0, 0])
        .count()
}

#[test]
fn white_and_black_match_the_square_references() {
    let settings = config(DotShape::Square, 4, 4, false);
    let white = render(&solid(4, 4, 255), &settings, &NeverCancel, &NoProgress)
        .expect("rendering must succeed");
    let black = render(&solid(4, 4, 0), &settings, &NeverCancel, &NoProgress)
        .expect("rendering must succeed");

    assert_eq!(dot_count(&white), 0);
    assert_eq!(dot_count(&black), 16);
}

#[test]
fn middle_gray_draws_a_centered_square() {
    let output = render(
        &solid(4, 4, 188),
        &config(DotShape::Square, 4, 4, false),
        &NeverCancel,
        &NoProgress,
    )
    .expect("rendering must succeed");
    let expected = [
        255, 255, 255, 255, // row 0
        255, 0, 0, 255, // row 1
        255, 0, 0, 255, // row 2
        255, 255, 255, 255, // row 3
    ];

    for (pixel, expected_value) in output.rgba().chunks_exact(4).zip(expected) {
        assert_eq!(
            pixel,
            &[expected_value, expected_value, expected_value, 255]
        );
    }
}

#[test]
fn a_full_circle_is_centered_and_has_hard_edges() {
    let output = render(
        &solid(4, 4, 0),
        &config(DotShape::Circle, 4, 4, false),
        &NeverCancel,
        &NoProgress,
    )
    .expect("rendering must succeed");
    let expected = [
        255, 0, 0, 255, // row 0
        0, 0, 0, 0, // row 1
        0, 0, 0, 0, // row 2
        255, 0, 0, 255, // row 3
    ];

    for (pixel, expected_value) in output.rgba().chunks_exact(4).zip(expected) {
        assert_eq!(
            pixel,
            &[expected_value, expected_value, expected_value, 255]
        );
    }
}

#[test]
fn inversion_reverses_white_and_black_coverage() {
    let settings = config(DotShape::Square, 4, 4, true);
    let white = render(&solid(4, 4, 255), &settings, &NeverCancel, &NoProgress)
        .expect("rendering must succeed");
    let black = render(&solid(4, 4, 0), &settings, &NeverCancel, &NoProgress)
        .expect("rendering must succeed");

    assert_eq!(dot_count(&white), 16);
    assert_eq!(dot_count(&black), 0);
}

#[test]
fn gradient_reference_has_nonincreasing_dot_coverage() {
    let shades = [0, 137, 188, 225, 255];
    let mut rgba = Vec::with_capacity(40 * 8 * 4);
    for _y in 0..8 {
        for shade in shades {
            for _x in 0..8 {
                rgba.extend_from_slice(&[shade, shade, shade, 255]);
            }
        }
    }
    let source = Raster::new(40, 8, rgba).expect("the fixture is valid");
    let output = render(
        &source,
        &config(DotShape::Square, 8, 8, false),
        &NeverCancel,
        &NoProgress,
    )
    .expect("rendering must succeed");
    let counts: Vec<_> = (0..5)
        .map(|cell| {
            output
                .rgba()
                .chunks_exact(4)
                .enumerate()
                .filter(|(index, pixel)| index % 40 / 8 == cell && pixel[0..3] == [0, 0, 0])
                .count()
        })
        .collect();

    assert_eq!(counts, vec![64, 36, 36, 16, 0]);
}

#[test]
fn partial_cells_and_transparency_preserve_dimensions_and_alpha() {
    let mut source_bytes = Vec::with_capacity(25 * 4);
    for _ in 0..25 {
        source_bytes.extend_from_slice(&[0, 0, 0, 255]);
    }
    for (index, alpha) in [0, 32, 64, 128, 255].into_iter().enumerate() {
        source_bytes[index * 4 + 3] = alpha;
    }
    let source = Raster::new(5, 5, source_bytes).expect("the fixture is valid");
    let output = render(
        &source,
        &config(DotShape::Circle, 4, 4, false),
        &NeverCancel,
        &NoProgress,
    )
    .expect("rendering must succeed");

    assert_eq!((output.width(), output.height()), (5, 5));
    assert_eq!(dot_count(&output), 21);
    assert_eq!(
        output
            .rgba()
            .iter()
            .skip(3)
            .step_by(4)
            .copied()
            .collect::<Vec<_>>(),
        source
            .rgba()
            .iter()
            .skip(3)
            .step_by(4)
            .copied()
            .collect::<Vec<_>>()
    );
    assert!(
        output
            .rgba()
            .chunks_exact(4)
            .all(|pixel| { pixel[0..3] == [0, 0, 0] || pixel[0..3] == [255, 255, 255] })
    );
}

#[test]
fn transparent_pixels_do_not_affect_cell_luminance() {
    let source = Raster::new(
        2,
        2,
        vec![255, 255, 255, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    )
    .expect("the fixture is valid");
    let output = render(
        &source,
        &config(DotShape::Square, 2, 2, false),
        &NeverCancel,
        &NoProgress,
    )
    .expect("rendering must succeed");

    assert_eq!(dot_count(&output), 0);
}

struct CancelOnThirdCheck(Cell<u8>);

impl CancellationCheck for CancelOnThirdCheck {
    fn is_cancelled(&self) -> bool {
        let calls = self.0.get() + 1;
        self.0.set(calls);
        calls >= 3
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
fn reports_cell_rows_and_can_stop_between_them() {
    let source = solid(2, 4, 128);
    let progress = Trace::default();
    let result = render(
        &source,
        &config(DotShape::Square, 2, 2, false),
        &CancelOnThirdCheck(Cell::new(0)),
        &progress,
    );

    assert_eq!(result, Err(RenderError::Cancelled));
    assert_eq!(*progress.0.borrow(), vec![0.0, 0.5]);
}
