use std::{hint::black_box, time::Instant};

use dither_engine::{
    EffectConfig, ErrorDiffusionAlgorithm, ErrorDiffusionConfig, NeverCancel, NoProgress, Palette,
    Raster, Scale, render,
};

const SIDE: u32 = 2_048;

fn main() {
    let source = gradient_fixture();

    for algorithm in [
        ErrorDiffusionAlgorithm::FloydSteinberg,
        ErrorDiffusionAlgorithm::Atkinson,
    ] {
        let config = EffectConfig::ErrorDiffusion(ErrorDiffusionConfig::new(
            algorithm,
            Scale::MIN,
            Palette::black_and_white(),
        ));
        let started = Instant::now();
        let output = render(black_box(&source), &config, &NeverCancel, &NoProgress)
            .expect("the benchmark render must succeed");
        let elapsed = started.elapsed();
        let megapixels = f64::from(SIDE) * f64::from(SIDE) / 1_000_000.0;

        println!(
            "{algorithm:?}: {elapsed:.2?}, {:.2} MP/s, {} output bytes",
            megapixels / elapsed.as_secs_f64(),
            output.rgba().len()
        );
        black_box(output);
    }
}

fn gradient_fixture() -> Raster {
    let capacity = usize::try_from(u64::from(SIDE) * u64::from(SIDE) * 4)
        .expect("the fixed benchmark size fits in usize");
    let mut rgba = Vec::with_capacity(capacity);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let value = u8::try_from((x + y) % 256).expect("the modulo result fits in u8");
            rgba.extend_from_slice(&[value, value, value, 255]);
        }
    }

    Raster::new(SIDE, SIDE, rgba).expect("the benchmark fixture is valid")
}
