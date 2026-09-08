//! Desktop application entry point.

mod app;
mod theme;

use app::DesktopApp;

const WINDOW_TITLE: &str = "Dithering";

pub fn run() -> eframe::Result {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([960.0, 640.0])
            .with_min_inner_size([640.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        WINDOW_TITLE,
        options,
        Box::new(|context| {
            theme::apply(&context.egui_ctx);
            Ok(Box::new(DesktopApp))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::WINDOW_TITLE;

    #[test]
    fn window_has_a_temporary_title() {
        assert!(!WINDOW_TITLE.is_empty());
    }
}
