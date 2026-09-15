use std::{path::PathBuf, sync::Arc};

use dither_engine::Raster;

use crate::components::processing::{DocumentId, ProcessingEvent, Revision};

use super::{DecodedImage, SourceFormat};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResultView {
    Original,
    Result,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Empty,
    Loading,
    LoadFailed(String),
    Processing { progress: f32 },
    ResultReady,
    RenderFailed(String),
    Saving,
    SaveFailed(String),
}

#[derive(Clone, Debug)]
pub struct Document {
    pub id: DocumentId,
    pub revision: Revision,
    pub name: String,
    pub source_path: Option<PathBuf>,
    pub source_format: SourceFormat,
    pub source: Arc<Raster>,
    pub result: Option<(Revision, Arc<Raster>)>,
}

#[derive(Clone, Debug)]
pub struct DocumentState {
    pub document: Option<Document>,
    pub status: Status,
    pub view: ResultView,
    pub error_details: Option<String>,
}

impl DocumentState {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            document: None,
            status: Status::Empty,
            view: ResultView::Result,
            error_details: None,
        }
    }

    pub fn start_loading(&mut self) {
        self.status = Status::Loading;
        self.error_details = None;
    }

    pub fn load_succeeded(
        &mut self,
        id: DocumentId,
        name: String,
        source_path: Option<PathBuf>,
        decoded: DecodedImage,
    ) {
        self.document = Some(Document {
            id,
            revision: Revision::new(1),
            name,
            source_path,
            source_format: decoded.format,
            source: Arc::new(decoded.raster),
            result: None,
        });
        self.status = Status::Processing { progress: 0.0 };
        self.view = ResultView::Result;
        self.error_details = None;
    }

    pub fn load_failed(&mut self, message: String, details: String) {
        self.status = Status::LoadFailed(message);
        self.error_details = Some(details);
    }

    pub fn settings_changed(&mut self) -> bool {
        let Some(document) = self.document.as_mut() else {
            return false;
        };
        let Some(revision) = document.revision.next() else {
            self.status = Status::RenderFailed("No se pueden crear más revisiones".to_owned());
            return false;
        };
        document.revision = revision;
        self.status = Status::Processing { progress: 0.0 };
        self.error_details = None;
        true
    }

    pub fn apply_processing_event(&mut self, event: ProcessingEvent) -> bool {
        let Some(document) = self.document.as_mut() else {
            return false;
        };
        match event {
            ProcessingEvent::Progress {
                document_id,
                revision,
                value,
            } if document_id == document.id && revision == document.revision => {
                self.status = Status::Processing {
                    progress: value.clamp(0.0, 1.0),
                };
                true
            }
            ProcessingEvent::Completed {
                document_id,
                revision,
                raster,
            } if document_id == document.id && revision == document.revision => {
                document.result = Some((revision, Arc::new(raster)));
                self.status = Status::ResultReady;
                self.view = ResultView::Result;
                true
            }
            ProcessingEvent::Failed {
                document_id,
                revision,
                error,
            } if document_id == document.id && revision == document.revision => {
                self.status = Status::RenderFailed("No se pudo procesar la imagen".to_owned());
                self.error_details = Some(error.to_string());
                true
            }
            _ => false,
        }
    }

    #[must_use]
    pub fn can_save(&self) -> bool {
        let Some(document) = &self.document else {
            return false;
        };
        document
            .result
            .as_ref()
            .is_some_and(|(revision, _)| *revision == document.revision)
            && !matches!(self.status, Status::Saving)
    }

    #[must_use]
    pub fn displayed_raster(&self) -> Option<&Arc<Raster>> {
        let document = self.document.as_ref()?;
        match self.view {
            ResultView::Original => Some(&document.source),
            ResultView::Result => document
                .result
                .as_ref()
                .map_or(Some(&document.source), |(_, raster)| Some(raster)),
        }
    }

    #[must_use]
    pub fn result_is_stale(&self) -> bool {
        self.document.as_ref().is_some_and(|document| {
            document
                .result
                .as_ref()
                .is_some_and(|(revision, _)| *revision != document.revision)
        })
    }

    pub fn save_started(&mut self) {
        if self.can_save() {
            self.status = Status::Saving;
            self.error_details = None;
        }
    }

    pub fn save_succeeded(&mut self) {
        if matches!(self.status, Status::Saving) {
            self.status = Status::ResultReady;
        }
    }

    pub fn save_failed(&mut self, message: String, details: String) {
        self.status = Status::SaveFailed(message);
        self.error_details = Some(details);
    }
}

impl Default for DocumentState {
    fn default() -> Self {
        Self::new()
    }
}
