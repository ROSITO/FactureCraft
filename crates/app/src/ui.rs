use crate::Language;
use anyhow::Result;
use eframe::egui;
use facturecraft_core::{Draft, Store};

pub fn run(store: Store, language: Language) -> Result<()> {
    let image = image::load_from_memory(include_bytes!("../../../assets/facturecraft-logo.png"))?
        .into_rgba8();
    let (width, height) = image.dimensions();
    let icon = egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([760.0, 520.0])
            .with_min_inner_size([420.0, 300.0])
            .with_icon(icon),
        ..Default::default()
    };
    eframe::run_native(
        "FactureCraft",
        options,
        Box::new(move |_| {
            let mut app = Drafts {
                store,
                items: Vec::new(),
                error: None,
                language,
            };
            app.refresh();
            Ok(Box::new(app))
        }),
    )
    .map_err(|err| anyhow::anyhow!("{err}"))
}

struct Drafts {
    store: Store,
    items: Vec<Draft>,
    error: Option<String>,
    language: Language,
}

impl Drafts {
    fn refresh(&mut self) {
        match self.store.list() {
            Ok(items) => {
                self.items = items;
                self.error = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }
}

impl eframe::App for Drafts {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        let (heading, refresh, empty, error_label) = match self.language {
            Language::Fr => (
                "Brouillons",
                "Actualiser",
                "Aucun brouillon",
                "Impossible de charger les brouillons",
            ),
            Language::En => ("Drafts", "Refresh", "No drafts", "Could not load drafts"),
        };
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.heading(heading);
                if ui.button(refresh).clicked() {
                    self.refresh();
                }
            });
            ui.separator();
            if let Some(error) = &self.error {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    format!("{error_label} — {error}"),
                );
            } else if self.items.is_empty() {
                ui.label(empty);
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                for draft in &self.items {
                    ui.group(|ui| {
                        ui.label(egui::RichText::new(&draft.title).strong());
                        ui.weak(format!(
                            "#{}  ·  {}  ·  {}",
                            draft.id,
                            draft.currency,
                            draft.language.to_uppercase()
                        ));
                    });
                    ui.add_space(6.0);
                }
            });
        });
    }
}
