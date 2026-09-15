use std::{
    fs::{self, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use dither_engine::Raster;
use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
use thiserror::Error;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Error)]
pub enum SaveError {
    #[error("no se pudo crear el archivo temporal")]
    CreateTemporary(#[source] std::io::Error),
    #[error("no se pudo codificar la imagen PNG")]
    Encode(#[source] image::ImageError),
    #[error("no se pudo confirmar la escritura")]
    Flush(#[source] std::io::Error),
    #[error("no se pudo sustituir el archivo de destino")]
    Replace(#[source] std::io::Error),
    #[error("la ruta de destino no tiene un nombre de archivo")]
    InvalidPath,
}

pub struct PngWriter;

impl PngWriter {
    /// Encodes a raster as RGBA8 PNG and replaces the destination only after success.
    ///
    /// # Errors
    ///
    /// Returns [`SaveError`] if the temporary file or final replacement fails.
    pub fn save(path: &Path, raster: &Raster) -> Result<(), SaveError> {
        save_with_encoder(path, |writer| {
            PngEncoder::new(writer)
                .write_image(
                    raster.rgba(),
                    raster.width(),
                    raster.height(),
                    ExtendedColorType::Rgba8,
                )
                .map_err(SaveError::Encode)
        })
    }
}

fn save_with_encoder(
    path: &Path,
    encode: impl FnOnce(&mut dyn Write) -> Result<(), SaveError>,
) -> Result<(), SaveError> {
    let (temporary_path, file) = create_temporary_file(path)?;
    let result = (|| {
        let mut writer = BufWriter::new(file);
        encode(&mut writer)?;
        writer.flush().map_err(SaveError::Flush)?;
        writer.get_ref().sync_all().map_err(SaveError::Flush)?;
        fs::rename(&temporary_path, path).map_err(SaveError::Replace)
    })();

    if result.is_err() {
        let _ignored = fs::remove_file(&temporary_path);
    }
    result
}

fn create_temporary_file(path: &Path) -> Result<(PathBuf, fs::File), SaveError> {
    let parent = path.parent().ok_or(SaveError::InvalidPath)?;
    let file_name = path.file_name().ok_or(SaveError::InvalidPath)?;
    for _ in 0..100 {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let mut temporary_name = file_name.to_os_string();
        temporary_name.push(format!(".{sequence}.tmp"));
        let temporary_path = parent.join(temporary_name);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
        {
            Ok(file) => return Ok((temporary_path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(SaveError::CreateTemporary(error)),
        }
    }
    Err(SaveError::CreateTemporary(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not create a unique temporary file",
    )))
}

#[must_use]
pub fn suggested_output_path(source: &Path) -> PathBuf {
    let stem = source
        .file_stem()
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| std::ffi::OsStr::new("imagen"));
    let mut name = stem.to_os_string();
    name.push("-dither.png");
    source.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use std::{fs, sync::atomic::Ordering};

    use super::{SaveError, save_with_encoder};

    static TEST_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    #[test]
    fn an_encoding_failure_keeps_the_previous_file() {
        let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "dither-safe-save-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&directory).expect("the test directory can be created");
        let destination = directory.join("existing.png");
        fs::write(&destination, b"previous").expect("the old file can be written");

        let result = save_with_encoder(&destination, |writer| {
            writer.write_all(b"partial").map_err(SaveError::Flush)?;
            Err(SaveError::Flush(std::io::Error::other(
                "simulated encoding failure",
            )))
        });

        assert!(result.is_err());
        assert_eq!(
            fs::read(&destination).expect("the old file still exists"),
            b"previous"
        );
        assert_eq!(
            fs::read_dir(&directory).expect("directory exists").count(),
            1
        );
        fs::remove_dir_all(directory).expect("the test directory can be removed");
    }
}
