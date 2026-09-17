use std::{
    path::PathBuf,
    sync::{Arc, mpsc},
    thread,
};

use dither_desktop::components::{
    document::{
        ArboardClipboard, DecodedImage, DocumentState, ImageClipboard, ImageCodec, PngWriter,
        ResultView, SourceFormat, Status, suggested_output_path,
    },
    processing::{
        Dispatch, DocumentId, ProcessingCoordinator, ProcessingEvent, RenderRequest, Revision,
    },
    settings::{ActiveEffect, Settings},
    viewer::{ImageVariant, ViewImageId, Viewer},
};
use dither_engine::{BayerMatrix, DotShape, ErrorDiffusionAlgorithm, Rgb8};
use eframe::egui;

struct LoadOutcome {
    generation: u64,
    path: PathBuf,
    result: Result<DecodedImage, (String, String)>,
}

struct SaveOutcome {
    document_id: DocumentId,
    revision: Revision,
    result: Result<(), String>,
}

enum DroppedSelection {
    None,
    One(PathBuf),
    Multiple(usize),
    MissingPath,
}

#[derive(Clone, Copy, Debug, Default)]
struct SettingsResponse {
    changed: bool,
    immediate: bool,
}

pub struct DesktopApp {
    state: DocumentState,
    settings: Settings,
    keep_settings: bool,
    coordinator: Option<ProcessingCoordinator>,
    next_document_id: u64,
    load_generation: u64,
    load_sender: mpsc::Sender<LoadOutcome>,
    load_receiver: mpsc::Receiver<LoadOutcome>,
    save_sender: mpsc::Sender<SaveOutcome>,
    save_receiver: mpsc::Receiver<SaveOutcome>,
    viewer: Viewer,
    drag_hovered: bool,
}

impl DesktopApp {
    pub fn new(context: &egui::Context) -> Self {
        let (load_sender, load_receiver) = mpsc::channel();
        let (save_sender, save_receiver) = mpsc::channel();
        let coordinator = ProcessingCoordinator::new().ok();
        let mut state = DocumentState::new();
        if coordinator.is_none() {
            state.load_failed(
                "No se pudo iniciar el procesamiento".to_owned(),
                "El sistema no pudo crear el hilo de trabajo".to_owned(),
            );
        }
        context.request_repaint();
        Self {
            state,
            settings: Settings::default(),
            keep_settings: false,
            coordinator,
            next_document_id: 1,
            load_generation: 0,
            load_sender,
            load_receiver,
            save_sender,
            save_receiver,
            viewer: Viewer::new(),
            drag_hovered: false,
        }
    }

    fn open_requested(&mut self, context: &egui::Context) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Abrir imagen")
            .add_filter("Imágenes", &["png", "jpg", "jpeg", "webp"])
            .pick_file()
        else {
            return;
        };
        self.start_path_load(context, path);
    }

    fn start_path_load(&mut self, context: &egui::Context, path: PathBuf) {
        self.load_generation = self.load_generation.saturating_add(1);
        let generation = self.load_generation;
        if let Some(coordinator) = self.coordinator.as_mut() {
            coordinator.cancel_current();
        }
        self.state.start_loading();
        let sender = self.load_sender.clone();
        let repaint = context.clone();
        let spawn_result = thread::Builder::new()
            .name("dither-image-loader".to_owned())
            .spawn(move || {
                let result = ImageCodec::load_path(&path)
                    .map_err(|error| (error.user_message().to_owned(), error.to_string()));
                let _ignored = sender.send(LoadOutcome {
                    generation,
                    path,
                    result,
                });
                repaint.request_repaint();
            });
        if let Err(error) = spawn_result {
            self.state
                .load_failed("No se pudo abrir la imagen".to_owned(), error.to_string());
        }
    }

    fn handle_background_events(&mut self, context: &egui::Context) {
        while let Ok(outcome) = self.load_receiver.try_recv() {
            if outcome.generation != self.load_generation {
                continue;
            }
            match outcome.result {
                Ok(decoded) => {
                    let name = outcome.path.file_name().map_or_else(
                        || "Imagen".to_owned(),
                        |name| name.to_string_lossy().into_owned(),
                    );
                    self.finish_load(context, name, Some(outcome.path), decoded);
                }
                Err((message, details)) => self.state.load_failed(message, details),
            }
        }

        let mut processing_events = Vec::new();
        if let Some(coordinator) = self.coordinator.as_mut() {
            while let Some(event) = coordinator.poll_event() {
                processing_events.push(event);
            }
        }
        for event in processing_events {
            let completed = matches!(event, ProcessingEvent::Completed { .. });
            let _accepted = self.state.apply_processing_event(event);
            if completed {
                context.request_repaint();
            }
        }

        while let Ok(outcome) = self.save_receiver.try_recv() {
            let is_current = self.state.document.as_ref().is_some_and(|document| {
                document.id == outcome.document_id && document.revision == outcome.revision
            });
            if !is_current {
                continue;
            }
            match outcome.result {
                Ok(()) => self.state.save_succeeded(),
                Err(details) => self
                    .state
                    .save_failed("No se pudo guardar la imagen".to_owned(), details),
            }
        }

        if matches!(self.state.status, Status::Processing { .. }) {
            context.request_repaint_after(std::time::Duration::from_millis(33));
        }
    }

    fn finish_load(
        &mut self,
        _context: &egui::Context,
        name: String,
        path: Option<PathBuf>,
        decoded: DecodedImage,
    ) {
        if !self.keep_settings {
            self.settings = Settings::default();
        }
        let document_id = DocumentId::new(self.next_document_id);
        self.next_document_id = self.next_document_id.saturating_add(1);
        self.state.load_succeeded(document_id, name, path, decoded);
        if let Some(document) = self.state.document.as_ref() {
            self.viewer
                .reset_for_image(document.id.get(), &document.source);
        }
        self.submit_current(Dispatch::Immediate);
    }

    fn submit_current(&mut self, dispatch: Dispatch) {
        let (Some(coordinator), Some(document)) =
            (self.coordinator.as_mut(), self.state.document.as_ref())
        else {
            return;
        };
        coordinator.submit(
            RenderRequest::new(
                document.id,
                document.revision,
                Arc::clone(&document.source),
                self.settings.effect_config(),
            ),
            dispatch,
        );
    }

    fn settings_changed(&mut self, response: SettingsResponse) {
        if !response.changed || !self.state.settings_changed() {
            return;
        }
        self.submit_current(if response.immediate {
            Dispatch::Immediate
        } else {
            Dispatch::Debounced
        });
    }

    fn save_requested(&mut self, context: &egui::Context) {
        if !self.state.can_save() {
            return;
        }
        let Some(document) = self.state.document.as_ref() else {
            return;
        };
        let suggested = document
            .source_path
            .as_deref()
            .map_or_else(|| PathBuf::from("imagen-dither.png"), suggested_output_path);
        let mut dialog = rfd::FileDialog::new()
            .set_title("Guardar imagen")
            .add_filter("Imagen PNG", &["png"]);
        if let Some(parent) = suggested.parent() {
            dialog = dialog.set_directory(parent);
        }
        if let Some(name) = suggested.file_name() {
            dialog = dialog.set_file_name(name.to_string_lossy());
        }
        let Some(path) = dialog.save_file() else {
            return;
        };
        let Some((revision, raster)) = document.result.as_ref() else {
            return;
        };
        let document_id = document.id;
        let revision = *revision;
        let raster = Arc::clone(raster);
        self.state.save_started();
        let sender = self.save_sender.clone();
        let repaint = context.clone();
        let spawn_result = thread::Builder::new()
            .name("dither-png-writer".to_owned())
            .spawn(move || {
                let result = PngWriter::save(&path, &raster).map_err(|error| error.to_string());
                let _ignored = sender.send(SaveOutcome {
                    document_id,
                    revision,
                    result,
                });
                repaint.request_repaint();
            });
        if let Err(error) = spawn_result {
            self.state
                .save_failed("No se pudo guardar la imagen".to_owned(), error.to_string());
        }
    }

    fn paste_requested(&mut self, context: &egui::Context) {
        let result = ArboardClipboard::new().and_then(|mut clipboard| clipboard.read_image());
        match result {
            Ok(raster) => self.finish_load(
                context,
                "Imagen pegada".to_owned(),
                None,
                DecodedImage {
                    raster,
                    format: SourceFormat::Clipboard,
                },
            ),
            Err(error) => self.state.load_failed(
                "El portapapeles no contiene una imagen".to_owned(),
                error.to_string(),
            ),
        }
    }

    fn handle_drop_and_shortcuts(&mut self, context: &egui::Context) {
        let (hovered_files, dropped_files, paste, open, save, fit, actual) =
            context.input(|input| {
                (
                    input.raw.hovered_files.clone(),
                    input.raw.dropped_files.clone(),
                    input.modifiers.command && input.key_pressed(egui::Key::V),
                    input.modifiers.command && input.key_pressed(egui::Key::O),
                    input.modifiers.command && input.key_pressed(egui::Key::S),
                    input.modifiers.command && input.key_pressed(egui::Key::F),
                    input.modifiers.command && input.key_pressed(egui::Key::Num0),
                )
            });
        self.drag_hovered = hovered_files
            .iter()
            .any(|file| file.path.as_deref().is_some_and(has_supported_extension));
        match dropped_selection(&dropped_files) {
            DroppedSelection::None => {}
            DroppedSelection::One(path) => self.start_path_load(context, path),
            DroppedSelection::Multiple(count) => self.state.load_failed(
                "Abre una imagen a la vez".to_owned(),
                format!("Se recibieron {count} archivos"),
            ),
            DroppedSelection::MissingPath => self.state.load_failed(
                "No se pudo abrir la imagen".to_owned(),
                "El archivo soltado no tiene una ruta local".to_owned(),
            ),
        }
        if !context.egui_wants_keyboard_input() {
            if paste {
                self.paste_requested(context);
            } else if open {
                self.open_requested(context);
            } else if save {
                self.save_requested(context);
            } else if fit {
                self.viewer.fit();
            } else if actual && let Some(raster) = self.state.displayed_raster() {
                self.viewer.actual_size(raster);
            }
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        if ui.button("Abrir").on_hover_text("Ctrl+O").clicked() {
            self.open_requested(ui.ctx());
        }

        if ui.button("Pegar").on_hover_text("Ctrl+V").clicked() {
            self.paste_requested(ui.ctx());
        }

        if ui
            .add_enabled(self.state.can_save(), egui::Button::new("Guardar como"))
            .on_hover_text("Ctrl+S")
            .clicked()
        {
            self.save_requested(ui.ctx());
        }
        ui.separator();
        let has_document = self.state.document.is_some();
        ui.add_enabled_ui(has_document, |ui| {
            ui.selectable_value(&mut self.state.view, ResultView::Original, "Original");
            ui.selectable_value(&mut self.state.view, ResultView::Result, "Resultado");

            if ui.button("Ajustar").on_hover_text("Ctrl+F").clicked() {
                self.viewer.fit();
            }

            if ui.button("100 %").on_hover_text("Ctrl+0").clicked()
                && let Some(raster) = self.state.displayed_raster()
            {
                self.viewer.actual_size(raster);
            }
            ui.label(format!("{:.0} %", self.viewer.zoom_percent()));
        });

        if self.state.result_is_stale() {
            ui.label("Vista previa anterior");
        }
    }

    fn settings_panel(&mut self, ui: &mut egui::Ui) -> SettingsResponse {
        let enabled = self.state.document.is_some()
            && !matches!(self.state.status, Status::Loading | Status::Saving);
        let mut response = SettingsResponse::default();
        ui.add_enabled_ui(enabled, |ui| {
            ui.heading("Efecto");
            egui::ComboBox::from_id_salt("effect")
                .selected_text(effect_name(self.settings.active_effect))
                .show_ui(ui, |ui| {
                    response.changed |= ui
                        .selectable_value(
                            &mut self.settings.active_effect,
                            ActiveEffect::ErrorDiffusion,
                            "Difusión de error",
                        )
                        .changed();
                    response.changed |= ui
                        .selectable_value(
                            &mut self.settings.active_effect,
                            ActiveEffect::Ordered,
                            "Bayer",
                        )
                        .changed();
                    response.changed |= ui
                        .selectable_value(
                            &mut self.settings.active_effect,
                            ActiveEffect::Halftone,
                            "Semitono",
                        )
                        .changed();
                });
            response.immediate |= response.changed;

            match self.settings.active_effect {
                ActiveEffect::ErrorDiffusion => self.diffusion_controls(ui, &mut response),
                ActiveEffect::Ordered => self.ordered_controls(ui, &mut response),
                ActiveEffect::Halftone => self.halftone_controls(ui, &mut response),
            }
        });

        ui.separator();
        ui.checkbox(
            &mut self.keep_settings,
            "Mantener ajustes al abrir otra imagen",
        );
        response
    }

    fn common_controls(&mut self, ui: &mut egui::Ui, response: &mut SettingsResponse) {
        let scale = ui.add(egui::Slider::new(&mut self.settings.scale, 1..=16).text("Escala"));
        response.changed |= scale.changed();
        response.immediate |= scale.drag_stopped();
        ui.separator();
        ui.label("Paleta");
        let can_remove = self.settings.palette.len() > 2;
        let mut remove = None;

        for (index, color) in self.settings.palette.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                let mut rgb = [color.r, color.g, color.b];

                if ui.color_edit_button_srgb(&mut rgb).changed() {
                    *color = Rgb8::new(rgb[0], rgb[1], rgb[2]);
                    response.changed = true;
                }
                ui.monospace(format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b));

                if ui.add_enabled(can_remove, egui::Button::new("−")).clicked() {
                    remove = Some(index);
                }
            });
        }

        if let Some(index) = remove {
            self.settings.remove_palette_color(index);
            response.changed = true;
            response.immediate = true;
        }

        if ui
            .add_enabled(
                self.settings.palette.len() < 8,
                egui::Button::new("Añadir color"),
            )
            .clicked()
        {
            self.settings.add_palette_color();
            response.changed = true;
            response.immediate = true;
        }
    }

    fn diffusion_controls(&mut self, ui: &mut egui::Ui, response: &mut SettingsResponse) {
        egui::ComboBox::from_label("Algoritmo")
            .selected_text(match self.settings.diffusion_algorithm {
                ErrorDiffusionAlgorithm::FloydSteinberg => "Floyd–Steinberg",
                ErrorDiffusionAlgorithm::Atkinson => "Atkinson",
            })
            .show_ui(ui, |ui| {
                response.changed |= ui
                    .selectable_value(
                        &mut self.settings.diffusion_algorithm,
                        ErrorDiffusionAlgorithm::FloydSteinberg,
                        "Floyd–Steinberg",
                    )
                    .changed();
                response.changed |= ui
                    .selectable_value(
                        &mut self.settings.diffusion_algorithm,
                        ErrorDiffusionAlgorithm::Atkinson,
                        "Atkinson",
                    )
                    .changed();
            });
        self.common_controls(ui, response);
    }

    fn ordered_controls(&mut self, ui: &mut egui::Ui, response: &mut SettingsResponse) {
        egui::ComboBox::from_label("Matriz")
            .selected_text(bayer_name(self.settings.bayer_matrix))
            .show_ui(ui, |ui| {
                for (matrix, name) in [
                    (BayerMatrix::Two, "2 × 2"),
                    (BayerMatrix::Four, "4 × 4"),
                    (BayerMatrix::Eight, "8 × 8"),
                ] {
                    response.changed |= ui
                        .selectable_value(&mut self.settings.bayer_matrix, matrix, name)
                        .changed();
                }
            });
        self.common_controls(ui, response);
    }

    fn halftone_controls(&mut self, ui: &mut egui::Ui, response: &mut SettingsResponse) {
        response.changed |= ui
            .radio_value(
                &mut self.settings.halftone_shape,
                DotShape::Circle,
                "Círculo",
            )
            .changed();
        response.changed |= ui
            .radio_value(
                &mut self.settings.halftone_shape,
                DotShape::Square,
                "Cuadrado",
            )
            .changed();

        let cell = ui.add(
            egui::Slider::new(&mut self.settings.halftone_cell_size, 2..=128).text("Cuadrícula"),
        );

        if self.settings.halftone_max_size > self.settings.halftone_cell_size {
            self.settings.halftone_max_size = self.settings.halftone_cell_size;
        }

        let size = ui.add(
            egui::Slider::new(
                &mut self.settings.halftone_max_size,
                1..=self.settings.halftone_cell_size,
            )
            .text("Tamaño máximo"),
        );
        response.changed |= cell.changed() || size.changed();
        response.immediate |= cell.drag_stopped() || size.drag_stopped();
        response.changed |= ui
            .checkbox(&mut self.settings.halftone_inverted, "Invertir")
            .changed();
        response.changed |= color_control(ui, "Fondo", &mut self.settings.halftone_background);
        response.changed |= color_control(ui, "Punto", &mut self.settings.halftone_dot);
    }

    fn central_panel(&mut self, ui: &mut egui::Ui) {
        if let Some(document) = self.state.document.as_ref() {
            let (id, raster) = match self.state.view {
                ResultView::Original => (
                    ViewImageId {
                        document: document.id.get(),
                        revision: 0,
                        variant: ImageVariant::Original,
                    },
                    &document.source,
                ),
                ResultView::Result => document.result.as_ref().map_or_else(
                    || {
                        (
                            ViewImageId {
                                document: document.id.get(),
                                revision: 0,
                                variant: ImageVariant::Original,
                            },
                            &document.source,
                        )
                    },
                    |(revision, raster)| {
                        (
                            ViewImageId {
                                document: document.id.get(),
                                revision: revision.get(),
                                variant: ImageVariant::Result,
                            },
                            raster,
                        )
                    },
                ),
            };
            self.viewer.show(ui, id, raster);
        } else {
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("Abre, arrastra o pega una imagen");
                    ui.label("PNG, JPEG, JPG o WebP estático");
                });
            });
        }

        if self.drag_hovered {
            let rect = ui.max_rect();
            ui.painter().rect_filled(
                rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(35, 110, 190, 80),
            );
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Suelta la imagen para abrirla",
                egui::FontId::proportional(20.0),
                egui::Color32::WHITE,
            );
        }
    }

    fn bottom_bar(&self, ui: &mut egui::Ui) {
        if let Some(document) = &self.state.document {
            ui.label(&document.name);
            ui.separator();
            ui.label(format!(
                "{} × {} · {}",
                document.source.width(),
                document.source.height(),
                document.source_format
            ));
            ui.separator();
        }

        match &self.state.status {
            Status::Empty => ui.label("Sin imagen"),
            Status::Loading => ui.label("Abriendo imagen…"),
            Status::LoadFailed(message)
            | Status::RenderFailed(message)
            | Status::SaveFailed(message) => ui.colored_label(egui::Color32::LIGHT_RED, message),
            Status::Processing { progress } => {
                ui.add(egui::ProgressBar::new(*progress).show_percentage())
            }
            Status::ResultReady => ui.label("Resultado listo"),
            Status::Saving => ui.label("Guardando…"),
        };

        if let Some(details) = &self.state.error_details {
            ui.collapsing("Detalles", |ui| {
                ui.monospace(details);
            });
        }
    }
}

impl eframe::App for DesktopApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_drop_and_shortcuts(ui.ctx());
        self.handle_background_events(ui.ctx());
        egui::Panel::top("top_bar").show(ui, |ui| {
            ui.horizontal(|ui| self.top_bar(ui));
        });
        egui::Panel::bottom("bottom_bar").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| self.bottom_bar(ui));
        });
        egui::Panel::left("settings")
            .resizable(false)
            .default_size(230.0)
            .show(ui, |ui| {
                let response = self.settings_panel(ui);
                self.settings_changed(response);
            });
        egui::CentralPanel::default().show(ui, |ui| self.central_panel(ui));
    }
}

fn color_control(ui: &mut egui::Ui, label: &str, color: &mut Rgb8) -> bool {
    let mut rgb = [color.r, color.g, color.b];
    let changed = ui.horizontal(|ui| {
        ui.label(label);
        let changed = ui.color_edit_button_srgb(&mut rgb).changed();
        ui.monospace(format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]));
        changed
    });

    if changed.inner {
        *color = Rgb8::new(rgb[0], rgb[1], rgb[2]);
    }

    changed.inner
}

const fn effect_name(effect: ActiveEffect) -> &'static str {
    match effect {
        ActiveEffect::ErrorDiffusion => "Difusión de error",
        ActiveEffect::Ordered => "Bayer",
        ActiveEffect::Halftone => "Semitono",
    }
}

const fn bayer_name(matrix: BayerMatrix) -> &'static str {
    match matrix {
        BayerMatrix::Two => "2 × 2",
        BayerMatrix::Four => "4 × 4",
        BayerMatrix::Eight => "8 × 8",
    }
}

fn has_supported_extension(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "webp"
            )
        })
}

fn dropped_selection(files: &[egui::DroppedFile]) -> DroppedSelection {
    match files {
        [] => DroppedSelection::None,
        [file] => file
            .path
            .clone()
            .map_or(DroppedSelection::MissingPath, DroppedSelection::One),
        many => DroppedSelection::Multiple(many.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::{DesktopApp, DroppedSelection, dropped_selection};
    use dither_desktop::components::document::{DecodedImage, SourceFormat};
    use dither_engine::Raster;

    fn decoded() -> DecodedImage {
        DecodedImage {
            raster: Raster::new(1, 1, vec![0, 0, 0, 255]).expect("valid fixture"),
            format: SourceFormat::Png,
        }
    }

    #[test]
    fn keep_settings_starts_disabled() {
        let context = eframe::egui::Context::default();
        let app = DesktopApp::new(&context);
        assert!(!app.keep_settings);
    }

    #[test]
    fn a_successful_load_resets_settings_by_default() {
        let context = eframe::egui::Context::default();
        let mut app = DesktopApp::new(&context);
        app.settings.scale = 4;

        app.finish_load(
            &context,
            "entrada.png".to_owned(),
            Some("entrada.png".into()),
            decoded(),
        );

        assert_eq!(app.settings.scale, 1);
    }

    #[test]
    fn a_successful_load_keeps_settings_when_selected() {
        let context = eframe::egui::Context::default();
        let mut app = DesktopApp::new(&context);
        app.keep_settings = true;
        app.settings.scale = 4;

        app.finish_load(
            &context,
            "entrada.png".to_owned(),
            Some("entrada.png".into()),
            decoded(),
        );

        assert_eq!(app.settings.scale, 4);
    }

    #[test]
    fn drop_accepts_one_local_file_and_rejects_multiple_files() {
        let one = [eframe::egui::DroppedFile {
            path: Some("imagen.png".into()),
            ..Default::default()
        }];
        assert!(matches!(
            dropped_selection(&one),
            DroppedSelection::One(path) if path.ends_with("imagen.png")
        ));
        let two = [one[0].clone(), one[0].clone()];
        assert!(matches!(
            dropped_selection(&two),
            DroppedSelection::Multiple(2)
        ));
    }
}
