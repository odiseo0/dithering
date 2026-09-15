use std::{
    fs,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use dither_desktop::components::{
    document::{DocumentState, ImageCodec, PngWriter, SourceFormat},
    processing::{Dispatch, DocumentId, ProcessingCoordinator, RenderRequest},
    settings::Settings,
};
use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};

#[test]
fn opens_processes_saves_and_reopens_one_image() {
    let source_rgba = [20, 40, 60, 128, 220, 230, 240, 255];
    let mut encoded = Vec::new();
    PngEncoder::new(&mut encoded)
        .write_image(&source_rgba, 2, 1, ExtendedColorType::Rgba8)
        .expect("PNG fixture encodes");
    let decoded = ImageCodec::decode(&encoded).expect("PNG fixture decodes");
    let mut state = DocumentState::new();
    state.load_succeeded(DocumentId::new(1), "entrada.png".to_owned(), None, decoded);
    let document = state.document.as_ref().expect("document exists");
    let mut coordinator = ProcessingCoordinator::new().expect("worker starts");
    coordinator.submit(
        RenderRequest::new(
            document.id,
            document.revision,
            Arc::clone(&document.source),
            Settings::default().effect_config(),
        ),
        Dispatch::Immediate,
    );

    let deadline = Instant::now() + Duration::from_secs(2);
    while !state.can_save() {
        if let Some(event) = coordinator.poll_event() {
            state.apply_processing_event(event);
        }
        assert!(Instant::now() < deadline, "render did not finish in time");
        thread::sleep(Duration::from_millis(1));
    }

    let directory = std::env::temp_dir().join(format!("dither-flow-{}", std::process::id()));
    fs::create_dir_all(&directory).expect("test directory can be created");
    let destination = directory.join("resultado.png");
    let (_, result) = state
        .document
        .as_ref()
        .and_then(|document| document.result.as_ref())
        .expect("current result exists");
    PngWriter::save(&destination, result).expect("current result saves");
    let reopened = ImageCodec::load_path(&destination).expect("saved result reopens");

    assert_eq!(reopened.format, SourceFormat::Png);
    assert_eq!((reopened.raster.width(), reopened.raster.height()), (2, 1));
    assert_eq!(
        reopened
            .raster
            .rgba()
            .chunks_exact(4)
            .map(|pixel| pixel[3])
            .collect::<Vec<_>>(),
        vec![128, 255]
    );
    fs::remove_dir_all(directory).expect("test directory can be removed");
}
