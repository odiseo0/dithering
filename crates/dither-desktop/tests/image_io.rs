use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use dither_desktop::components::document::{
    ClipboardError, ClipboardImage, ImageClipboard, ImageCodec, ImageLoadError, PngWriter,
    SourceFormat, clipboard_data_to_raster, suggested_output_path,
};
use dither_engine::Raster;
use image::{
    ExtendedColorType, ImageEncoder,
    codecs::{jpeg::JpegEncoder, png::PngEncoder, webp::WebPEncoder},
};

static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "dither-desktop-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("the test directory can be created");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ignored = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn decodes_png_jpeg_and_webp_by_content() {
    let rgba = [255, 0, 0, 128, 0, 255, 0, 255];
    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(&rgba, 2, 1, ExtendedColorType::Rgba8)
        .expect("PNG encoding succeeds");
    let decoded = ImageCodec::decode(&png).expect("PNG decoding succeeds");
    assert_eq!(decoded.format, SourceFormat::Png);
    assert_eq!(decoded.raster.rgba(), rgba);

    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, 100)
        .write_image(&[255, 0, 0, 0, 255, 0], 2, 1, ExtendedColorType::Rgb8)
        .expect("JPEG encoding succeeds");
    let decoded = ImageCodec::decode(&jpeg).expect("JPEG decoding succeeds");
    assert_eq!(decoded.format, SourceFormat::Jpeg);
    assert_eq!((decoded.raster.width(), decoded.raster.height()), (2, 1));

    let mut webp = Vec::new();
    WebPEncoder::new_lossless(&mut webp)
        .write_image(&rgba, 2, 1, ExtendedColorType::Rgba8)
        .expect("WebP encoding succeeds");
    let decoded = ImageCodec::decode(&webp).expect("WebP decoding succeeds");
    assert_eq!(decoded.format, SourceFormat::WebP);
    assert_eq!(decoded.raster.rgba(), rgba);
}

#[test]
fn converts_rgb_and_sixteen_bit_png_input_to_rgba8() {
    let mut rgb_png = Vec::new();
    PngEncoder::new(&mut rgb_png)
        .write_image(&[10, 20, 30], 1, 1, ExtendedColorType::Rgb8)
        .expect("RGB PNG encoding succeeds");
    assert_eq!(
        ImageCodec::decode(&rgb_png)
            .expect("RGB PNG decoding succeeds")
            .raster
            .rgba(),
        &[10, 20, 30, 255]
    );

    let samples = [u16::MAX, 0, 32_896, u16::MAX];
    let rgba16: Vec<u8> = samples.into_iter().flat_map(u16::to_ne_bytes).collect();
    let mut png16 = Vec::new();
    PngEncoder::new(&mut png16)
        .write_image(&rgba16, 1, 1, ExtendedColorType::Rgba16)
        .expect("16-bit PNG encoding succeeds");
    assert_eq!(
        ImageCodec::decode(&png16)
            .expect("16-bit PNG decoding succeeds")
            .raster
            .rgba(),
        &[255, 0, 128, 255]
    );
}

#[test]
fn applies_jpeg_exif_orientation_before_creating_the_raster() {
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, 100)
        .write_image(&[255, 0, 0, 0, 255, 0], 2, 1, ExtendedColorType::Rgb8)
        .expect("JPEG encoding succeeds");
    insert_orientation_six(&mut jpeg);

    let decoded = ImageCodec::decode(&jpeg).expect("oriented JPEG decoding succeeds");

    assert_eq!((decoded.raster.width(), decoded.raster.height()), (1, 2));
}

#[test]
fn rejects_apng_and_animated_webp_before_decoding_a_frame() {
    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(&[0, 0, 0, 255], 1, 1, ExtendedColorType::Rgba8)
        .expect("PNG encoding succeeds");
    png.splice(
        33..33,
        [
            0, 0, 0, 8, b'a', b'c', b'T', b'L', 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
    );
    assert!(matches!(
        ImageCodec::decode(&png),
        Err(ImageLoadError::AnimatedImage)
    ));

    let animated_webp =
        b"RIFF\x10\x00\x00\x00WEBPVP8X\x0a\x00\x00\x00\x02\x00\x00\x00\x00\x00\x00\x00\x00\x00";
    assert!(matches!(
        ImageCodec::decode(animated_webp),
        Err(ImageLoadError::AnimatedImage)
    ));
}

#[test]
fn rejects_unsupported_and_corrupt_input_without_panicking() {
    assert!(matches!(
        ImageCodec::decode(b"GIF89a"),
        Err(ImageLoadError::UnsupportedFormat)
    ));
    assert!(matches!(
        ImageCodec::decode(b"\x89PNG\r\n\x1a\ncorrupt"),
        Err(ImageLoadError::Decode(_))
    ));
}

#[test]
fn rejects_extreme_dimensions_from_the_header_before_pixel_allocation() {
    let too_wide = png_header(16_385, 1);
    assert!(ImageCodec::decode(&too_wide).is_err());

    let too_many_pixels = png_header(6_325, 6_325);
    assert!(ImageCodec::decode(&too_many_pixels).is_err());
}

#[test]
fn loads_content_with_a_wrong_extension_and_a_unicode_windows_path() {
    let directory = TestDirectory::new("rutas con espacios-á-画像");
    let path = directory.path().join("foto.jpg.txt");
    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(&[10, 20, 30, 40], 1, 1, ExtendedColorType::Rgba8)
        .expect("PNG encoding succeeds");
    fs::write(&path, png).expect("the fixture can be written");

    let decoded = ImageCodec::load_path(&path).expect("content detection succeeds");

    assert_eq!(decoded.format, SourceFormat::Png);
    assert_eq!(decoded.raster.rgba(), &[10, 20, 30, 40]);
}

#[test]
fn saved_png_reopens_and_replaces_an_existing_file() {
    let directory = TestDirectory::new("save");
    let path = directory.path().join("salida.png");
    fs::write(&path, b"previous contents").expect("the previous file can be written");
    let raster = Raster::new(2, 1, vec![1, 2, 3, 4, 5, 6, 7, 8]).expect("valid raster");

    PngWriter::save(&path, &raster).expect("safe PNG save succeeds");
    let reopened = ImageCodec::load_path(&path).expect("saved PNG reopens");

    assert_eq!(reopened.raster, raster);
    let entries = fs::read_dir(directory.path())
        .expect("directory can be read")
        .count();
    assert_eq!(entries, 1, "the temporary file must not remain");
}

#[test]
fn suggests_the_required_output_name() {
    assert_eq!(
        suggested_output_path(Path::new(r"C:\imágenes\entrada.jpeg")),
        PathBuf::from(r"C:\imágenes\entrada-dither.png")
    );
}

#[test]
fn clipboard_adapter_contract_handles_image_and_absence() {
    struct FakeClipboard(Option<ClipboardImage>);
    impl ImageClipboard for FakeClipboard {
        fn read_image(&mut self) -> Result<Raster, ClipboardError> {
            let image = self.0.take().ok_or(ClipboardError::NoImage)?;
            clipboard_data_to_raster(image)
        }
    }

    let mut clipboard = FakeClipboard(Some(ClipboardImage {
        width: 1,
        height: 1,
        rgba: vec![1, 2, 3, 4],
    }));
    assert_eq!(
        clipboard.read_image().expect("image exists").rgba(),
        &[1, 2, 3, 4]
    );
    assert!(matches!(
        clipboard.read_image(),
        Err(ClipboardError::NoImage)
    ));
}

fn insert_orientation_six(jpeg: &mut Vec<u8>) {
    let exif = [
        b'E', b'x', b'i', b'f', 0, 0, b'I', b'I', 42, 0, 8, 0, 0, 0, 1, 0, 0x12, 0x01, 3, 0, 1, 0,
        0, 0, 6, 0, 0, 0, 0, 0, 0, 0,
    ];
    let segment_length = u16::try_from(exif.len() + 2).expect("small EXIF fixture");
    let mut segment = vec![0xff, 0xe1];
    segment.extend_from_slice(&segment_length.to_be_bytes());
    segment.extend_from_slice(&exif);
    jpeg.splice(2..2, segment);
}

fn png_header(width: u32, height: u32) -> Vec<u8> {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut chunk = b"IHDR".to_vec();
    chunk.extend_from_slice(&width.to_be_bytes());
    chunk.extend_from_slice(&height.to_be_bytes());
    chunk.extend_from_slice(&[8, 6, 0, 0, 0]);
    png.extend_from_slice(&13_u32.to_be_bytes());
    png.extend_from_slice(&chunk);
    png.extend_from_slice(&crc32(&chunk).to_be_bytes());
    png
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}
