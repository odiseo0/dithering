use dither_engine::{
    AtomicCancellationToken, EffectConfig, Palette, Raster, RenderError, Rgb8, Scale, render,
};

#[test]
fn public_types_build_a_valid_default_request() {
    let source = Raster::new(1, 1, vec![0, 0, 0, 255]).expect("valid fixture");
    let palette = Palette::new(vec![Rgb8::BLACK, Rgb8::WHITE]).expect("valid palette");
    let scale = Scale::new(1).expect("valid scale");

    assert_eq!(source.width(), 1);
    assert_eq!(palette.colors().len(), 2);
    assert_eq!(scale.get(), 1);
    assert_eq!(
        EffectConfig::default().kind(),
        dither_engine::EffectKind::ErrorDiffusion
    );
}

#[test]
fn public_render_contract_accepts_shared_cancellation() {
    let source = Raster::new(1, 1, vec![0, 0, 0, 255]).expect("valid fixture");
    let cancellation = AtomicCancellationToken::new();
    cancellation.cancel();

    assert_eq!(
        render(
            &source,
            &EffectConfig::default(),
            &cancellation,
            &dither_engine::NoProgress,
        ),
        Err(RenderError::Cancelled)
    );
}
