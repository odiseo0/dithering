use std::borrow::Cow;

use dither_engine::{Raster, RasterError};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardImage {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

pub trait ImageClipboard {
    /// Reads the current image from the clipboard.
    ///
    /// # Errors
    ///
    /// Returns [`ClipboardError`] if no image exists or the clipboard cannot be read.
    fn read_image(&mut self) -> Result<Raster, ClipboardError>;
}

#[derive(Debug, Error)]
pub enum ClipboardError {
    #[error("el portapapeles no contiene una imagen")]
    NoImage,
    #[error("no se pudo acceder al portapapeles")]
    Access(#[source] arboard::Error),
    #[error("la imagen del portapapeles no es válida")]
    InvalidImage(#[source] RasterError),
    #[error("las dimensiones del portapapeles son demasiado grandes")]
    DimensionOverflow,
}

pub struct ArboardClipboard {
    inner: arboard::Clipboard,
}

impl ArboardClipboard {
    /// Opens the system clipboard.
    ///
    /// # Errors
    ///
    /// Returns [`ClipboardError`] if the clipboard cannot be opened.
    pub fn new() -> Result<Self, ClipboardError> {
        let inner = arboard::Clipboard::new().map_err(ClipboardError::Access)?;

        Ok(Self { inner })
    }
}

impl ImageClipboard for ArboardClipboard {
    fn read_image(&mut self) -> Result<Raster, ClipboardError> {
        let image = self.inner.get_image().map_err(|error| match error {
            arboard::Error::ContentNotAvailable => ClipboardError::NoImage,
            other => ClipboardError::Access(other),
        })?;

        clipboard_data_to_raster(ClipboardImage {
            width: image.width,
            height: image.height,
            rgba: Cow::into_owned(image.bytes),
        })
    }
}

/// Converts clipboard RGBA bytes into an engine raster.
///
/// # Errors
///
/// Returns [`ClipboardError`] if dimensions do not fit or the buffer is invalid.
pub fn clipboard_data_to_raster(image: ClipboardImage) -> Result<Raster, ClipboardError> {
    let width = u32::try_from(image.width).map_err(|_| ClipboardError::DimensionOverflow)?;
    let height = u32::try_from(image.height).map_err(|_| ClipboardError::DimensionOverflow)?;
    Raster::new(width, height, image.rgba).map_err(ClipboardError::InvalidImage)
}
