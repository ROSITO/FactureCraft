use crate::Language;
use anyhow::Result;
use eframe::egui;
use facturecraft_core::{Draft, Store};

use crate::brand::{AMBER, CREAM, DEEP_PLUM, INK, LILAC, PLUM, Tone};
use facturecraft_core::{billing::BillingState, products::Product};

pub fn run(store: Store, language: Language) -> Result<()> {
    let image = image::load_from_memory(include_bytes!("../../../assets/facturecraft-logo.png"))?
        .into_rgba8();
    let (width, height) = image.dimensions();
    let mark = egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &image);
    let icon = egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 700.0])
            .with_min_inner_size([650.0, 420.0])
            .with_icon(icon),
        ..Default::default()
    };
    eframe::run_native(
        "FactureCraft",
        options,
        Box::new(move |creation| {
            creation.egui_ctx.set_theme(egui::Theme::Light);
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "Sora".into(),
                egui::FontData::from_static(include_bytes!("../../../assets/fonts/Sora.ttf"))
                    .into(),
            );
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "Sora".into());
            creation.egui_ctx.set_fonts(fonts);
            let logo = creation.egui_ctx.load_texture(
                "facturecraft-mark",
                mark,
                egui::TextureOptions::LINEAR,
            );
            let mut visuals = egui::Visuals::light();
            visuals.panel_fill = CREAM;
            visuals.selection.bg_fill = LILAC;
            // Input fields stay visible on white cards.
            visuals.extreme_bg_color = egui::Color32::from_rgb(250, 246, 250);
            visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, crate::brand::BORDER);
            creation.egui_ctx.set_visuals(visuals);
            creation.egui_ctx.style_mut(|style| {
                style.spacing.item_spacing = egui::vec2(10.0, 10.0);
                style.spacing.button_padding = egui::vec2(14.0, 8.0);
            });
            let mut app = Drafts {
                store,
                logo,
                items: Vec::new(),
                error: None,
                language,
                title: String::new(),
                quote: Default::default(),
                editing: None,
                contacts: Vec::new(),
                currency: "EUR".into(),
                document_language: language,
                composing: false,
                page: Page::Dashboard,
                contact_query: String::new(),
                new_contact: None,
                archive_items: Vec::new(),
                archive_query: String::new(),
                quote_query: String::new(),
                confirm_discard: false,
                close_pending: false,
                notice: None,
                review: None,
                review_version: 0,
                edit_version: None,
                event_date: String::new(),
                payment_amount: String::new(),
                payment_method: String::new(),
                deposit_id: String::new(),
                issuer: Default::default(),
                board: None,
                board_year: 0,
                board_currency: "EUR".into(),
                products: Vec::new(),
                product_edit: None,
                confirm_delete_product: None,
                live_since: None,
                next_numbers: Vec::new(),
                confirm_live: false,
            };
            app.refresh();
            Ok(Box::new(app))
        }),
    )
    .map_err(|err| anyhow::anyhow!("{err}"))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Dashboard,
    Quotes,
    Products,
    Invoices,
    Settings,
    Clients,
    Archive,
}

struct Drafts {
    store: Store,
    logo: egui::TextureHandle,
    items: Vec<Draft>,
    error: Option<String>,
    language: Language,
    title: String,
    quote: facturecraft_core::quote::Quote,
    editing: Option<i64>,
    contacts: Vec<facturecraft_core::contacts::Contact>,
    currency: String,
    document_language: Language,
    composing: bool,
    page: Page,
    contact_query: String,
    new_contact: Option<facturecraft_core::contacts::Contact>,
    archive_items: Vec<facturecraft_core::import::ArchiveItem>,
    archive_query: String,
    quote_query: String,
    confirm_discard: bool,
    close_pending: bool,
    notice: Option<String>,
    review: Option<i64>,
    review_version: i64,
    edit_version: Option<i64>,
    event_date: String,
    payment_amount: String,
    payment_method: String,
    deposit_id: String,
    issuer: facturecraft_core::issuer::Issuer,
    board: Option<facturecraft_core::dashboard::Dashboard>,
    board_year: i32,
    board_currency: String,
    products: Vec<Product>,
    product_edit: Option<Product>,
    confirm_delete_product: Option<i64>,
    live_since: Option<String>,
    next_numbers: Vec<(String, String)>,
    confirm_live: bool,
}

/// Localized status of a document, coloured like the brand kit badges.
fn status(state: &BillingState, fr: bool) -> (&'static str, Tone) {
    let t = |f, e| if fr { f } else { e };
    let outstanding: f64 = state.outstanding.parse().unwrap_or(0.0);
    match (state.kind.as_str(), &state.number) {
        (_, None) => (t("Brouillon", "Draft"), Tone::Draft),
        ("quote", Some(_)) if state.accepted => (t("Accepté", "Accepted"), Tone::Positive),
        ("quote", Some(_)) => (t("Finalisé", "Finalized"), Tone::Sent),
        ("credit", Some(_)) => (t("Avoir émis", "Credit issued"), Tone::Sent),
        (_, Some(_)) if outstanding <= 0.0 => (t("Payée", "Paid"), Tone::Positive),
        (_, Some(_)) if state.paid != "0" => {
            (t("Partiellement payée", "Partly paid"), Tone::Pending)
        }
        (_, Some(_)) => (t("En attente", "Awaiting payment"), Tone::Pending),
    }
}

fn kind_name(kind: &str, fr: bool) -> &'static str {
    match (kind, fr) {
        ("invoice", true) => "Facture",
        ("invoice", false) => "Invoice",
        ("deposit", true) => "Acompte",
        ("deposit", false) => "Deposit",
        ("credit", true) => "Avoir",
        ("credit", false) => "Credit note",
        (_, true) => "Devis",
        _ => "Quote",
    }
}

impl Drafts {
    fn preview(&self, ui: &mut egui::Ui) {
        let french_ui = matches!(self.language, Language::Fr);
        let french_document = matches!(self.document_language, Language::Fr);
        ui.add_space(18.0);
        ui.label(
            egui::RichText::new(if french_ui {
                "Aperçu du brouillon"
            } else {
                "Draft preview"
            })
            .size(18.0)
            .strong(),
        );
        ui.add_space(8.0);
        let ink = INK;
        egui::Frame::new()
            .fill(crate::brand::PAPER)
            .inner_margin(24)
            .corner_radius(8)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.label(
                    egui::RichText::new(if french_document {
                        "DEVIS · BROUILLON"
                    } else {
                        "QUOTE · DRAFT"
                    })
                    .color(ink)
                    .strong(),
                );
                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new(self.title.trim())
                        .size(26.0)
                        .color(ink)
                        .strong(),
                );
                ui.label(egui::RichText::new(self.currency.trim().to_uppercase()).color(ink));
                ui.label(egui::RichText::new(&self.quote.recipient).color(INK));
                for field in [&self.quote.recipient_address, &self.quote.recipient_email] {
                    if !field.is_empty() {
                        ui.label(egui::RichText::new(field).color(INK));
                    }
                }
                for line in &self.quote.lines {
                    if !line.description.is_empty() {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} × {}",
                                line.quantity, line.description
                            ))
                            .color(INK),
                        );
                    }
                }
                if let Ok((net, vat, total)) = self.quote.totals(&self.currency) {
                    ui.separator();
                    let currency = self.currency.trim().to_uppercase();
                    for (label, amount) in [
                        (
                            if french_document {
                                "Total HT"
                            } else {
                                "Subtotal"
                            },
                            net,
                        ),
                        (if french_document { "TVA" } else { "Tax" }, vat),
                        ("Total", total),
                    ] {
                        ui.label(
                            egui::RichText::new(format!("{label}  {amount} {currency}")).color(INK),
                        );
                    }
                }
                if self.quote.totals(&self.currency).is_err() {
                    ui.colored_label(ui.visuals().error_fg_color, if french_ui { "Vérifiez les montants, les descriptions et la devise pour calculer le total." } else { "Check amounts, descriptions and currency to calculate the total." });
                }
                ui.add_space(72.0);
            });
    }

    fn refresh(&mut self) {
        if self.page == Page::Archive {
            match self.store.archive() {
                Ok(items) => {
                    self.archive_items = items;
                    self.error = None;
                }
                Err(error) => self.error = Some(error.to_string()),
            }
            return;
        }
        let loaded = self.store.products().and_then(|products| {
            self.products = products;
            self.live_since = self.store.live_numbering()?;
            self.next_numbers = self.store.next_numbers()?;
            if self.page == Page::Dashboard {
                self.board = Some(
                    self.store
                        .dashboard(self.board_year, &self.board_currency)?,
                );
            }
            Ok(())
        });
        if let Err(error) = loaded {
            self.error = Some(error.to_string());
            return;
        }
        match self.store.contacts() {
            Ok(contacts) => self.contacts = contacts,
            Err(error) => {
                self.error = Some(error.to_string());
                return;
            }
        }
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
        if self.composing && ctx.input(|input| input.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.close_pending = true;
            self.confirm_discard = true;
        }
        let fr = matches!(self.language, Language::Fr);
        let t = |f, e| if fr { f } else { e };
        egui::SidePanel::left("navigation")
            .exact_width(222.0)
            .resizable(false)
            .frame(
                egui::Frame::side_top_panel(&ctx.style())
                    .fill(DEEP_PLUM)
                    .inner_margin(18),
            )
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.image((self.logo.id(), egui::vec2(54.0, 54.0)));
                ui.label(
                    egui::RichText::new("FactureCraft")
                        .size(20.0)
                        .strong()
                        .color(CREAM),
                );
                let (line, _) = ui.allocate_exact_size(egui::vec2(38.0, 3.0), egui::Sense::hover());
                ui.painter().rect_filled(line, 2.0, AMBER);
                ui.add_space(17.0);
                for (page, label) in [
                    (Page::Dashboard, t("📊  Tableau de bord", "📊  Dashboard")),
                    (Page::Quotes, t("🗋  Devis", "🗋  Quotes")),
                    (Page::Invoices, t("🗐  Factures", "🗐  Invoices")),
                    (Page::Clients, t("👤  Clients", "👤  Clients")),
                    (
                        Page::Products,
                        t("📦  Produits & services", "📦  Products & services"),
                    ),
                    (Page::Archive, t("🗄  Archives", "🗄  Archive")),
                    (Page::Settings, t("⚙  Paramètres", "⚙  Settings")),
                ] {
                    if ui
                        .add_enabled_ui(!self.composing, |ui| {
                            ui.add_sized(
                                [186.0, 38.0],
                                egui::Button::new(egui::RichText::new(label).color(CREAM))
                                    .wrap_mode(egui::TextWrapMode::Truncate)
                                    .fill(if self.page == page { PLUM } else { DEEP_PLUM })
                                    .stroke(egui::Stroke::NONE)
                                    .corner_radius(8),
                            )
                        })
                        .inner
                        .clicked()
                    {
                        self.page = page;
                        if page == Page::Settings {
                            match self.store.issuer() {
                                Ok(issuer) => self.issuer = issuer,
                                Err(error) => self.error = Some(error.to_string()),
                            }
                        }
                        self.refresh();
                    }
                }
            });
        if matches!(self.page, Page::Quotes | Page::Invoices)
            && self.composing
            && ctx.content_rect().width() >= 800.0
        {
            egui::SidePanel::right("quote_preview")
                .default_width(340.0)
                .min_width(300.0)
                .show(ctx, |ui| self.preview(ui));
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(&ctx.style()).inner_margin(28))
            .show(ctx, |ui| {
                if self.page == Page::Dashboard {
                    if let Some(error) = &self.error {
                        ui.colored_label(ui.visuals().error_fg_color, error);
                    }
                    let action = self.board.as_ref().and_then(|board| {
                        egui::ScrollArea::vertical()
                            .id_salt("dashboard")
                            .show(ui, |ui| crate::dashboard::show(ui, board, fr))
                            .inner
                    });
                    match action {
                        Some(crate::dashboard::Action::Year(year)) => {
                            self.board_year = year;
                            self.refresh();
                        }
                        Some(crate::dashboard::Action::Currency(currency)) => {
                            self.board_currency = currency;
                            self.refresh();
                        }
                        None => {}
                    }
                    return;
                }
                if self.page == Page::Products {
                    self.products_page(ui, fr);
                    return;
                }
                if self.page == Page::Settings {
                    egui::ScrollArea::vertical().id_salt("settings").show(ui, |ui| {
                        if let Some(notice) = &self.notice { ui.label(notice); }
                        if let Some(error) = &self.error { ui.colored_label(ui.visuals().error_fg_color, error); }
                        ui.heading(t("Identité de l’émetteur", "Issuer details"));
                        ui.label(t("Ces informations figurent sur les documents finalisés. Elles sont figées dans chaque document au moment de sa finalisation.", "These details appear on finalized documents and are frozen into each document when it is finalized."));
                        for (label, field) in [
                            (t("Nom ou société", "Name or company"), &mut self.issuer.name),
                            (t("Adresse", "Address"), &mut self.issuer.address),
                            (t("E-mail", "Email"), &mut self.issuer.email),
                            (t("Identifiant d’immatriculation", "Registration number"), &mut self.issuer.registration),
                            (t("Numéro de TVA", "VAT number"), &mut self.issuer.vat_id),
                            (t("Conditions par défaut", "Default terms"), &mut self.issuer.terms),
                        ] { ui.label(label); ui.add(egui::TextEdit::singleline(field).desired_width(f32::INFINITY)); }
                        if ui.button(t("Enregistrer les paramètres", "Save settings")).clicked() { match self.store.save_issuer(&self.issuer) { Ok(()) => { self.notice = Some(t("Paramètres enregistrés.", "Settings saved.").into()); self.error = None; }, Err(error) => self.error = Some(error.to_string()) } }
                        ui.add_space(18.0);
                        self.numbering_settings(ui, fr);
                    });
                    return;
                }
                if self.page == Page::Clients {
                    ui.horizontal(|ui| {
                        ui.heading(t("Carnet clients", "Client directory"));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(t("+ Nouveau client", "+ New client"))
                                            .color(CREAM),
                                    )
                                    .fill(PLUM),
                                )
                                .clicked()
                            {
                                self.new_contact.get_or_insert_with(Default::default);
                            }
                        });
                    });
                    if let Some(contact) = &mut self.new_contact {
                        let mut close = false;
                        egui::Frame::group(ui.style())
                            .inner_margin(16)
                            .show(ui, |ui| {
                                for (label, field) in [
                                    (t("Nom ou société", "Name or company"), &mut contact.name),
                                    (t("E-mail", "Email"), &mut contact.email),
                                    (t("Adresse", "Address"), &mut contact.address),
                                ] {
                                    ui.label(label);
                                    ui.text_edit_singleline(field);
                                }
                                ui.horizontal(|ui| {
                                    if ui
                                        .button(t("Enregistrer dans le carnet", "Save contact"))
                                        .clicked()
                                    {
                                        match self.store.save_contact(contact) {
                                            Ok(()) => close = true,
                                            Err(error) => self.error = Some(error.to_string()),
                                        }
                                    }
                                    if ui.button(t("Annuler", "Cancel")).clicked() {
                                        close = true;
                                    }
                                });
                            });
                        if close {
                            self.new_contact = None;
                            self.refresh();
                        }
                    }
                    if let Some(error) = &self.error {
                        ui.colored_label(ui.visuals().error_fg_color, error);
                    }
                    ui.add(
                        egui::TextEdit::singleline(&mut self.contact_query).hint_text(t(
                            "Rechercher un client ou une société",
                            "Search clients or companies",
                        )),
                    );
                    let query = self.contact_query.trim().to_lowercase();
                    egui::ScrollArea::vertical()
                        .id_salt("contacts")
                        .show(ui, |ui| {
                            let mut found = false;
                            for contact in &self.contacts {
                                if ![&contact.name, &contact.email, &contact.address]
                                    .iter()
                                    .any(|field| field.to_lowercase().contains(&query))
                                {
                                    continue;
                                }
                                found = true;
                                egui::Frame::group(ui.style())
                                    .fill(egui::Color32::WHITE)
                                    .inner_margin(12)
                                    .corner_radius(8)
                                    .show(ui, |ui| {
                                        ui.set_min_width(ui.available_width());
                                        ui.strong(&contact.name);
                                        for field in [&contact.email, &contact.address] {
                                            if !field.is_empty() {
                                                ui.weak(field);
                                            }
                                        }
                                    });
                            }
                            if !found {
                                ui.weak(t("Aucun contact trouvé.", "No contacts found."));
                            }
                        });
                    return;
                }
                if self.page == Page::Archive {
                    ui.heading(t("Archives Facture.net", "Facture.net archive"));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.archive_query)
                            .hint_text(t("Rechercher", "Search")),
                    );
                    if let Some(error) = &self.error {
                        ui.colored_label(ui.visuals().error_fg_color, error);
                    }
                    if self.archive_items.is_empty() {
                        ui.weak(t("Aucune archive importée.", "No imported archive."));
                    }
                    let query = self.archive_query.trim().to_lowercase();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for item in &self.archive_items {
                            if !format!("{} {} {}", item.label, item.category, item.key)
                                .to_lowercase()
                                .contains(&query)
                            {
                                continue;
                            }
                            ui.horizontal_wrapped(|ui| {
                                ui.strong(&item.label);
                                ui.weak(format!("{} · {}", item.category, item.source));
                                if let Some(link) = &item.record_link {
                                    ui.weak(link);
                                }
                            });
                        }
                    });
                    return;
                }
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(if self.page == Page::Invoices {
                            t("Factures", "Invoices")
                        } else {
                            t("Devis", "Quotes")
                        })
                        .size(36.0)
                        .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_enabled(
                                !self.composing,
                                egui::Button::new(
                                    egui::RichText::new(if self.page == Page::Invoices {
                                        t("+ Nouvelle facture", "+ New invoice")
                                    } else {
                                        t("+ Nouveau devis", "+ New quote")
                                    })
                                    .color(egui::Color32::WHITE)
                                    .strong(),
                                )
                                .fill(PLUM),
                            )
                            .clicked()
                        {
                            self.editing = None;
                            self.edit_version = None;
                            self.title.clear();
                            if self.page == Page::Invoices {
                                match self.store.create_invoice(
                                    t("Nouvelle facture", "New invoice"),
                                    "EUR",
                                    self.language.code(),
                                ) {
                                    Ok(draft) => {
                                        self.editing = Some(draft.id);
                                        self.edit_version = self
                                            .store
                                            .billing_state(draft.id)
                                            .ok()
                                            .map(|s| s.version);
                                        self.title = draft.title;
                                        self.refresh();
                                    }
                                    Err(error) => {
                                        self.error = Some(error.to_string());
                                        return;
                                    }
                                }
                            }
                            self.quote = Default::default();
                            if let Ok(issuer) = self.store.issuer() { self.quote.terms = issuer.terms; }
                            self.currency = "EUR".into();
                            self.document_language = self.language;
                            self.error = None;
                            self.composing = true;
                        }
                    });
                });
                ui.weak(if self.live_since.is_some() {
                    t("Brouillons et documents émis", "Drafts and issued documents")
                } else {
                    t("Brouillons et documents de démonstration", "Drafts and demonstration documents")
                });
                ui.add_space(12.0);

                if self.composing {
                    egui::ScrollArea::vertical()
                        .id_salt("editor")
                        .show(ui, |ui| {
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
                                        .show_ui(
                                            ui,
                                            |ui| {
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
                                            },
                                        );
                                    });
                                    ui.label(t("Destinataire", "Recipient"));
                                    egui::ComboBox::from_id_salt("recipient_book")
                                        .selected_text(t(
                                            "Choisir dans le carnet",
                                            "Choose from contacts",
                                        ))
                                        .show_ui(ui, |ui| {
                                            for contact in &self.contacts {
                                                let label = if contact.email.is_empty() {
                                                    contact.name.clone()
                                                } else {
                                                    format!("{} · {}", contact.name, contact.email)
                                                };
                                                if ui.selectable_label(false, label).clicked() {
                                                    self.quote.recipient.clone_from(&contact.name);
                                                    self.quote
                                                        .recipient_email
                                                        .clone_from(&contact.email);
                                                    self.quote
                                                        .recipient_address
                                                        .clone_from(&contact.address);
                                                }
                                            }
                                        });
                                    for (label, field) in [
                                        (
                                            t("Nom ou société", "Name or company"),
                                            &mut self.quote.recipient,
                                        ),
                                        (t("E-mail", "Email"), &mut self.quote.recipient_email),
                                        (
                                            t("Adresse", "Address"),
                                            &mut self.quote.recipient_address,
                                        ),
                                    ] {
                                        ui.horizontal(|ui| {
                                            ui.label(label);
                                            ui.add(
                                                egui::TextEdit::singleline(field)
                                                    .desired_width(ui.available_width()),
                                            );
                                        });
                                    }
                                    if ui
                                        .button(t("Ajouter au carnet", "Save to contacts"))
                                        .clicked()
                                    {
                                        let contact = facturecraft_core::contacts::Contact {
                                            name: self.quote.recipient.clone(),
                                            email: self.quote.recipient_email.clone(),
                                            address: self.quote.recipient_address.clone(),
                                        };
                                        match self.store.save_contact(&contact) {
                                            Ok(()) => self.refresh(),
                                            Err(error) => self.error = Some(error.to_string()),
                                        }
                                    }
                                    ui.separator();
                                    ui.heading(t("Prestations", "Line items"));
                                    egui::ScrollArea::vertical()
                                        .id_salt("quote_lines")
                                        .max_height(230.0)
                                        .show(ui, |ui| {
                                            let mut remove = None;
                                            for (index, line) in
                                                self.quote.lines.iter_mut().enumerate()
                                            {
                                                ui.push_id(index, |ui| {
                                                    ui.label(t("Description", "Description"));
                                                    ui.text_edit_singleline(&mut line.description);
                                                    ui.horizontal(|ui| {
                                                        ui.label(t("Unité", "Unit"));
                                                        ui.text_edit_singleline(&mut line.unit);
                                                    });
                                                    ui.horizontal(|ui| {
                                                        ui.label(t(
                                                            "Motif d’exonération",
                                                            "Tax exemption reason",
                                                        ));
                                                        ui.text_edit_singleline(
                                                            &mut line.exemption_reason,
                                                        );
                                                    });
                                                    ui.horizontal_wrapped(|ui| {
                                                        for (label, field) in [
                                                            (t("Qté", "Qty"), &mut line.quantity),
                                                            (
                                                                t("Prix HT", "Unit price"),
                                                                &mut line.price,
                                                            ),
                                                            (
                                                                t("Remise %", "Discount %"),
                                                                &mut line.discount,
                                                            ),
                                                            (t("TVA %", "Tax %"), &mut line.vat),
                                                        ] {
                                                            ui.label(label);
                                                            ui.add(
                                                                egui::TextEdit::singleline(field)
                                                                    .desired_width(52.0),
                                                            );
                                                        }
                                                        if ui
                                                            .button(t("Retirer", "Remove"))
                                                            .clicked()
                                                        {
                                                            remove = Some(index);
                                                        }
                                                    });
                                                    ui.separator();
                                                });
                                            }
                                            if let Some(index) = remove {
                                                self.quote.lines.remove(index);
                                            }
                                        });
                                    ui.horizontal(|ui| {
                                        if ui.button(t("+ Ajouter une ligne", "+ Add line")).clicked() {
                                            self.quote.lines.push(Default::default());
                                        }
                                        if !self.products.is_empty() {
                                            egui::ComboBox::from_id_salt("catalogue")
                                                .selected_text(t("+ Depuis le catalogue", "+ From catalogue"))
                                                .show_ui(ui, |ui| {
                                                    for product in &self.products {
                                                        let label = format!("{} · {} {}", product.name, product.price, self.currency.trim().to_uppercase());
                                                        if ui.selectable_label(false, label).clicked() {
                                                            self.quote.lines.push(product.line());
                                                        }
                                                    }
                                                });
                                        }
                                    });
                                    ui.separator();
                                    ui.heading(t("Conditions", "Terms"));
                                    ui.horizontal(|ui| {
                                        ui.label(t("Remise globale %", "Overall discount %"));
                                        ui.text_edit_singleline(&mut self.quote.global_discount);
                                    });
                                    ui.horizontal(|ui| {
                                        ui.label(t("Valable jusqu’au", "Valid until"));
                                        ui.text_edit_singleline(&mut self.quote.valid_until);
                                    });
                                    ui.label(t("Conditions", "Terms"));
                                    ui.text_edit_multiline(&mut self.quote.terms);
                                    ui.horizontal_wrapped(|ui| {
                                        if ui
                                            .button(t("Enregistrer le brouillon", "Save draft"))
                                            .clicked()
                                        {
                                            match if let (Some(id), Some(version)) =
                                                (self.editing, self.edit_version)
                                            {
                                                self.store.save_quote_versioned(
                                                    id,
                                                    version,
                                                    &self.title,
                                                    &self.currency,
                                                    self.document_language.code(),
                                                    &self.quote,
                                                )
                                            } else {
                                                self.store.save_quote(
                                                    None,
                                                    &self.title,
                                                    &self.currency,
                                                    self.document_language.code(),
                                                    &self.quote,
                                                )
                                            } {
                                                Ok(_) => {
                                                    self.title.clear();
                                                    self.notice = Some(
                                                        t("Brouillon enregistré.", "Draft saved.")
                                                            .into(),
                                                    );
                                                    self.composing = false;
                                                    self.refresh();
                                                }
                                                Err(error) => self.error = Some(error.to_string()),
                                            }
                                        }
                                        if ui
                                            .button(t(
                                                "Exporter le PDF brouillon",
                                                "Export draft PDF",
                                            ))
                                            .clicked()
                                        {
                                            let kind = self.editing.and_then(|id| self.store.billing_state(id).ok())
                                                .map(|state| state.kind)
                                                .unwrap_or_else(|| if self.page == Page::Invoices { "invoice".into() } else { "quote".into() });
                                            let language = self.document_language.code();
                                            let result = self.store.issuer().and_then(|issuer| crate::pdf::render(
                                                &crate::pdf::draft_label(&kind, language),
                                                &self.title,
                                                &self.currency,
                                                self.document_language.code(),
                                                &self.quote,
                                                crate::pdf::DocumentContext { issuer: Some(&issuer), ..Default::default() },
                                            ));
                                            match result {
                                                Ok(bytes) => {
                                                    if let Some(path) = rfd::FileDialog::new()
                                                        .add_filter("PDF", &["pdf"])
                                                        .set_file_name("devis-brouillon.pdf")
                                                        .save_file()
                                                    {
                                                        match crate::operations::write_file(&path, &bytes) {
                                                            Ok(()) => {
                                                                self.notice = Some(
                                                                    t(
                                                                        "PDF brouillon exporté.",
                                                                        "Draft PDF exported.",
                                                                    )
                                                                    .into(),
                                                                )
                                                            }
                                                            Err(error) => {
                                                                self.error = Some(error.to_string())
                                                            }
                                                        }
                                                    }
                                                }
                                                Err(error) => self.error = Some(error.to_string()),
                                            }
                                        }
                                        if ui.button(t("Annuler", "Cancel")).clicked() {
                                            self.confirm_discard = true;
                                        }
                                    });
                                });
                        });
                }
                if let Some(notice) = &self.notice {
                    ui.label(notice);
                }
                if let Some(error) = &self.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
                if self.items.is_empty() && !self.composing && self.page == Page::Quotes {
                    egui::Frame::group(ui.style())
                        .fill(egui::Color32::WHITE)
                        .inner_margin(32)
                        .corner_radius(12)
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.heading(t("Créez votre premier devis", "Create your first quote"));
                            ui.label(t(
                                "Choisissez le titre, la devise et la langue du document.",
                                "Choose a title, currency, and document language.",
                            ));
                            if ui
                                .button(t("Commencer un devis", "Start a quote"))
                                .clicked()
                            {
                                self.editing = None;
                                self.title.clear();
                                self.quote = Default::default();
                                self.error = None;
                                self.composing = true;
                            }
                            if ui
                                .link(t(
                                    "Consulter les archives Facture.net",
                                    "Browse the Facture.net archive",
                                ))
                                .clicked()
                            {
                                self.page = Page::Archive;
                                self.refresh();
                            }
                        });
                }
                if matches!(self.page, Page::Quotes | Page::Invoices)
                    && self.composing
                    && ctx.content_rect().width() < 800.0
                {
                    ui.collapsing(t("Aperçu du brouillon", "Draft preview"), |ui| {
                        self.preview(ui)
                    });
                }
                let mut selected = None;
                if !self.composing {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.quote_query)
                            .hint_text(t("Rechercher un document", "Search documents")),
                    );
                    let query = self.quote_query.trim().to_lowercase();
                    let mut found = false;
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for draft in &self.items {
                            let Ok(state) = self.store.billing_state(draft.id) else {
                                continue;
                            };
                            if (self.page == Page::Quotes) != (state.kind == "quote") {
                                continue;
                            }
                            if !draft.title.to_lowercase().contains(&query)
                                && !draft.id.to_string().contains(&query)
                            {
                                continue;
                            }
                            found = true;
                            egui::Frame::group(ui.style())
                                .fill(egui::Color32::WHITE)
                                .inner_margin(16)
                                .corner_radius(10)
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new(&draft.title).size(17.0).strong());
                                        let (label, tone) = status(&state, fr);
                                        crate::brand::badge(ui, label, tone);
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if let Ok((_, _, gross)) = self.store.quote(draft.id).and_then(|q| q.totals(&draft.currency)) {
                                                ui.label(egui::RichText::new(crate::brand::money(&gross, &draft.currency, fr)).size(16.0).strong().color(INK));
                                            }
                                        });
                                    });
                                    ui.weak(format!(
                                        "{} {}  ·  {}  ·  {}",
                                        kind_name(&state.kind, fr),
                                        state.number.as_deref().unwrap_or(&format!("#{}", draft.id)),
                                        draft.currency,
                                        draft.language.to_uppercase()
                                    ));
                                    ui.horizontal(|ui| {
                                        if state.number.is_none()
                                            && ui.button(t("Modifier", "Edit")).clicked()
                                        {
                                            selected = Some(draft.id);
                                        }
                                        if ui
                                            .button(t("Voir et agir", "Review and actions"))
                                            .clicked()
                                        {
                                            self.review = Some(draft.id);
                                            self.review_version = state.version;
                                        }
                                    });
                                });
                        }
                        if !found && !self.items.is_empty() {
                            ui.weak(t("Aucun document trouvé.", "No documents found."));
                        }
                    });
                }
                if let Some(id) = selected {
                    match (self.store.get(id), self.store.quote(id)) {
                        (Ok(Some(draft)), Ok(quote)) => {
                            self.editing = Some(id);
                            self.edit_version =
                                self.store.billing_state(id).ok().map(|state| state.version);
                            self.title = draft.title;
                            self.currency = draft.currency;
                            self.document_language = if draft.language == "en" {
                                Language::En
                            } else {
                                Language::Fr
                            };
                            self.quote = quote;
                            self.composing = true;
                            self.error = None;
                        }
                        _ => {
                            self.error = Some(
                                t("Impossible de charger le devis.", "Could not load quote.")
                                    .into(),
                            )
                        }
                    }
                }
            });
        if let Some(id) = self.review {
            if let (Ok(Some(draft)), Ok(quote), Ok(state)) = (
                self.store.get(id),
                self.store.quote(id),
                self.store.billing_state(id),
            ) {
                egui::Window::new(t("Vérifier le document", "Review document")).collapsible(false).resizable(true).show(ctx, |ui| {
                    ui.heading(&draft.title);
                    ui.horizontal(|ui| {
                        ui.label(format!("{} · {}", kind_name(&state.kind, fr), state.number.as_deref().unwrap_or(t("Brouillon", "Draft"))));
                        let (label, tone) = status(&state, fr);
                        crate::brand::badge(ui, label, tone);
                    });
                    ui.label(format!("{}: {}", t("Destinataire", "Recipient"), quote.recipient));
                    if let Ok((net, vat, total)) = quote.totals(&draft.currency) {
                        ui.label(format!("HT / Tax / Total : {net} / {vat} / {total} {}", draft.currency));
                    }
                    for line in &quote.lines { ui.label(format!("{} × {} · {}", line.quantity, line.description, line.price)); }
                    let live = self.live_since.is_some();
                    if state.number.is_none() {
                        if live {
                            let next = self.next_numbers.iter().find(|(kind, _)| *kind == state.kind).map(|(_, n)| n.as_str()).unwrap_or("");
                            ui.colored_label(PLUM, format!("{} {next}. {}", t("La finalisation attribuera le numéro", "Finalizing will assign number"), t("Le document deviendra définitif et non modifiable.", "The document becomes final and can no longer be edited.")));
                        } else {
                            ui.label(t("Numérotation réelle désactivée : la finalisation produit une démonstration numérotée TEST. Activez-la dans Paramètres.", "Real numbering is off: finalizing produces a TEST-numbered demonstration. Enable it in Settings."));
                        }
                    }
                    if state.kind == "invoice" && state.number.is_none() {
                        ui.label(t("Affecter un acompte de démonstration", "Apply a demonstration deposit"));
                        egui::ComboBox::from_id_salt("deposit_to_apply").selected_text(if self.deposit_id.is_empty() { t("Choisir un acompte", "Choose a deposit") } else { &self.deposit_id }).show_ui(ui, |ui| {
                            for candidate in &self.items {
                                if let Ok(info) = self.store.billing_state(candidate.id)
                                    && info.kind == "deposit" && info.number.is_some() && info.source_id.is_some() && info.source_id == state.source_id && candidate.currency == draft.currency {
                                        let label = format!("{} · {}", candidate.title, info.number.unwrap_or_default());
                                        if ui.selectable_label(self.deposit_id == candidate.id.to_string(), label).clicked() { self.deposit_id = candidate.id.to_string(); }
                                }
                            }
                        });
                        if ui.button(t("Affecter", "Apply")).clicked() {
                            match self.deposit_id.parse::<i64>() { Ok(deposit) => match self.store.allocate_deposit(id, deposit) { Ok(()) => { self.review = None; self.deposit_id.clear(); self.refresh(); }, Err(error) => self.error = Some(error.to_string()) }, Err(_) => self.error = Some(t("Choisissez un acompte.", "Choose a deposit.").into()) }
                        }
                    }
                    let finalize_label = if live { t("Finaliser et numéroter", "Finalize and number") } else { t("Finaliser la démonstration", "Finalize demonstration") };
                    if state.number.is_none() && ui.add(crate::brand::primary(finalize_label)).clicked() {
                        let result = self.store.finalize_document_versioned(id, self.review_version, |draft, quote, number, issued_on| {
                            let issuer = self.store.issuer()?;
                            let kind = crate::pdf::kind_label(&state.kind, &draft.language);
                            let label = if number.starts_with("TEST-") {
                                let marker = if draft.language == "fr" { "DÉMONSTRATION" } else { "DEMONSTRATION" };
                                format!("{marker} · {kind} · {number}")
                            } else {
                                format!("{kind} · {number}")
                            };
                            crate::pdf::render(&label, &draft.title, &draft.currency, &draft.language, quote,
                                crate::pdf::DocumentContext {
                                    issuer: Some(&issuer),
                                    applied_deposit: (state.kind == "invoice").then_some((&state.deposits, &state.outstanding)),
                                    issued_on: Some(issued_on),
                                })
                        });
                        match result { Ok(number) => { self.notice = Some(format!("{} {number}", if number.starts_with("TEST-") { t("Démonstration finalisée :", "Demonstration finalized:") } else { t("Document finalisé :", "Document finalized:") })); self.review = None; self.refresh(); }, Err(error) => self.error = Some(error.to_string()) }
                    }
                    if state.number.is_some() {
                        if ui.button(t("Exporter le PDF figé", "Export finalized PDF")).clicked() {
                            match self.store.finalized_pdf(id) { Ok(bytes) => if let Some(path) = rfd::FileDialog::new().add_filter("PDF", &["pdf"]).set_file_name(format!("{}.pdf", state.number.as_deref().unwrap_or("document"))).save_file() && let Err(error) = crate::operations::write_file(&path, &bytes) { self.error = Some(error.to_string()); }, Err(error) => self.error = Some(error.to_string()) }
                        }
                        if state.kind == "quote" && !state.accepted {
                            ui.horizontal(|ui| { ui.label(t("Date d’acceptation (AAAA-MM-JJ)", "Acceptance date (YYYY-MM-DD)")); ui.text_edit_singleline(&mut self.event_date); });
                            if ui.button(t("Marquer accepté", "Mark accepted")).clicked() { match self.store.accept_quote(id, &self.event_date) { Ok(()) => { self.review = None; self.refresh(); }, Err(error) => self.error = Some(error.to_string()) } }
                        }
                        if state.kind == "quote" && state.accepted {
                            for (kind, label) in [("invoice", t("Créer une facture", "Create invoice")), ("deposit", t("Créer un acompte", "Create deposit"))] {
                                if ui.button(label).clicked() { match self.store.derive_document(id, kind, &draft.title) { Ok(_) => { self.page = Page::Invoices; self.review = None; self.refresh(); }, Err(error) => self.error = Some(error.to_string()) } }
                            }
                        }
                        if state.kind == "invoice" && ui.button(t("Créer un avoir", "Create credit note")).clicked() { match self.store.derive_document(id, "credit", &draft.title) { Ok(_) => { self.review = None; self.refresh(); }, Err(error) => self.error = Some(error.to_string()) } }
                        if matches!(state.kind.as_str(), "invoice" | "deposit") {
                            ui.label(format!("{}: {} · {}: {} · {}: {} · {}: {}", t("Acomptes", "Deposits"), state.deposits, t("Réglé", "Paid"), state.paid, t("Avoirs", "Credits"), state.credited, t("Reste à payer", "Outstanding"), state.outstanding));
                            ui.horizontal(|ui| { ui.label(t("Montant", "Amount")); ui.text_edit_singleline(&mut self.payment_amount); ui.label(t("Date AAAA-MM-JJ", "Date YYYY-MM-DD")); ui.text_edit_singleline(&mut self.event_date); });
                            ui.horizontal(|ui| { ui.label(t("Mode", "Method")); ui.text_edit_singleline(&mut self.payment_method); });
                            if ui.button(t("Enregistrer le règlement", "Record payment")).clicked() { match self.store.record_payment(id, &self.payment_amount, &self.event_date, &self.payment_method, "") { Ok(()) => { self.review = None; self.refresh(); }, Err(error) => self.error = Some(error.to_string()) } }
                        }
                    }
                    if let Some(error) = &self.error { ui.colored_label(ui.visuals().error_fg_color, error); }
                    if ui.button(t("Fermer", "Close")).clicked() { self.review = None; self.error = None; }
                });
            } else {
                self.review = None;
            }
        }
        if self.confirm_discard {
            egui::Window::new(t("Abandonner les modifications ?", "Discard changes?"))
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(t(
                        "Les modifications non enregistrées seront perdues.",
                        "Unsaved changes will be lost.",
                    ));
                    ui.horizontal(|ui| {
                        if ui
                            .button(t("Continuer la saisie", "Keep editing"))
                            .clicked()
                        {
                            self.confirm_discard = false;
                        }
                        if ui.button(t("Abandonner", "Discard")).clicked() {
                            self.confirm_discard = false;
                            self.composing = false;
                            self.error = None;
                            if self.close_pending {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                self.close_pending = false;
                            }
                        }
                    });
                });
        }
    }
}

impl Drafts {
    fn products_page(&mut self, ui: &mut egui::Ui, fr: bool) {
        let t = |f, e| if fr { f } else { e };
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(t("Produits & services", "Products & services"))
                        .size(30.0)
                        .strong()
                        .color(INK),
                );
                ui.label(
                    egui::RichText::new(t(
                        "Prestations réutilisables à ajouter en un clic dans vos devis et factures",
                        "Reusable items to add to quotes and invoices in one click",
                    ))
                    .color(crate::brand::MUTED),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(crate::brand::primary(t(
                        "+ Nouveau produit",
                        "+ New product",
                    )))
                    .clicked()
                {
                    self.product_edit = Some(Product {
                        vat: "20".into(),
                        ..Default::default()
                    });
                    self.error = None;
                }
            });
        });
        ui.add_space(12.0);
        if let Some(product) = &mut self.product_edit {
            let mut close = false;
            crate::brand::card(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.label(
                    egui::RichText::new(if product.id == 0 {
                        t("Nouveau produit", "New product")
                    } else {
                        t("Modifier le produit", "Edit product")
                    })
                    .size(16.0)
                    .strong(),
                );
                egui::Grid::new("product_form")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        for (label, field, width) in [
                            (t("Nom", "Name"), &mut product.name, 320.0),
                            (t("Unité", "Unit"), &mut product.unit, 120.0),
                            (
                                t("Prix unitaire HT", "Unit price (excl. tax)"),
                                &mut product.price,
                                120.0,
                            ),
                            (t("TVA %", "Tax %"), &mut product.vat, 80.0),
                            (
                                t("Motif d’exonération", "Tax exemption reason"),
                                &mut product.exemption_reason,
                                320.0,
                            ),
                        ] {
                            ui.label(label);
                            ui.add(egui::TextEdit::singleline(field).desired_width(width));
                            ui.end_row();
                        }
                        ui.label(t("Description", "Description"));
                        ui.add(
                            egui::TextEdit::multiline(&mut product.description)
                                .desired_rows(3)
                                .desired_width(320.0),
                        );
                        ui.end_row();
                    });
                ui.weak(t(
                    "Avec un motif d’exonération, la TVA doit être à 0 %.",
                    "With an exemption reason, tax must be 0 %.",
                ));
                ui.horizontal(|ui| {
                    if ui
                        .add(crate::brand::primary(t("Enregistrer", "Save")))
                        .clicked()
                    {
                        if product.name.trim().is_empty() || product.price.trim().is_empty() {
                            self.error = Some(
                                t(
                                    "Indiquez un nom et un prix unitaire HT.",
                                    "Enter a name and a unit price.",
                                )
                                .into(),
                            );
                        } else {
                            match self.store.save_product(product) {
                                Ok(_) => close = true,
                                Err(error) => self.error = Some(error.to_string()),
                            }
                        }
                    }
                    if ui.button(t("Annuler", "Cancel")).clicked() {
                        close = true;
                    }
                });
            });
            if close {
                self.product_edit = None;
                self.error = None;
                self.refresh();
            }
            ui.add_space(12.0);
        }
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        if self.products.is_empty() && self.product_edit.is_none() {
            crate::brand::card(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.heading(t("Votre catalogue est vide", "Your catalogue is empty"));
                ui.label(t("Enregistrez une prestation que vous facturez souvent : nom, prix, unité et TVA. Elle sera recopiée dans le document, qui restera inchangé si vous modifiez le catalogue.", "Save an item you often bill: name, price, unit and tax. It is copied into the document, which stays unchanged if you edit the catalogue."));
            });
        }
        let mut delete = None;
        egui::ScrollArea::vertical()
            .id_salt("products")
            .show(ui, |ui| {
                for product in &self.products {
                    crate::brand::card(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new(&product.name)
                                        .size(16.0)
                                        .strong()
                                        .color(INK),
                                );
                                if !product.description.trim().is_empty() {
                                    ui.weak(product.description.lines().next().unwrap_or(""));
                                }
                                let tax = if product.exemption_reason.trim().is_empty() {
                                    format!("{} {} %", t("TVA", "Tax"), product.vat)
                                } else {
                                    product.exemption_reason.clone()
                                };
                                ui.weak(tax);
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if self.confirm_delete_product == Some(product.id) {
                                        if ui
                                            .button(t("Confirmer la suppression", "Confirm delete"))
                                            .clicked()
                                        {
                                            delete = Some(product.id);
                                        }
                                    } else if ui.button(t("Supprimer", "Delete")).clicked() {
                                        self.confirm_delete_product = Some(product.id);
                                    }
                                    if ui.button(t("Modifier", "Edit")).clicked() {
                                        self.product_edit = Some(product.clone());
                                    }
                                    let unit = if product.unit.trim().is_empty() {
                                        String::new()
                                    } else {
                                        format!(" / {}", product.unit)
                                    };
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{}{unit}",
                                            crate::brand::money(&product.price, "EUR", fr)
                                        ))
                                        .size(16.0)
                                        .strong()
                                        .color(PLUM),
                                    );
                                },
                            );
                        });
                    });
                    ui.add_space(8.0);
                }
            });
        if let Some(id) = delete {
            self.confirm_delete_product = None;
            if let Err(error) = self.store.delete_product(id) {
                self.error = Some(error.to_string());
            }
            self.refresh();
        }
    }

    fn numbering_settings(&mut self, ui: &mut egui::Ui, fr: bool) {
        let t = |f, e| if fr { f } else { e };
        ui.heading(t("Numérotation et émission", "Numbering and issuance"));
        crate::brand::card(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let names = |kind: &str| match kind {
                "quote" => t("Prochain devis", "Next quote"),
                "invoice" => t("Prochaine facture", "Next invoice"),
                "deposit" => t("Prochain acompte", "Next deposit"),
                _ => t("Prochain avoir", "Next credit note"),
            };
            match &self.live_since {
                Some(since) => {
                    crate::brand::badge(
                        ui,
                        t("Émission réelle activée", "Real issuance enabled"),
                        Tone::Positive,
                    );
                    ui.label(format!(
                        "{} {since}. {}",
                        t("Depuis le", "Since"),
                        t(
                            "FactureCraft est votre seul outil d’émission.",
                            "FactureCraft is your only issuing tool."
                        )
                    ));
                }
                None => {
                    crate::brand::badge(
                        ui,
                        t("Mode démonstration", "Demonstration mode"),
                        Tone::Draft,
                    );
                    ui.label(t("Les documents finalisés reçoivent un numéro TEST. Activez l’émission réelle quand vous n’émettez plus de documents sur Facture.net.", "Finalized documents receive a TEST number. Enable real issuance once you no longer issue documents on Facture.net."));
                }
            }
            ui.label(t("Format repris de Facture.net : préfixe, mois, année sur deux chiffres, compteur annuel à cinq chiffres. La série continue après le dernier numéro importé.", "Format carried over from Facture.net: prefix, month, two-digit year, five-digit yearly counter. The series continues after the last imported number."));
            egui::Grid::new("next_numbers")
                .num_columns(2)
                .spacing([16.0, 4.0])
                .show(ui, |ui| {
                    for (kind, number) in &self.next_numbers {
                        ui.label(names(kind));
                        ui.label(egui::RichText::new(number).strong().color(PLUM));
                        ui.end_row();
                    }
                });
            ui.add_space(6.0);
            if self.live_since.is_none() {
                ui.checkbox(&mut self.confirm_live, t("Je n’émets plus de documents sur Facture.net et je confirme la reprise de la série ci-dessus.", "I no longer issue documents on Facture.net and confirm continuing the series above."));
                if ui
                    .add_enabled(
                        self.confirm_live,
                        crate::brand::primary(t(
                            "Activer l’émission réelle",
                            "Enable real issuance",
                        )),
                    )
                    .clicked()
                {
                    match self.store.enable_live_numbering() {
                        Ok(()) => {
                            self.confirm_live = false;
                            self.notice = Some(
                                t("Émission réelle activée.", "Real issuance enabled.").into(),
                            );
                            self.error = None;
                            self.refresh();
                        }
                        Err(error) => self.error = Some(error.to_string()),
                    }
                }
            } else if ui
                .button(t(
                    "Revenir au mode démonstration",
                    "Back to demonstration mode",
                ))
                .on_hover_text(t(
                    "Possible tant qu’aucun numéro réel n’a été attribué.",
                    "Possible while no real number has been assigned.",
                ))
                .clicked()
            {
                match self.store.disable_live_numbering() {
                    Ok(()) => {
                        self.notice = None;
                        self.error = None;
                        self.refresh();
                    }
                    Err(error) => self.error = Some(error.to_string()),
                }
            }
        });
    }
}
