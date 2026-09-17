use eframe::egui::{Pos2, Vec2, pos2};

pub const MIN_ZOOM: f32 = 0.05;
pub const MAX_ZOOM: f32 = 32.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ZoomMode {
    Fit,
    Custom,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewerState {
    zoom: f32,
    center: Pos2,
    mode: ZoomMode,
}

impl ViewerState {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            zoom: 1.0,
            center: Pos2::ZERO,
            mode: ZoomMode::Fit,
        }
    }

    pub fn reset_for_image(&mut self, image_size: Vec2) {
        self.center = pos2(image_size.x * 0.5, image_size.y * 0.5);
        self.mode = ZoomMode::Fit;
    }

    pub fn fit(&mut self, viewport: Vec2, image_size: Vec2, pixels_per_point: f32) {
        if image_size.x <= 0.0 || image_size.y <= 0.0 {
            return;
        }
        self.zoom = ((viewport.x * pixels_per_point / image_size.x)
            .min(viewport.y * pixels_per_point / image_size.y))
        .clamp(MIN_ZOOM, MAX_ZOOM);
        self.center = pos2(image_size.x * 0.5, image_size.y * 0.5);
        self.mode = ZoomMode::Fit;
    }

    pub fn actual_size(&mut self, viewport: Vec2, image_size: Vec2) {
        self.zoom = 1.0;
        self.mode = ZoomMode::Custom;
        self.clamp_center(viewport, image_size, 1.0);
    }

    pub fn zoom_at(
        &mut self,
        factor: f32,
        pointer_from_view_center: Vec2,
        viewport: Vec2,
        image_size: Vec2,
        pixels_per_point: f32,
    ) {
        let old_scale = self.point_scale(pixels_per_point);
        let image_at_pointer = self.center + pointer_from_view_center / old_scale;
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let new_scale = self.point_scale(pixels_per_point);
        self.center = image_at_pointer - pointer_from_view_center / new_scale;
        self.mode = ZoomMode::Custom;
        self.clamp_center(viewport, image_size, pixels_per_point);
    }

    pub fn pan(
        &mut self,
        screen_delta: Vec2,
        viewport: Vec2,
        image_size: Vec2,
        pixels_per_point: f32,
    ) {
        self.center -= screen_delta / self.point_scale(pixels_per_point);
        self.mode = ZoomMode::Custom;
        self.clamp_center(viewport, image_size, pixels_per_point);
    }

    pub fn update_fit(&mut self, viewport: Vec2, image_size: Vec2, pixels_per_point: f32) {
        if self.mode == ZoomMode::Fit {
            self.fit(viewport, image_size, pixels_per_point);
        } else {
            self.clamp_center(viewport, image_size, pixels_per_point);
        }
    }

    #[must_use]
    pub const fn zoom(self) -> f32 {
        self.zoom
    }

    #[must_use]
    pub const fn center(self) -> Pos2 {
        self.center
    }

    #[must_use]
    pub const fn mode(self) -> ZoomMode {
        self.mode
    }

    #[must_use]
    pub fn point_scale(self, pixels_per_point: f32) -> f32 {
        self.zoom / pixels_per_point.max(0.1)
    }

    fn clamp_center(&mut self, viewport: Vec2, image_size: Vec2, pixels_per_point: f32) {
        let scale = self.point_scale(pixels_per_point);
        self.center.x = clamp_axis(self.center.x, viewport.x / scale, image_size.x);
        self.center.y = clamp_axis(self.center.y, viewport.y / scale, image_size.y);
    }
}

impl Default for ViewerState {
    fn default() -> Self {
        Self::new()
    }
}

fn clamp_axis(center: f32, visible: f32, image: f32) -> f32 {
    if visible >= image {
        image * 0.5
    } else {
        let half = visible * 0.5;
        center.clamp(half, image - half)
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::{pos2, vec2};

    use super::{MAX_ZOOM, MIN_ZOOM, ViewerState, ZoomMode};

    #[test]
    fn fit_uses_the_largest_zoom_that_keeps_the_whole_image_visible() {
        let mut state = ViewerState::new();
        state.fit(vec2(500.0, 300.0), vec2(1000.0, 400.0), 2.0);
        assert!((state.zoom() - 1.0).abs() < f32::EPSILON);
        assert_eq!(state.center(), pos2(500.0, 200.0));
        assert_eq!(state.mode(), ZoomMode::Fit);
    }

    #[test]
    fn actual_size_means_one_image_pixel_per_physical_pixel() {
        let mut state = ViewerState::new();
        state.reset_for_image(vec2(1000.0, 1000.0));
        state.actual_size(vec2(200.0, 200.0), vec2(1000.0, 1000.0));
        assert!((state.zoom() - 1.0).abs() < f32::EPSILON);
        assert!((state.point_scale(2.0) - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn zoom_keeps_the_pixel_below_the_pointer_stable() {
        let mut state = ViewerState::new();
        state.reset_for_image(vec2(1000.0, 1000.0));
        state.actual_size(vec2(200.0, 200.0), vec2(1000.0, 1000.0));
        let pointer = vec2(50.0, 20.0);
        let before = state.center() + pointer / state.point_scale(1.0);
        state.zoom_at(2.0, pointer, vec2(200.0, 200.0), vec2(1000.0, 1000.0), 1.0);
        let after = state.center() + pointer / state.point_scale(1.0);
        assert!((before - after).length() < 0.001);
    }

    #[test]
    fn zoom_is_limited() {
        let mut state = ViewerState::new();
        state.reset_for_image(vec2(100.0, 100.0));
        state.zoom_at(
            0.000_1,
            eframe::egui::Vec2::ZERO,
            vec2(10.0, 10.0),
            vec2(100.0, 100.0),
            1.0,
        );
        assert!((state.zoom() - MIN_ZOOM).abs() < f32::EPSILON);
        state.zoom_at(
            1_000_000.0,
            eframe::egui::Vec2::ZERO,
            vec2(10.0, 10.0),
            vec2(100.0, 100.0),
            1.0,
        );
        assert!((state.zoom() - MAX_ZOOM).abs() < f32::EPSILON);
    }
}
