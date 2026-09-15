use std::sync::Arc;

use dither_engine::{EffectConfig, Raster, RenderError};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DocumentId(u64);

impl DocumentId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Revision(u64);

impl Revision {
    pub const INITIAL: Self = Self(0);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct RequestIdentity {
    pub document_id: DocumentId,
    pub revision: Revision,
}

#[derive(Clone, Debug)]
pub struct RenderRequest {
    pub document_id: DocumentId,
    pub revision: Revision,
    pub source: Arc<Raster>,
    pub config: EffectConfig,
}

impl RenderRequest {
    #[must_use]
    pub fn new(
        document_id: DocumentId,
        revision: Revision,
        source: Arc<Raster>,
        config: EffectConfig,
    ) -> Self {
        Self {
            document_id,
            revision,
            source,
            config,
        }
    }

    pub(crate) const fn identity(&self) -> RequestIdentity {
        RequestIdentity {
            document_id: self.document_id,
            revision: self.revision,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProcessingEvent {
    Progress {
        document_id: DocumentId,
        revision: Revision,
        value: f32,
    },
    Completed {
        document_id: DocumentId,
        revision: Revision,
        raster: Raster,
    },
    Failed {
        document_id: DocumentId,
        revision: Revision,
        error: RenderError,
    },
}

impl ProcessingEvent {
    pub(crate) const fn identity(&self) -> RequestIdentity {
        match self {
            Self::Progress {
                document_id,
                revision,
                ..
            }
            | Self::Completed {
                document_id,
                revision,
                ..
            }
            | Self::Failed {
                document_id,
                revision,
                ..
            } => RequestIdentity {
                document_id: *document_id,
                revision: *revision,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WorkerEvent {
    pub generation: u64,
    pub event: ProcessingEvent,
}
