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
            .with_inner_size([920.0, 620.0])
            .with_min_inner_size([520.0, 380.0])
            .with_icon(icon),
        ..Default::default()
    };
    eframe::run_native(
        "FactureCraft",
        options,
        Box::new(move |creation| {
            let mut visuals = egui::Visuals::dark();
            visuals.panel_fill = egui::Color32::from_rgb(24, 20, 28);
            visuals.selection.bg_fill = egui::Color32::from_rgb(116, 52, 120);
            creation.egui_ctx.set_visuals(visuals);
            creation.egui_ctx.style_mut(|style| {
                style.spacing.item_spacing = egui::vec2(10.0, 10.0);
                style.spacing.button_padding = egui::vec2(14.0, 8.0);
            });
            let mut app = Drafts {
                store,
                items: Vec::new(),
                error: None,
                language,
                title: String::new(),
                currency: "EUR".into(),
                document_language: language,
                composing: false,
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
    title: String,
    currency: String,
    document_language: Language,
    composing: bool,
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
        let fr = matches!(self.language, Language::Fr);
        let t = |f, e| if fr { f } else { e };
        let accent = egui::Color32::from_rgb(198, 129, 207);
        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(&ctx.style()).inner_margin(28))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("FactureCraft")
                            .size(17.0)
                            .strong()
                            .color(accent),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(t("Actualiser", "Refresh")).clicked() {
                            self.refresh();
                        }
                    });
                });
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(t("Devis", "Quotes"))
                            .size(36.0)
                            .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(t("+ Nouveau devis", "+ New quote"))
                                    .fill(egui::Color32::from_rgb(116, 52, 120)),
                            )
                            .clicked()
                        {
                            self.composing = true;
                        }
                    });
                });
                ui.weak(format!(
                    "{} {}",
                    self.items.len(),
                    t("brouillon(s)", "draft(s)")
                ));
                ui.add_space(12.0);

                if self.composing {
                    egui::Frame::group(ui.style())
                        .inner_margin(16)
                        .show(ui, |ui| {
                            ui.label(t("Titre du devis", "Quote title"));
                            ui.add(
                                egui::TextEdit::singleline(&mut self.title)
                                    .desired_width(f32::INFINITY),
                            );
                            ui.horizontal_wrapped(|ui| {
                                ui.label(t("Devise", "Currency"));
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.currency)
                                        .desired_width(56.0)
                                        .char_limit(3),
                                );
                                egui::ComboBox::from_label(t(
                                    "Langue du document",
                                    "Document language",
                                ))
                                .selected_text(self.document_language.code().to_uppercase())
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut self.document_language,
                                        Language::Fr,
                                        "Français",
                                    );
                                    ui.selectable_value(
                                        &mut self.document_language,
                                        Language::En,
                                        "English",
                                    );
                                });
                            });
                            ui.horizontal(|ui| {
                                if ui
                                    .button(t("Enregistrer le brouillon", "Save draft"))
                                    .clicked()
                                {
                                    match self.store.create(
                                        &self.title,
                                        &self.currency,
                                        self.document_language.code(),
                                    ) {
                                        Ok(_) => {
                                            self.title.clear();
                                            self.composing = false;
                                            self.refresh();
                                        }
                                        Err(error) => self.error = Some(error.to_string()),
                                    }
                                }
                                if ui.button(t("Annuler", "Cancel")).clicked() {
                                    self.composing = false;
                                    self.error = None;
                                }
                            });
                        });
                }
                if let Some(error) = &self.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
                if self.items.is_empty() {
                    ui.weak(t("Aucun devis enregistré.", "No saved quotes."));
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for draft in &self.items {
                        egui::Frame::group(ui.style())
                            .fill(egui::Color32::from_rgb(38, 31, 43))
                            .inner_margin(16)
                            .corner_radius(10)
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                ui.label(egui::RichText::new(&draft.title).size(17.0).strong());
                                ui.weak(format!(
                                    "#{}  ·  {}  ·  {}",
                                    draft.id,
                                    draft.currency,
                                    draft.language.to_uppercase()
                                ));
                            });
                    }
                });
            });
    }
}
