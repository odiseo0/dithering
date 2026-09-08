use thiserror::Error;

pub const MAX_SIDE: u32 = 16_384;
pub const MAX_PIXELS: u64 = 40_000_000;

const CHANNELS: u64 = 4;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Raster {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl Raster {
    /// # Errors
    ///
    /// Returns [`RasterError`] if dimensions exceed the limits or the buffer length is wrong.
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Result<Self, RasterError> {
        if width == 0 || height == 0 {
            return Err(RasterError::ZeroDimension);
        }

        if width > MAX_SIDE || height > MAX_SIDE {
            return Err(RasterError::SideTooLong {
                width,
                height,
                maximum: MAX_SIDE,
            });
        }

        let pixels = u64::from(width)
            .checked_mul(u64::from(height))
            .ok_or(RasterError::SizeOverflow)?;

        if pixels > MAX_PIXELS {
            return Err(RasterError::TooManyPixels {
                pixels,
                maximum: MAX_PIXELS,
            });
        }

        let expected_u64 = pixels
            .checked_mul(CHANNELS)
            .ok_or(RasterError::SizeOverflow)?;
        let expected = usize::try_from(expected_u64).map_err(|_| RasterError::SizeOverflow)?;

        if rgba.len() != expected {
            return Err(RasterError::InvalidBufferLength {
                actual: rgba.len(),
                expected,
            });
        }

        Ok(Self {
            width,
            height,
            rgba,
        })
    }

    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    #[must_use]
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    #[must_use]
    pub fn into_rgba(self) -> Vec<u8> {
        self.rgba
    }
}

/// An error found while validating an image for the engine.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RasterError {
    #[error("width and height must be greater than zero")]
    ZeroDimension,
    #[error("image side exceeds the configured limit")]
    SideTooLong {
        width: u32,
        height: u32,
        maximum: u32,
    },
    #[error("image contains too many pixels")]
    TooManyPixels { pixels: u64, maximum: u64 },
    #[error("image size overflows the supported integer range")]
    SizeOverflow,
    #[error("RGBA buffer length does not match the image dimensions")]
    InvalidBufferLength { actual: usize, expected: usize },
}

#[cfg(test)]
mod tests {
    use super::{MAX_PIXELS, MAX_SIDE, Raster, RasterError};

    #[test]
    fn accepts_a_valid_rgba_buffer() {
        let raster = Raster::new(2, 1, vec![0; 8]).expect("the fixture is valid");

        assert_eq!(raster.width(), 2);
        assert_eq!(raster.height(), 1);
        assert_eq!(raster.rgba(), &[0; 8]);
    }

    #[test]
    fn rejects_zero_dimensions() {
        assert_eq!(
            Raster::new(0, 1, Vec::new()),
            Err(RasterError::ZeroDimension)
        );
    }

    #[test]
    fn rejects_a_side_over_the_limit_before_buffer_validation() {
        assert_eq!(
            Raster::new(MAX_SIDE + 1, 1, Vec::new()),
            Err(RasterError::SideTooLong {
                width: MAX_SIDE + 1,
                height: 1,
                maximum: MAX_SIDE,
            })
        );
    }

    #[test]
    fn rejects_too_many_pixels_before_buffer_validation() {
        let side = 6_325;
        let pixels = u64::from(side) * u64::from(side);
        assert!(pixels > MAX_PIXELS);
        assert_eq!(
            Raster::new(side, side, Vec::new()),
            Err(RasterError::TooManyPixels {
                pixels,
                maximum: MAX_PIXELS,
            })
        );
    }

    #[test]
    fn rejects_an_incorrect_buffer_length() {
        assert_eq!(
            Raster::new(2, 2, vec![0; 15]),
            Err(RasterError::InvalidBufferLength {
                actual: 15,
                expected: 16,
            })
        );
    }
}
