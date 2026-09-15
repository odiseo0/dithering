use std::{fmt, fs, io::Cursor, path::Path};

use dither_engine::{MAX_PIXELS, MAX_SIDE, Raster, RasterError};
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};
use thiserror::Error;

const MAX_ENCODED_BYTES: u64 = 512 * 1024 * 1024;
const MAX_DECODE_ALLOC: u64 = MAX_PIXELS * 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceFormat {
    Png,
    Jpeg,
    WebP,
}

impl fmt::Display for SourceFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
            Self::WebP => "WebP",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedImage {
    pub raster: Raster,
    pub format: SourceFormat,
}

#[derive(Debug, Error)]
pub enum ImageLoadError {
    #[error("no se pudo leer el archivo")]
    Read(#[source] std::io::Error),
    #[error("el archivo es demasiado grande")]
    EncodedFileTooLarge,
    #[error("el formato de imagen no está admitido")]
    UnsupportedFormat,
    #[error("las imágenes animadas no están admitidas")]
    AnimatedImage,
    #[error("la imagen está dañada o no se puede decodificar")]
    Decode(#[source] image::ImageError),
    #[error("las dimensiones o el búfer de la imagen no son válidos")]
    InvalidRaster(#[source] RasterError),
}

pub struct ImageCodec;

impl ImageCodec {
    /// Reads and decodes one static image based on its contents.
    ///
    /// # Errors
    ///
    /// Returns [`ImageLoadError`] when the file cannot be read or decoded safely.
    pub fn load_path(path: &Path) -> Result<DecodedImage, ImageLoadError> {
        let metadata = fs::metadata(path).map_err(ImageLoadError::Read)?;

        if metadata.len() > MAX_ENCODED_BYTES {
            return Err(ImageLoadError::EncodedFileTooLarge);
        }

        let bytes = fs::read(path).map_err(ImageLoadError::Read)?;
        Self::decode(&bytes)
    }

    /// Decodes one static image based on its contents.
    ///
    /// # Errors
    ///
    /// Returns [`ImageLoadError`] for unsupported, animated, oversized, or corrupt data.
    pub fn decode(bytes: &[u8]) -> Result<DecodedImage, ImageLoadError> {
        let image_format =
            image::guess_format(bytes).map_err(|_| ImageLoadError::UnsupportedFormat)?;
        let format = match image_format {
            ImageFormat::Png => SourceFormat::Png,
            ImageFormat::Jpeg => SourceFormat::Jpeg,
            ImageFormat::WebP => SourceFormat::WebP,
            _ => return Err(ImageLoadError::UnsupportedFormat),
        };

        if is_animated(bytes, format) {
            return Err(ImageLoadError::AnimatedImage);
        }

        let mut reader = ImageReader::with_format(Cursor::new(bytes), image_format);
        let mut limits = Limits::default();
        limits.max_image_width = Some(MAX_SIDE);
        limits.max_image_height = Some(MAX_SIDE);
        limits.max_alloc = Some(MAX_DECODE_ALLOC);
        reader.limits(limits);
        let mut decoder = reader.into_decoder().map_err(ImageLoadError::Decode)?;
        let (width, height) = decoder.dimensions();
        validate_dimensions(width, height)?;
        let orientation = decoder.orientation().map_err(ImageLoadError::Decode)?;
        let mut dynamic_image =
            DynamicImage::from_decoder(decoder).map_err(ImageLoadError::Decode)?;
        dynamic_image.apply_orientation(orientation);
        let rgba = dynamic_image.into_rgba8();
        let raster = Raster::new(rgba.width(), rgba.height(), rgba.into_raw())
            .map_err(ImageLoadError::InvalidRaster)?;

        Ok(DecodedImage { raster, format })
    }
}

fn validate_dimensions(width: u32, height: u32) -> Result<(), ImageLoadError> {
    if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
        let error = if width == 0 || height == 0 {
            RasterError::ZeroDimension
        } else {
            RasterError::SideTooLong {
                width,
                height,
                maximum: MAX_SIDE,
            }
        };

        return Err(ImageLoadError::InvalidRaster(error));
    }
    let pixels = u64::from(width) * u64::from(height);

    if pixels > MAX_PIXELS {
        return Err(ImageLoadError::InvalidRaster(RasterError::TooManyPixels {
            pixels,
            maximum: MAX_PIXELS,
        }));
    }

    Ok(())
}

fn is_animated(bytes: &[u8], format: SourceFormat) -> bool {
    match format {
        SourceFormat::Png => png_has_animation(bytes),
        SourceFormat::WebP => webp_has_animation(bytes),
        SourceFormat::Jpeg => false,
    }
}

fn png_has_animation(bytes: &[u8]) -> bool {
    const SIGNATURE_LEN: usize = 8;
    let mut offset = SIGNATURE_LEN;

    while let Some(header) = bytes.get(offset..offset.saturating_add(8)) {
        let length = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as usize;
        let chunk_type = &header[4..8];

        if chunk_type == b"acTL" {
            return true;
        }

        let Some(next) = offset
            .checked_add(12)
            .and_then(|value| value.checked_add(length))
        else {
            break;
        };

        if next > bytes.len() || chunk_type == b"IEND" {
            break;
        }

        offset = next;
    }
    false
}

fn webp_has_animation(bytes: &[u8]) -> bool {
    let Some(feature_flags) = bytes.get(20) else {
        return false;
    };

    if bytes.get(12..16) == Some(b"VP8X") && feature_flags & 0x02 != 0 {
        return true;
    }

    let mut offset = 12_usize;

    while let Some(header) = bytes.get(offset..offset.saturating_add(8)) {
        let chunk_type = &header[..4];

        if chunk_type == b"ANIM" || chunk_type == b"ANMF" {
            return true;
        }

        let length = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
        let padded = length.saturating_add(length & 1);
        let Some(next) = offset
            .checked_add(8)
            .and_then(|value| value.checked_add(padded))
        else {
            break;
        };

        if next > bytes.len() {
            break;
        }

        offset = next;
    }

    false
}
