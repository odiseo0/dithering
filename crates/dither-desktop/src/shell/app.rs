use eframe::egui;

pub struct DesktopApp;

impl eframe::App for DesktopApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.centered_and_justified(|ui| {
            ui.vertical_centered(|ui| {
                ui.heading("Abre, arrastra o pega una imagen");
                ui.label("PNG, JPEG, JPG o WebP estático");
                ui.small(format!("Motor conectado: {}", dither_engine::CRATE_NAME));
            });
        });
    }
}
