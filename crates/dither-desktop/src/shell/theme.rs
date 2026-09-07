use eframe::egui;

pub fn apply(context: &egui::Context) {
    context.set_visuals(egui::Visuals::dark());
}
