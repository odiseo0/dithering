use dither_engine::Raster;
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Vec2, pos2, vec2};

use super::{
    model::{ViewerState, ZoomMode},
    texture_cache::{TextureCache, ViewImageId},
};

const CHECKER_SIDE: f32 = 10.0;
const TILE_ZOOM_THRESHOLD: f32 = 0.5;

pub struct Viewer {
    state: ViewerState,
    textures: TextureCache,
    last_viewport: Vec2,
}

impl Viewer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: ViewerState::new(),
            textures: TextureCache::new(),
            last_viewport: Vec2::ZERO,
        }
    }

    pub fn reset_for_image(&mut self, document: u64, raster: &Raster) {
        self.state.reset_for_image(image_size(raster));
        self.textures.retain_document(document);
    }

    pub fn fit(&mut self) {
        self.state = ViewerState::new();
    }

    pub fn actual_size(&mut self, raster: &Raster) {
        self.state
            .actual_size(self.last_viewport, image_size(raster));
    }

    #[must_use]
    pub fn zoom_percent(&self) -> f32 {
        self.state.zoom() * 100.0
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        image_id: ViewImageId,
        raster: &Raster,
    ) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        self.last_viewport = rect.size();
        let pixels_per_point = ui.ctx().pixels_per_point();
        let size = image_size(raster);
        self.state.update_fit(rect.size(), size, pixels_per_point);
        self.process_input(ui, &response, rect, size, pixels_per_point);
        self.textures.begin_frame();

        let point_scale = self.state.point_scale(pixels_per_point);
        let image_origin = rect.center() - self.state.center().to_vec2() * point_scale;
        let image_rect = Rect::from_min_size(image_origin, size * point_scale);
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::from_gray(24));
        paint_checkerboard(&painter, rect.intersect(image_rect));

        let max_texture_side = ui.input(|input| input.max_texture_side).max(1);

        if self.state.mode() == ZoomMode::Fit || self.state.zoom() < TILE_ZOOM_THRESHOLD {
            let texture = self
                .textures
                .reduced(ui.ctx(), image_id, raster, max_texture_side);
            painter.image(
                texture,
                image_rect,
                Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            self.paint_tiles(
                ui.ctx(),
                &painter,
                rect,
                image_rect,
                image_id,
                raster,
                max_texture_side,
                point_scale,
            );
        }

        response
    }

    fn process_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: Rect,
        image_size: Vec2,
        pixels_per_point: f32,
    ) {
        if !response.hovered() && !response.dragged() {
            return;
        }

        let (zoom_delta, scroll, modifiers, pointer) = ui.input(|input| {
            (
                input.zoom_delta(),
                input.smooth_scroll_delta,
                input.modifiers,
                input.pointer.hover_pos(),
            )
        });

        if modifiers.ctrl && (zoom_delta - 1.0).abs() > f32::EPSILON {
            let pointer_offset = pointer.map_or(Vec2::ZERO, |position| position - rect.center());
            self.state.zoom_at(
                zoom_delta,
                pointer_offset,
                rect.size(),
                image_size,
                pixels_per_point,
            );
        } else if !modifiers.ctrl && scroll != Vec2::ZERO {
            self.state
                .pan(scroll, rect.size(), image_size, pixels_per_point);
        }

        let space_held = ui.input(|input| input.key_down(egui::Key::Space));

        if response.dragged_by(egui::PointerButton::Middle)
            || (space_held && response.dragged_by(egui::PointerButton::Primary))
        {
            self.state.pan(
                response.drag_delta(),
                rect.size(),
                image_size,
                pixels_per_point,
            );
        }
    }

    #[allow(clippy::cast_precision_loss, clippy::too_many_arguments)]
    fn paint_tiles(
        &mut self,
        context: &egui::Context,
        painter: &egui::Painter,
        viewport: Rect,
        image_rect: Rect,
        image_id: ViewImageId,
        raster: &Raster,
        max_texture_side: usize,
        point_scale: f32,
    ) {
        let visible = viewport.intersect(image_rect);
        if !visible.is_positive() {
            return;
        }
        let tile_side = TextureCache::tile_side(max_texture_side).max(1);
        let image_width = usize::try_from(raster.width()).unwrap_or(1);
        let image_height = usize::try_from(raster.height()).unwrap_or(1);
        let columns = image_width.div_ceil(tile_side);
        let rows = image_height.div_ceil(tile_side);
        let min_image = (visible.min - image_rect.min) / point_scale;
        let max_image = (visible.max - image_rect.min) / point_scale;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let first_x = ((min_image.x.max(0.0) as usize) / tile_side).saturating_sub(1);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let first_y = ((min_image.y.max(0.0) as usize) / tile_side).saturating_sub(1);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let last_x = ((max_image.x.max(0.0) as usize) / tile_side + 1).min(columns - 1);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let last_y = ((max_image.y.max(0.0) as usize) / tile_side + 1).min(rows - 1);

        for tile_y in first_y..=last_y {
            for tile_x in first_x..=last_x {
                let width = tile_side.min(image_width - tile_x * tile_side);
                let height = tile_side.min(image_height - tile_y * tile_side);
                let min = image_rect.min
                    + vec2(
                        (tile_x * tile_side) as f32 * point_scale,
                        (tile_y * tile_side) as f32 * point_scale,
                    );
                let tile_rect = Rect::from_min_size(
                    min,
                    vec2(width as f32 * point_scale, height as f32 * point_scale),
                );
                let texture = self.textures.tile(
                    context,
                    image_id,
                    raster,
                    u32::try_from(tile_x).unwrap_or(u32::MAX),
                    u32::try_from(tile_y).unwrap_or(u32::MAX),
                    max_texture_side,
                );
                painter.image(
                    texture,
                    tile_rect,
                    Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
        }
    }
}

impl Default for Viewer {
    fn default() -> Self {
        Self::new()
    }
}

fn image_size(raster: &Raster) -> Vec2 {
    let width = u16::try_from(raster.width()).unwrap_or(u16::MAX);
    let height = u16::try_from(raster.height()).unwrap_or(u16::MAX);
    vec2(f32::from(width), f32::from(height))
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn paint_checkerboard(painter: &egui::Painter, visible: Rect) {
    if !visible.is_positive() {
        return;
    }

    let first_x = (visible.left() / CHECKER_SIDE).floor() as i32;
    let last_x = (visible.right() / CHECKER_SIDE).ceil() as i32;
    let first_y = (visible.top() / CHECKER_SIDE).floor() as i32;
    let last_y = (visible.bottom() / CHECKER_SIDE).ceil() as i32;

    for y in first_y..last_y {
        for x in first_x..last_x {
            let color = if (x + y) % 2 == 0 {
                Color32::from_gray(92)
            } else {
                Color32::from_gray(132)
            };

            let square = Rect::from_min_size(
                pos2(x as f32 * CHECKER_SIDE, y as f32 * CHECKER_SIDE),
                Vec2::splat(CHECKER_SIDE),
            )
            .intersect(visible);
            painter.rect_filled(square, 0.0, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use dither_engine::Raster;

    use super::Viewer;
    use crate::components::viewer::{ImageVariant, ViewImageId};

    #[test]
    fn a_wide_image_uses_reduced_and_tiled_views_without_one_large_texture() {
        let raster = Raster::new(3_000, 2, vec![255; 3_000 * 2 * 4]).expect("valid fixture");
        let id = ViewImageId {
            document: 1,
            revision: 0,
            variant: ImageVariant::Original,
        };
        let mut viewer = Viewer::new();
        eframe::egui::__run_test_ui(|ui| {
            viewer.reset_for_image(1, &raster);
            viewer.show(ui, id, &raster);
            viewer.actual_size(&raster);
            viewer.show(ui, id, &raster);
        });
    }
}
