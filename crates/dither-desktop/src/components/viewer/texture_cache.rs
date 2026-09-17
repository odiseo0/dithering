use std::collections::HashMap;

use dither_engine::Raster;
use eframe::egui;

const MAX_TILE_SIDE: usize = 1024;
const MAX_CACHE_BYTES: usize = 128 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ImageVariant {
    Original,
    Result,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ViewImageId {
    pub document: u64,
    pub revision: u64,
    pub variant: ImageVariant,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct TileKey {
    image: ViewImageId,
    x: u32,
    y: u32,
}

struct CacheEntry {
    texture: egui::TextureHandle,
    bytes: usize,
    last_used: u64,
}

pub(crate) struct TextureCache {
    reduced: HashMap<ViewImageId, CacheEntry>,
    tiles: HashMap<TileKey, CacheEntry>,
    bytes: usize,
    frame: u64,
    max_bytes: usize,
}

impl TextureCache {
    pub fn new() -> Self {
        Self {
            reduced: HashMap::new(),
            tiles: HashMap::new(),
            bytes: 0,
            frame: 0,
            max_bytes: MAX_CACHE_BYTES,
        }
    }

    pub fn begin_frame(&mut self) {
        self.frame = self.frame.saturating_add(1);
    }

    pub fn retain_document(&mut self, document: u64) {
        self.reduced.retain(|key, entry| {
            let keep = key.document == document;
            if !keep {
                self.bytes = self.bytes.saturating_sub(entry.bytes);
            }
            keep
        });

        self.tiles.retain(|key, entry| {
            let keep = key.image.document == document;

            if !keep {
                self.bytes = self.bytes.saturating_sub(entry.bytes);
            }

            keep
        });
    }

    pub fn reduced(
        &mut self,
        context: &egui::Context,
        id: ViewImageId,
        raster: &Raster,
        max_texture_side: usize,
    ) -> egui::TextureId {
        self.reduced.retain(|key, entry| {
            let superseded = key.document == id.document
                && key.variant == id.variant
                && key.revision != id.revision;

            if superseded {
                self.bytes = self.bytes.saturating_sub(entry.bytes);
            }

            !superseded
        });

        let entry = self.reduced.entry(id).or_insert_with(|| {
            let image = reduced_image(raster, max_texture_side.max(1));
            let bytes = image.pixels.len().saturating_mul(4);
            self.bytes = self.bytes.saturating_add(bytes);
            CacheEntry {
                texture: context.load_texture(
                    format!("reduced-{id:?}"),
                    image,
                    egui::TextureOptions::LINEAR,
                ),
                bytes,
                last_used: self.frame,
            }
        });
        entry.last_used = self.frame;
        entry.texture.id()
    }

    pub fn tile(
        &mut self,
        context: &egui::Context,
        id: ViewImageId,
        raster: &Raster,
        x: u32,
        y: u32,
        max_texture_side: usize,
    ) -> egui::TextureId {
        let key = TileKey { image: id, x, y };
        let tile_side = MAX_TILE_SIDE.min(max_texture_side.max(1));
        let entry = self.tiles.entry(key).or_insert_with(|| {
            let image = tile_image(raster, x, y, tile_side);
            let bytes = image.pixels.len().saturating_mul(4);
            self.bytes = self.bytes.saturating_add(bytes);
            CacheEntry {
                texture: context.load_texture(
                    format!("tile-{id:?}-{x}-{y}"),
                    image,
                    egui::TextureOptions::NEAREST,
                ),
                bytes,
                last_used: self.frame,
            }
        });
        entry.last_used = self.frame;
        let texture = entry.texture.id();
        self.evict_old_tiles();
        texture
    }

    #[must_use]
    pub const fn tile_side(max_texture_side: usize) -> usize {
        if max_texture_side == 0 {
            1
        } else if max_texture_side < MAX_TILE_SIDE {
            max_texture_side
        } else {
            MAX_TILE_SIDE
        }
    }

    fn evict_old_tiles(&mut self) {
        while self.bytes > self.max_bytes {
            let Some(oldest) = self
                .tiles
                .iter()
                .filter(|(_, entry)| entry.last_used != self.frame)
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| *key)
            else {
                break;
            };
            if let Some(entry) = self.tiles.remove(&oldest) {
                self.bytes = self.bytes.saturating_sub(entry.bytes);
            }
        }
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn reduced_image(raster: &Raster, limit: usize) -> egui::ColorImage {
    let width = usize::try_from(raster.width()).unwrap_or(1);
    let height = usize::try_from(raster.height()).unwrap_or(1);
    let (output_width, output_height) = if width <= limit && height <= limit {
        (width, height)
    } else if width >= height {
        (limit, height.saturating_mul(limit).div_ceil(width).max(1))
    } else {
        (width.saturating_mul(limit).div_ceil(height).max(1), limit)
    };
    let mut rgba = Vec::with_capacity(output_width.saturating_mul(output_height).saturating_mul(4));
    for output_y in 0..output_height {
        for output_x in 0..output_width {
            rgba.extend_from_slice(&bilinear_sample(
                raster.rgba(),
                width,
                height,
                output_x,
                output_y,
                output_width,
                output_height,
            ));
        }
    }
    egui::ColorImage::from_rgba_unmultiplied([output_width, output_height], &rgba)
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::too_many_arguments
)]
fn bilinear_sample(
    rgba: &[u8],
    width: usize,
    height: usize,
    output_x: usize,
    output_y: usize,
    output_width: usize,
    output_height: usize,
) -> [u8; 4] {
    let source_x = ((output_x as f32 + 0.5) * width as f32 / output_width as f32 - 0.5)
        .clamp(0.0, (width - 1) as f32);
    let source_y = ((output_y as f32 + 0.5) * height as f32 / output_height as f32 - 0.5)
        .clamp(0.0, (height - 1) as f32);
    let x0 = source_x.floor() as usize;
    let y0 = source_y.floor() as usize;
    let x1 = (x0 + 1).min(width - 1);
    let y1 = (y0 + 1).min(height - 1);
    let tx = source_x - x0 as f32;
    let ty = source_y - y0 as f32;
    let samples = [
        (rgba_at(rgba, width, x0, y0), (1.0 - tx) * (1.0 - ty)),
        (rgba_at(rgba, width, x1, y0), tx * (1.0 - ty)),
        (rgba_at(rgba, width, x0, y1), (1.0 - tx) * ty),
        (rgba_at(rgba, width, x1, y1), tx * ty),
    ];
    let mut output = [0; 4];
    let alpha = samples
        .iter()
        .map(|(sample, weight)| f32::from(sample[3]) * weight)
        .sum::<f32>();
    output[3] = alpha.round().clamp(0.0, 255.0) as u8;
    if alpha > f32::EPSILON {
        for channel in 0..3 {
            let premultiplied = samples
                .iter()
                .map(|(sample, weight)| f32::from(sample[channel]) * f32::from(sample[3]) * weight)
                .sum::<f32>();
            output[channel] = (premultiplied / alpha).round().clamp(0.0, 255.0) as u8;
        }
    }
    output
}

fn rgba_at(rgba: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
    let offset = (y * width + x) * 4;
    [
        rgba[offset],
        rgba[offset + 1],
        rgba[offset + 2],
        rgba[offset + 3],
    ]
}

fn tile_image(raster: &Raster, tile_x: u32, tile_y: u32, tile_side: usize) -> egui::ColorImage {
    let width = usize::try_from(raster.width()).unwrap_or(1);
    let height = usize::try_from(raster.height()).unwrap_or(1);
    let start_x = usize::try_from(tile_x)
        .unwrap_or(0)
        .saturating_mul(tile_side);
    let start_y = usize::try_from(tile_y)
        .unwrap_or(0)
        .saturating_mul(tile_side);
    let tile_width = tile_side.min(width.saturating_sub(start_x));
    let tile_height = tile_side.min(height.saturating_sub(start_y));
    let mut rgba = Vec::with_capacity(tile_width.saturating_mul(tile_height).saturating_mul(4));
    for y in start_y..start_y + tile_height {
        let start = (y.saturating_mul(width).saturating_add(start_x)).saturating_mul(4);
        let end = start.saturating_add(tile_width.saturating_mul(4));
        rgba.extend_from_slice(&raster.rgba()[start..end]);
    }
    egui::ColorImage::from_rgba_unmultiplied([tile_width, tile_height], &rgba)
}

#[cfg(test)]
mod tests {
    use dither_engine::Raster;

    use super::{ImageVariant, TextureCache, ViewImageId, reduced_image, tile_image};

    #[test]
    fn reduced_image_stays_inside_the_texture_limit() {
        let raster = Raster::new(8, 4, vec![255; 8 * 4 * 4]).expect("valid fixture");
        let image = reduced_image(&raster, 3);
        assert!(image.width() <= 3);
        assert!(image.height() <= 3);
        assert_eq!((image.width(), image.height()), (3, 2));
    }

    #[test]
    fn edge_tile_keeps_its_partial_dimensions_and_pixels() {
        let mut rgba = Vec::new();
        for value in 0_u8..15 {
            rgba.extend_from_slice(&[value, 0, 0, 255]);
        }
        let raster = Raster::new(5, 3, rgba).expect("valid fixture");
        let tile = tile_image(&raster, 1, 1, 2);
        assert_eq!((tile.width(), tile.height()), (2, 1));
        assert_eq!(tile.pixels[0].r(), 12);
        assert_eq!(tile.pixels[1].r(), 13);
    }

    #[test]
    fn reduced_filter_does_not_mix_invisible_rgb_into_visible_color() {
        let raster = Raster::new(2, 1, vec![255, 0, 0, 0, 0, 0, 255, 255]).expect("valid fixture");
        let image = reduced_image(&raster, 1);
        assert_eq!(image.pixels[0].to_srgba_unmultiplied(), [0, 0, 255, 128]);
    }

    #[test]
    fn tile_cache_evicts_old_entries_when_the_limit_is_reached() {
        let context = eframe::egui::Context::default();
        let raster = Raster::new(8, 2, vec![255; 8 * 2 * 4]).expect("valid fixture");
        let id = ViewImageId {
            document: 1,
            revision: 1,
            variant: ImageVariant::Result,
        };
        let mut cache = TextureCache::new();
        cache.max_bytes = 32;
        for x in 0..4 {
            cache.begin_frame();
            cache.tile(&context, id, &raster, x, 0, 2);
        }
        assert!(cache.bytes <= cache.max_bytes);
        assert!(cache.tiles.len() <= 2);
    }

    #[test]
    fn long_texture_churn_stays_bounded() {
        let context = eframe::egui::Context::default();
        let raster = Raster::new(16, 16, vec![255; 16 * 16 * 4]).expect("valid fixture");
        let mut cache = TextureCache::new();
        cache.max_bytes = 128;

        for revision in 0..10_000 {
            cache.begin_frame();
            cache.tile(
                &context,
                ViewImageId {
                    document: 1,
                    revision,
                    variant: ImageVariant::Result,
                },
                &raster,
                0,
                0,
                4,
            );
            assert!(cache.bytes <= cache.max_bytes);
            assert!(cache.tiles.len() <= 2);
        }
    }
}
