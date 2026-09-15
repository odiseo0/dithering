use dither_desktop::components::{
    document::{DecodedImage, DocumentState, ResultView, SourceFormat, Status},
    processing::{DocumentId, ProcessingEvent, Revision},
};
use dither_engine::Raster;

fn decoded(marker: u8) -> DecodedImage {
    DecodedImage {
        raster: Raster::new(1, 1, vec![marker, 0, 0, 255]).expect("valid test raster"),
        format: SourceFormat::Png,
    }
}

#[test]
fn a_loaded_document_starts_its_first_revision_without_a_result() {
    let mut state = DocumentState::new();
    state.load_succeeded(
        DocumentId::new(5),
        "entrada.png".to_owned(),
        None,
        decoded(1),
    );

    let document = state.document.as_ref().expect("document exists");
    assert_eq!(document.revision, Revision::new(1));
    assert!(document.result.is_none());
    assert!(!state.can_save());
    assert_eq!(state.view, ResultView::Result);
    assert!(matches!(state.status, Status::Processing { .. }));
}

#[test]
fn an_old_result_never_replaces_or_enables_the_current_result() {
    let mut state = DocumentState::new();
    state.load_succeeded(
        DocumentId::new(5),
        "entrada.png".to_owned(),
        None,
        decoded(1),
    );
    assert!(state.settings_changed());

    let accepted = state.apply_processing_event(ProcessingEvent::Completed {
        document_id: DocumentId::new(5),
        revision: Revision::new(1),
        raster: decoded(2).raster,
    });

    assert!(!accepted);
    assert!(!state.can_save());
    assert!(
        state
            .document
            .as_ref()
            .expect("document exists")
            .result
            .is_none()
    );
}

#[test]
fn a_current_result_enables_save_until_settings_change() {
    let mut state = DocumentState::new();
    state.load_succeeded(
        DocumentId::new(5),
        "entrada.png".to_owned(),
        None,
        decoded(1),
    );
    assert!(state.apply_processing_event(ProcessingEvent::Completed {
        document_id: DocumentId::new(5),
        revision: Revision::new(1),
        raster: decoded(2).raster,
    }));
    assert!(state.can_save());

    assert!(state.settings_changed());
    assert!(!state.can_save());
    assert!(state.result_is_stale());
    assert_eq!(
        state
            .displayed_raster()
            .expect("old preview remains")
            .rgba()[0],
        2
    );
}

#[test]
fn load_failure_keeps_the_previous_document() {
    let mut state = DocumentState::new();
    state.load_succeeded(
        DocumentId::new(5),
        "entrada.png".to_owned(),
        None,
        decoded(1),
    );
    state.load_failed("No se pudo abrir la imagen".to_owned(), "dañada".to_owned());

    assert_eq!(
        state.document.as_ref().expect("old document remains").id,
        DocumentId::new(5)
    );
    assert!(matches!(state.status, Status::LoadFailed(_)));
}

#[test]
fn saving_state_disables_a_current_result_and_recovers_after_failure() {
    let mut state = DocumentState::new();
    state.load_succeeded(
        DocumentId::new(5),
        "entrada.png".to_owned(),
        None,
        decoded(1),
    );
    state.apply_processing_event(ProcessingEvent::Completed {
        document_id: DocumentId::new(5),
        revision: Revision::new(1),
        raster: decoded(2).raster,
    });
    state.save_started();
    assert!(!state.can_save());
    assert_eq!(state.status, Status::Saving);

    state.save_failed("No se pudo guardar".to_owned(), "sin permiso".to_owned());
    assert!(state.can_save());
    assert!(matches!(state.status, Status::SaveFailed(_)));
}
