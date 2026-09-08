use std::{hint::black_box, time::Instant};

use dither_engine::{
    BayerMatrix, EffectConfig, NeverCancel, NoProgress, OrderedConfig, Palette, Raster, Rgb8,
    Scale, render,
};

const SIDE: u32 = 2_048;

fn main() {
    let source = gradient_fixture();
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
    .expect("the benchmark palette is valid");
    let config = EffectConfig::Ordered(OrderedConfig::new(BayerMatrix::Eight, Scale::MIN, palette));
    let started = Instant::now();
    let output = render(black_box(&source), &config, &NeverCancel, &NoProgress)
        .expect("the benchmark render must succeed");
    let elapsed = started.elapsed();
    let megapixels = f64::from(SIDE) * f64::from(SIDE) / 1_000_000.0;

    println!(
        "BayerEight, 8 colors: {elapsed:.2?}, {:.2} MP/s, {} output bytes",
        megapixels / elapsed.as_secs_f64(),
        output.rgba().len()
    );
    black_box(output);
}

fn gradient_fixture() -> Raster {
    let capacity = usize::try_from(u64::from(SIDE) * u64::from(SIDE) * 4)
        .expect("the fixed benchmark size fits in usize");
    let mut rgba = Vec::with_capacity(capacity);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let red = u8::try_from(x % 256).expect("the modulo result fits in u8");
            let green = u8::try_from(y % 256).expect("the modulo result fits in u8");
            let blue = u8::try_from((x + y) % 256).expect("the modulo result fits in u8");
            rgba.extend_from_slice(&[red, green, blue, 255]);
        }
    }

    Raster::new(SIDE, SIDE, rgba).expect("the benchmark fixture is valid")
}
