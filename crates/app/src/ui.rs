use crate::Language;
use anyhow::Result;
use eframe::egui;
use facturecraft_core::{Draft, Store};

use crate::brand::{AMBER, CREAM, INK, Tone, accent};
use facturecraft_core::{billing::BillingState, products::Product};

/// Currencies offered in the editor and settings; any other ISO 4217 code stays accepted.
const CURRENCIES: [(&str, &str, &str); 12] = [
    ("EUR", "Euro", "Euro"),
    ("USD", "Dollar américain", "US dollar"),
    ("GBP", "Livre sterling", "Pound sterling"),
    ("CHF", "Franc suisse", "Swiss franc"),
    ("CAD", "Dollar canadien", "Canadian dollar"),
    ("AUD", "Dollar australien", "Australian dollar"),
    ("JPY", "Yen", "Yen"),
    ("SEK", "Couronne suédoise", "Swedish krona"),
    ("NOK", "Couronne norvégienne", "Norwegian krone"),
    ("DKK", "Couronne danoise", "Danish krone"),
    ("PLN", "Złoty", "Złoty"),
    ("MAD", "Dirham marocain", "Moroccan dirham"),
];

fn currency_picker(ui: &mut egui::Ui, id: &str, value: &mut String, fr: bool) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt(id)
        .selected_text(value.as_str())
        .show_ui(ui, |ui| {
            for (code, name_fr, name_en) in CURRENCIES {
                let label = format!("{code} · {}", if fr { name_fr } else { name_en });
                if ui.selectable_label(value == code, label).clicked() {
                    *value = code.into();
                    changed = true;
                }
            }
        });
    changed
}

fn language_picker(ui: &mut egui::Ui, id: &str, value: &mut Language) -> bool {
    let before = *value;
    egui::ComboBox::from_id_salt(id)
        .selected_text(match value {
            Language::Fr => "Français",
            Language::En => "English",
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(value, Language::Fr, "Français");
            ui.selectable_value(value, Language::En, "English");
        });
    before != *value
}

pub fn run(store: Store, language: Option<Language>) -> Result<()> {
    let preference = |key: &str| store.preference(key).ok().flatten();
    let language = language
        .or_else(|| preference("ui.language").map(|code| Language::from_code(&code)))
        .unwrap_or(Language::Fr);
    let default_document_language = preference("ui.document_language")
        .map(|code| Language::from_code(&code))
        .unwrap_or(language);
    let default_currency = preference("ui.currency").unwrap_or_else(|| "EUR".into());
    // Open the dashboard where the history is, e.g. EUR archives with CHF as default.
    let store_currency = store
        .dashboard_currency(&default_currency)
        .unwrap_or_else(|_| default_currency.clone());
    if let Some(theme) = store.preference("ui.theme")? {
        crate::brand::set_theme(&theme);
    }
    let page_turn = store.preference("ui.page_turn")?.as_deref() != Some("off");
    let source = image::load_from_memory(include_bytes!("../../../assets/facturecraft-logo.png"))?
        .into_rgba8();
    let image = crate::brand::tint_logo(&source);
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
            crate::brand::apply_style(&creation.egui_ctx);
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
                currency: default_currency.clone(),
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
                board_currency: store_currency.clone(),
                default_currency,
                default_document_language,
                products: Vec::new(),
                product_edit: None,
                confirm_delete_product: None,
                live_since: None,
                next_numbers: Vec::new(),
                confirm_live: false,
                logo_texture: None,
                shown_page: Page::Dashboard,
                shown_at: f64::NEG_INFINITY,
                turn_forward: true,
                page_turn,
                payment_reference: String::new(),
                confirm_delete_document: false,
                confirm_delete_payment: None,
                confirm_delete_contact: None,
                status_filter: 0,
                logo_source: source,
            };
            app.refresh();
            Ok(Box::new(app))
        }),
    )
    .map_err(|err| anyhow::anyhow!("{err}"))
}

/// Sidebar order.
const NAVIGATION: [Page; 7] = [
    Page::Dashboard,
    Page::Quotes,
    Page::Invoices,
    Page::Clients,
    Page::Products,
    Page::Archive,
    Page::Settings,
];

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
    /// Defaults for new documents, chosen in Settings.
    default_currency: String,
    default_document_language: Language,
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
    /// Logo preview; `None` inside means no logo, outer `None` means not loaded yet.
    logo_texture: Option<Option<egui::TextureHandle>>,
    /// Page shown last frame and when it appeared, for the fade-in.
    shown_page: Page,
    shown_at: f64,
    turn_forward: bool,
    page_turn: bool,
    payment_reference: String,
    confirm_delete_document: bool,
    confirm_delete_payment: Option<i64>,
    confirm_delete_contact: Option<i64>,
    /// Index into [`STATUS_FILTERS`].
    status_filter: usize,
    /// Untinted logo, recoloured when the theme changes.
    logo_source: image::RgbaImage,
}

/// Localized status of a document, coloured like the brand kit badges.
fn status(state: &BillingState, fr: bool) -> (&'static str, Tone) {
    let t = |f, e| if fr { f } else { e };
    let outstanding: f64 = state.outstanding.parse().unwrap_or(0.0);
    match (state.kind.as_str(), &state.number) {
        (_, None) => (t("Brouillon", "Draft"), Tone::Draft),
        ("quote", Some(_)) if state.closed.as_deref() == Some("refused") => {
            (t("Refusé", "Refused"), Tone::Closed)
        }
        ("quote", Some(_)) if state.closed.is_some() => (t("Annulé", "Cancelled"), Tone::Closed),
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

/// Status filter labels (fr, en) shown above document lists.
const STATUS_FILTERS: [(&str, &str); 6] = [
    ("Tous les états", "All statuses"),
    ("Brouillons", "Drafts"),
    ("En attente", "Awaiting"),
    ("Payées ou acceptés", "Paid or accepted"),
    ("Refusés ou annulés", "Refused or cancelled"),
    ("Finalisés", "Finalized"),
];

fn matches_filter(filter: usize, state: &BillingState, fr: bool) -> bool {
    let (_, tone) = status(state, fr);
    match filter {
        1 => state.number.is_none(),
        2 => matches!(tone, Tone::Pending | Tone::Sent) && state.number.is_some(),
        3 => matches!(tone, Tone::Positive),
        4 => matches!(tone, Tone::Closed),
        5 => state.number.is_some(),
        _ => true,
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
        crate::brand::paint_background(ctx);
        let now = ctx.input(|input| input.time);
        if self.shown_page != self.page {
            // Going down the menu turns the page forward, going up turns it back.
            self.turn_forward = NAVIGATION.iter().position(|p| *p == self.page)
                >= NAVIGATION.iter().position(|p| *p == self.shown_page);
            self.shown_page = self.page;
            self.shown_at = now;
        }
        egui::SidePanel::left("navigation")
            .exact_width(246.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let fill = crate::brand::with_alpha(crate::brand::deep(), 242);
                let shadow = crate::brand::with_alpha(crate::brand::deep(), 70);
                crate::brand::glass_panel(ui, fill, 26, Some(shadow), |ui| {
                    ui.add_space(8.0);
                    ui.image((self.logo.id(), egui::vec2(54.0, 54.0)));
                    ui.label(
                        egui::RichText::new("FactureCraft")
                            .size(20.0)
                            .strong()
                            .color(CREAM),
                    );
                    let (line, _) =
                        ui.allocate_exact_size(egui::vec2(38.0, 3.0), egui::Sense::hover());
                    ui.painter().rect_filled(line, 2.0, AMBER);
                    ui.add_space(17.0);
                    if let Some(index) = NAVIGATION.iter().position(|page| *page == self.page) {
                        let step = 40.0 + ui.spacing().item_spacing.y;
                        let start = ui.cursor().min + egui::vec2(0.0, step * index as f32);
                        crate::brand::nav_indicator(
                            ui,
                            egui::Rect::from_min_size(
                                start,
                                egui::vec2(ui.available_width(), 40.0),
                            ),
                        );
                    }
                    for (page, icon, label) in [
                        (Page::Dashboard, "📊", t("Tableau de bord", "Dashboard")),
                        (Page::Quotes, "🗋", t("Devis", "Quotes")),
                        (Page::Invoices, "🗐", t("Factures", "Invoices")),
                        (Page::Clients, "👤", t("Clients", "Clients")),
                        (
                            Page::Products,
                            "📦",
                            t("Produits & services", "Products & services"),
                        ),
                        (Page::Archive, "🗄", t("Archives", "Archive")),
                        (Page::Settings, "⚙", t("Paramètres", "Settings")),
                    ] {
                        if ui
                            .add_enabled_ui(!self.composing, |ui| {
                                crate::brand::nav_item(ui, icon, label, self.page == page)
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
            });
        if matches!(self.page, Page::Quotes | Page::Invoices)
            && self.composing
            && ctx.content_rect().width() >= 800.0
        {
            egui::SidePanel::right("quote_preview")
                // Grows with the window, like a page beside the form.
                .exact_width((ctx.content_rect().width() * 0.34).clamp(320.0, 620.0))
                .show_separator_line(false)
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    let fill = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 150);
                    crate::brand::glass_panel(ui, fill, 22, None, |ui| self.preview(ui));
                });
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().inner_margin(egui::Margin { left: 18, right: 28, top: 28, bottom: 20 }))
            .show(ctx, |ui| {
                let turn = ((now - self.shown_at) / 0.5).clamp(0.0, 1.0) as f32;
                if turn < 1.0 && self.page_turn {
                    ctx.request_repaint();
                    crate::brand::page_turn(ui, turn, self.turn_forward);
                }
                // Readable measure on large windows: content is capped and centred.
                let measure = match self.page {
                    Page::Dashboard => 1280.0,
                    _ if self.composing => 980.0,
                    _ => 1080.0,
                };
                let space = ui.available_rect_before_wrap();
                let width = space.width().min(measure);
                let column = egui::Rect::from_min_size(
                    egui::pos2(space.center().x - width / 2.0, space.top()),
                    egui::vec2(width, space.height()),
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(column), |ui| {
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
                    crate::brand::page_header(
                        ui,
                        t("Paramètres", "Settings"),
                        t("Identité, mise en page des documents et numérotation", "Identity, document layout and numbering"),
                        |_| {},
                    );
                    egui::ScrollArea::vertical().id_salt("settings").show(ui, |ui| {
                        self.appearance_settings(ui, fr);
                        self.language_settings(ui, fr);
                        if let Some(notice) = &self.notice { ui.label(notice); }
                        if let Some(error) = &self.error { ui.colored_label(ui.visuals().error_fg_color, error); }
                        self.issuer_settings(ui, fr);
                        ui.add_space(18.0);
                        self.numbering_settings(ui, fr);
                    });
                    return;
                }
                if self.page == Page::Clients {
                    crate::brand::page_header(
                        ui,
                        t("Clients", "Clients"),
                        t("Vos clients et sociétés, y compris ceux importés de Facture.net", "Your clients and companies, including those imported from Facture.net"),
                        |ui| {
                            if ui.add(crate::brand::primary(t("+ Nouveau client", "+ New client"))).clicked() {
                                self.new_contact.get_or_insert_with(Default::default);
                            }
                        },
                    );
                    if let Some(contact) = &mut self.new_contact {
                        let mut close = false;
                        crate::brand::card(ui, |ui| {
                                crate::brand::card_title(ui, if contact.id.is_some() { t("Modifier le client", "Edit client") } else { t("Nouveau client", "New client") });
                                egui::Grid::new("contact_form").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                                    for (label, field, rows) in [
                                        (t("Nom ou société", "Name or company"), &mut contact.name, 1),
                                        (t("Adresse", "Address"), &mut contact.address, 2),
                                        (t("Pays", "Country"), &mut contact.country, 1),
                                        (t("N° d’entreprise", "Company number"), &mut contact.registration, 1),
                                        (t("E-mail", "Email"), &mut contact.email, 1),
                                    ] {
                                        ui.label(label);
                                        let edit = if rows > 1 { crate::brand::area(field).desired_rows(rows) } else { crate::brand::field(field) };
                                        ui.add(edit.desired_width(ui.available_width()));
                                        ui.end_row();
                                    }
                                });
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
                        crate::brand::field(&mut self.contact_query).hint_text(t(
                            "Rechercher un client ou une société",
                            "Search clients or companies",
                        )),
                    );
                    let query = self.contact_query.trim().to_lowercase();
                    egui::ScrollArea::vertical()
                        .id_salt("contacts")
                        .show(ui, |ui| {
                            let mut found = false;
                            let mut edit_contact = None;
                            let mut delete_contact = None;
                            for contact in &self.contacts {
                                if ![&contact.name, &contact.email, &contact.address]
                                    .iter()
                                    .any(|field| field.to_lowercase().contains(&query))
                                {
                                    continue;
                                }
                                found = true;
                                crate::brand::card(ui, |ui| {
                                        ui.set_min_width(ui.available_width());
                                        ui.horizontal(|ui| {
                                            ui.vertical(|ui| {
                                                ui.strong(&contact.name);
                                                for field in [&contact.email, &contact.address.replace('\n', ", "), &contact.registration] {
                                                    if !field.is_empty() {
                                                        ui.weak(field);
                                                    }
                                                }
                                            });
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                match contact.id {
                                                    Some(id) => {
                                                        if self.confirm_delete_contact == Some(id) {
                                                            if ui.button(egui::RichText::new(t("Confirmer la suppression", "Confirm deletion")).color(ui.visuals().error_fg_color)).clicked() {
                                                                delete_contact = Some(id);
                                                            }
                                                        } else if ui.button(t("Supprimer", "Delete")).clicked() {
                                                            self.confirm_delete_contact = Some(id);
                                                        }
                                                        if ui.button(t("Modifier", "Edit")).clicked() {
                                                            edit_contact = Some(contact.clone());
                                                        }
                                                    }
                                                    None => {
                                                        if ui.button(t("Copier dans le carnet", "Copy to address book")).on_hover_text(t("Contact importé de Facture.net, en lecture seule", "Imported from Facture.net, read only")).clicked() {
                                                            edit_contact = Some(contact.clone());
                                                        }
                                                    }
                                                }
                                            });
                                        });
                                    });
                            }
                            if !found {
                                ui.weak(t("Aucun contact trouvé.", "No contacts found."));
                            }
                            if let Some(contact) = edit_contact {
                                self.new_contact = Some(contact);
                            }
                            if let Some(id) = delete_contact {
                                self.confirm_delete_contact = None;
                                if let Err(error) = self.store.delete_contact(id) {
                                    self.error = Some(error.to_string());
                                }
                                self.refresh();
                            }
                        });
                    return;
                }
                if self.page == Page::Archive {
                    crate::brand::page_header(
                        ui,
                        t("Archives", "Archive"),
                        t("Historique importé de Facture.net, en lecture seule", "History imported from Facture.net, read only"),
                        |_| {},
                    );
                    ui.add(
                        crate::brand::field(&mut self.archive_query)
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
                let subtitle = if self.live_since.is_some() {
                    t("Brouillons et documents émis", "Drafts and issued documents")
                } else {
                    t("Brouillons et documents de démonstration", "Drafts and demonstration documents")
                };
                let page_title = if self.page == Page::Invoices { t("Factures", "Invoices") } else { t("Devis", "Quotes") };
                crate::brand::page_header(ui, page_title, subtitle, |ui| {
                        if ui
                            .add_enabled(
                                !self.composing,
                                crate::brand::primary(if self.page == Page::Invoices {
                                    t("+ Nouvelle facture", "+ New invoice")
                                } else {
                                    t("+ Nouveau devis", "+ New quote")
                                }),
                            )
                            .clicked()
                        {
                            self.editing = None;
                            self.edit_version = None;
                            self.title.clear();
                            if self.page == Page::Invoices {
                                match self.store.create_invoice(
                                    t("Nouvelle facture", "New invoice"),
                                    &self.default_currency,
                                    self.default_document_language.code(),
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
                            self.quote = self.new_quote();
                            if self.page == Page::Invoices {
                                self.quote.due_date = self.store.date_in_days(30).unwrap_or_default();
                            }
                            self.currency = self.default_currency.clone();
                            self.document_language = self.default_document_language;
                            self.error = None;
                            self.composing = true;
                        }
                });

                if self.composing {
                    egui::ScrollArea::vertical()
                        .id_salt("editor")
                        .show(ui, |ui| {
                            crate::brand::card(ui, |ui| {
                                    ui.label(t("Titre du devis", "Quote title"));
                                    ui.add(
                                        crate::brand::field(&mut self.title)
                                            .desired_width(f32::INFINITY),
                                    );
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(t("Devise", "Currency"));
                                        currency_picker(ui, "document_currency", &mut self.currency, fr);
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
                                                    self.quote
                                                        .recipient_country
                                                        .clone_from(&contact.country);
                                                    self.quote
                                                        .recipient_registration
                                                        .clone_from(&contact.registration);
                                                }
                                            }
                                        });
                                    egui::Grid::new("recipient_fields").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                                        for (label, field, rows) in [
                                            (t("Nom ou société", "Name or company"), &mut self.quote.recipient, 1),
                                            (t("Adresse", "Address"), &mut self.quote.recipient_address, 2),
                                            (t("Pays", "Country"), &mut self.quote.recipient_country, 1),
                                            (t("N° d’entreprise", "Company number"), &mut self.quote.recipient_registration, 1),
                                            (t("E-mail", "Email"), &mut self.quote.recipient_email, 1),
                                        ] {
                                            ui.label(label);
                                            let edit = if rows > 1 { crate::brand::area(field).desired_rows(rows) } else { crate::brand::field(field) };
                                            ui.add(edit.desired_width(ui.available_width()));
                                            ui.end_row();
                                        }
                                    });
                                    if ui
                                        .button(t("Ajouter au carnet", "Save to contacts"))
                                        .clicked()
                                    {
                                        let contact = facturecraft_core::contacts::Contact {
                                            id: None,
                                            name: self.quote.recipient.clone(),
                                            email: self.quote.recipient_email.clone(),
                                            address: self.quote.recipient_address.clone(),
                                            country: self.quote.recipient_country.clone(),
                                            registration: self.quote.recipient_registration.clone(),
                                        };
                                        match self.store.save_contact(&contact) {
                                            Ok(()) => self.refresh(),
                                            Err(error) => self.error = Some(error.to_string()),
                                        }
                                    }
                                    ui.separator();
                                    crate::brand::section(ui, t("Prestations", "Line items"));
                                    ui.scope(|ui| {
                                            let mut remove = None;
                                            for (index, line) in
                                                self.quote.lines.iter_mut().enumerate()
                                            {
                                                ui.push_id(index, |ui| {
                                                    ui.label(t("Description : intitulé sur la première ligne, détail en dessous", "Description: title on the first line, details below"));
                                                    ui.add(crate::brand::area(&mut line.description).desired_rows(6).desired_width(f32::INFINITY));
                                                    ui.horizontal(|ui| {
                                                        ui.label(t("Unité", "Unit"));
                                                        ui.add(crate::brand::field(&mut line.unit));
                                                    });
                                                    ui.horizontal(|ui| {
                                                        ui.label(t(
                                                            "Motif d’exonération",
                                                            "Tax exemption reason",
                                                        ));
                                                        ui.add(crate::brand::field(
                                                            &mut line.exemption_reason,
                                                        ));
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
                                                                crate::brand::field(field)
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
                                    crate::brand::section(ui, t("Conditions", "Terms"));
                                    ui.horizontal(|ui| {
                                        ui.label(t("Remise globale %", "Overall discount %"));
                                        ui.add(crate::brand::field(&mut self.quote.global_discount));
                                    });
                                    egui::Grid::new("terms_fields").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                                        for (label, field) in [
                                            (t("Conditions de règlement", "Payment terms"), &mut self.quote.payment_terms),
                                            (t("Mode de règlement", "Payment method"), &mut self.quote.payment_method),
                                            (t("Validité du devis", "Quote valid for"), &mut self.quote.valid_until),
                                            (t("Échéance (AAAA-MM-JJ, factures)", "Due date (YYYY-MM-DD, invoices)"), &mut self.quote.due_date),
                                            (t("Date de prestation (factures)", "Service date (invoices)"), &mut self.quote.service_date),
                                        ] {
                                            ui.label(label);
                                            ui.add(crate::brand::field(field).desired_width(ui.available_width()));
                                            ui.end_row();
                                        }
                                    });
                                    ui.label(t("Conditions particulières (page Conditions du PDF)", "Specific conditions (Terms page of the PDF)"));
                                    ui.add(crate::brand::area(&mut self.quote.terms).desired_rows(10).desired_width(f32::INFINITY));
                                    ui.collapsing(t("Conditions générales de vente de ce document", "General terms of sale for this document"), |ui| {
                                        ui.horizontal(|ui| {
                                            ui.weak(t("Reprises des Paramètres à la création du document, modifiables ici sans toucher au modèle.", "Copied from Settings when the document is created; editing here leaves the template unchanged."));
                                            if ui.button(t("Recharger le modèle", "Reload template")).clicked() {
                                                self.quote.cgv = self.store.issuer().map(|issuer| issuer.cgv).unwrap_or_default();
                                            }
                                        });
                                        ui.add(crate::brand::area(&mut self.quote.cgv).desired_rows(14).desired_width(f32::INFINITY));
                                    });
                                    ui.weak(t("Mise en forme : **gras**, « # » en début de ligne pour un intertitre, ligne vide pour un nouveau paragraphe. Valable dans les descriptions, les conditions et les CGV.", "Formatting: **bold**, “# ” at line start for a subheading, blank line for a new paragraph. Works in descriptions, conditions and terms of sale."));
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
                                            let result = (|| {
                                                let issuer = self.store.issuer()?;
                                                let logo = self.store.logo()?;
                                                crate::pdf::render(&crate::pdf::Document {
                                                    kind: &kind,
                                                    number: None,
                                                    date: &self.store.today()?,
                                                    title: &self.title,
                                                    currency: &self.currency,
                                                    language: self.document_language.code(),
                                                    quote: &self.quote,
                                                    issuer: &issuer,
                                                    logo: logo.as_deref(),
                                                    applied_deposit: None,
                                                })
                                            })();
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
                    crate::brand::card(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            crate::brand::card_title(ui, t("Créez votre premier devis", "Create your first quote"));
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
                                self.quote = self.new_quote();
                                self.currency = self.default_currency.clone();
                                self.document_language = self.default_document_language;
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
                    ui.horizontal(|ui| {
                        ui.add(
                            crate::brand::field(&mut self.quote_query)
                                .hint_text(t("Rechercher : titre, client, numéro", "Search: title, client, number")),
                        );
                        egui::ComboBox::from_id_salt("status_filter")
                            .selected_text(if fr { STATUS_FILTERS[self.status_filter].0 } else { STATUS_FILTERS[self.status_filter].1 })
                            .show_ui(ui, |ui| {
                                for (index, (label_fr, label_en)) in STATUS_FILTERS.iter().enumerate() {
                                    ui.selectable_value(&mut self.status_filter, index, if fr { *label_fr } else { *label_en });
                                }
                            });
                    });
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
                            if !matches_filter(self.status_filter, &state, fr) {
                                continue;
                            }
                            let recipient = self.store.quote(draft.id).map(|q| q.recipient).unwrap_or_default();
                            if ![draft.title.as_str(), recipient.as_str(), state.number.as_deref().unwrap_or("")]
                                .iter()
                                .any(|field| field.to_lowercase().contains(&query))
                                && !draft.id.to_string().contains(&query)
                            {
                                continue;
                            }
                            found = true;
                            crate::brand::card(ui, |ui| {
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
                            ui.colored_label(accent(), format!("{} {next}. {}", t("La finalisation attribuera le numéro", "Finalizing will assign number"), t("Le document deviendra définitif et non modifiable.", "The document becomes final and can no longer be edited.")));
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
                    let needs_due_date = matches!(state.kind.as_str(), "invoice" | "deposit") && quote.due_date.trim().is_empty();
                    if state.number.is_none() && ui.add(crate::brand::primary(finalize_label)).clicked() {
                        if needs_due_date {
                            self.error = Some(t("Indiquez la date d’échéance dans le document avant de le finaliser.", "Enter the due date in the document before finalizing it.").into());
                        } else {
                        let result = self.store.finalize_document_versioned(id, self.review_version, |draft, quote, number, issued_on| {
                            let issuer = self.store.issuer()?;
                            let logo = self.store.logo()?;
                            crate::pdf::render(&crate::pdf::Document {
                                kind: &state.kind,
                                number: Some(number),
                                date: issued_on,
                                title: &draft.title,
                                currency: &draft.currency,
                                language: &draft.language,
                                quote,
                                issuer: &issuer,
                                logo: logo.as_deref(),
                                applied_deposit: (state.kind == "invoice").then_some((&state.deposits, &state.outstanding)),
                            })
                        });
                        match result { Ok(number) => { self.notice = Some(format!("{} {number}", if number.starts_with("TEST-") { t("Démonstration finalisée :", "Demonstration finalized:") } else { t("Document finalisé :", "Document finalized:") })); self.review = None; self.refresh(); }, Err(error) => self.error = Some(error.to_string()) }
                        }
                    }
                    ui.horizontal(|ui| {
                        if matches!(state.kind.as_str(), "quote" | "invoice") && ui.button(t("Dupliquer", "Duplicate")).clicked() {
                            let title = format!("{} {}", t("Copie de", "Copy of"), draft.title);
                            match self.store.duplicate_document(id, &title) {
                                Ok(copy) => { self.notice = Some(format!("{} « {} ».", t("Brouillon créé :", "Draft created:"), copy.title)); self.review = None; self.refresh(); }
                                Err(error) => self.error = Some(error.to_string()),
                            }
                        }
                        if state.number.is_none() {
                            if self.confirm_delete_document {
                                if ui.button(egui::RichText::new(t("Confirmer la suppression", "Confirm deletion")).color(ui.visuals().error_fg_color)).clicked() {
                                    match self.store.delete_draft(id) {
                                        Ok(()) => { self.notice = Some(t("Brouillon supprimé.", "Draft deleted.").into()); self.review = None; self.confirm_delete_document = false; self.refresh(); }
                                        Err(error) => self.error = Some(error.to_string()),
                                    }
                                }
                            } else if ui.button(t("Supprimer le brouillon", "Delete draft")).clicked() {
                                self.confirm_delete_document = true;
                            }
                        }
                    });
                    if state.number.is_some() {
                        if ui.button(t("Exporter le PDF figé", "Export finalized PDF")).clicked() {
                            match self.store.finalized_pdf(id) { Ok(bytes) => if let Some(path) = rfd::FileDialog::new().add_filter("PDF", &["pdf"]).set_file_name(format!("{}.pdf", state.number.as_deref().unwrap_or("document"))).save_file() && let Err(error) = crate::operations::write_file(&path, &bytes) { self.error = Some(error.to_string()); }, Err(error) => self.error = Some(error.to_string()) }
                        }
                        if state.kind == "quote" && !state.accepted && state.closed.is_none() {
                            ui.horizontal(|ui| { ui.label(t("Date (AAAA-MM-JJ)", "Date (YYYY-MM-DD)")); ui.add(crate::brand::field(&mut self.event_date)); });
                            ui.horizontal(|ui| {
                                if ui.button(t("Marquer accepté", "Mark accepted")).clicked() { match self.store.accept_quote(id, &self.event_date) { Ok(()) => { self.review = None; self.refresh(); }, Err(error) => self.error = Some(error.to_string()) } }
                                for (status, label) in [("refused", t("Marquer refusé", "Mark refused")), ("cancelled", t("Marquer annulé", "Mark cancelled"))] {
                                    if ui.button(label).clicked() { match self.store.close_quote(id, status, &self.event_date) { Ok(()) => { self.review = None; self.refresh(); }, Err(error) => self.error = Some(error.to_string()) } }
                                }
                            });
                        }
                        if state.kind == "quote" && state.accepted {
                            for (kind, label) in [("invoice", t("Créer une facture", "Create invoice")), ("deposit", t("Créer un acompte", "Create deposit"))] {
                                if ui.button(label).clicked() { match self.store.derive_document(id, kind, &draft.title) { Ok(_) => { self.page = Page::Invoices; self.review = None; self.refresh(); }, Err(error) => self.error = Some(error.to_string()) } }
                            }
                        }
                        if state.kind == "invoice" && ui.button(t("Créer un avoir", "Create credit note")).clicked() { match self.store.derive_document(id, "credit", &draft.title) { Ok(_) => { self.review = None; self.refresh(); }, Err(error) => self.error = Some(error.to_string()) } }
                        if matches!(state.kind.as_str(), "invoice" | "deposit") {
                            ui.label(format!("{}: {} · {}: {} · {}: {} · {}: {}", t("Acomptes", "Deposits"), state.deposits, t("Réglé", "Paid"), state.paid, t("Avoirs", "Credits"), state.credited, t("Reste à payer", "Outstanding"), state.outstanding));
                            ui.horizontal(|ui| { ui.label(t("Montant", "Amount")); ui.add(crate::brand::field(&mut self.payment_amount)); ui.label(t("Date AAAA-MM-JJ", "Date YYYY-MM-DD")); ui.add(crate::brand::field(&mut self.event_date)); });
                            ui.horizontal(|ui| { ui.label(t("Mode", "Method")); ui.add(crate::brand::field(&mut self.payment_method)); ui.label(t("Référence", "Reference")); ui.add(crate::brand::field(&mut self.payment_reference)); });
                            if ui.button(t("Enregistrer le règlement", "Record payment")).clicked() { match self.store.record_payment(id, &self.payment_amount, &self.event_date, &self.payment_method, &self.payment_reference) { Ok(()) => { self.payment_amount.clear(); self.payment_reference.clear(); self.error = None; self.refresh(); }, Err(error) => self.error = Some(error.to_string()) } }
                            let payments = self.store.payments(id).unwrap_or_default();
                            if !payments.is_empty() {
                                crate::brand::section(ui, t("Règlements reçus", "Payments received"));
                            }
                            for payment in payments {
                                ui.horizontal(|ui| {
                                    ui.label(format!("{} · {} · {}{}", payment.date, crate::brand::money(&payment.amount, &draft.currency, fr), payment.method, if payment.reference.is_empty() { String::new() } else { format!(" · {}", payment.reference) }));
                                    if self.confirm_delete_payment == Some(payment.id) {
                                        if ui.button(egui::RichText::new(t("Confirmer", "Confirm")).color(ui.visuals().error_fg_color)).clicked() {
                                            match self.store.delete_payment(payment.id) { Ok(()) => { self.confirm_delete_payment = None; self.refresh(); }, Err(error) => self.error = Some(error.to_string()) }
                                        }
                                    } else if ui.small_button(t("Annuler ce règlement", "Remove this payment")).clicked() {
                                        self.confirm_delete_payment = Some(payment.id);
                                    }
                                });
                            }
                        }
                    }
                    if let Some(error) = &self.error { ui.colored_label(ui.visuals().error_fg_color, error); }
                    if ui.button(t("Fermer", "Close")).clicked() { self.review = None; self.error = None; self.confirm_delete_document = false; self.confirm_delete_payment = None; }
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
    /// Empty document carrying the issuer's default conditions.
    fn new_quote(&self) -> facturecraft_core::quote::Quote {
        let issuer = self.store.issuer().unwrap_or_default();
        facturecraft_core::quote::Quote {
            terms: issuer.terms,
            payment_terms: issuer.payment_terms,
            payment_method: issuer.payment_method,
            valid_until: issuer.quote_validity,
            cgv: issuer.cgv,
            ..Default::default()
        }
    }

    fn products_page(&mut self, ui: &mut egui::Ui, fr: bool) {
        let t = |f, e| if fr { f } else { e };
        crate::brand::page_header(
            ui,
            t("Produits & services", "Products & services"),
            t(
                "Prestations réutilisables à ajouter en un clic dans vos devis et factures",
                "Reusable items to add to quotes and invoices in one click",
            ),
            |ui| {
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
            },
        );
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
                            ui.add(crate::brand::field(field).desired_width(width));
                            ui.end_row();
                        }
                        ui.label(t("Description", "Description"));
                        ui.add(
                            crate::brand::area(&mut product.description)
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
                crate::brand::card_title(
                    ui,
                    t("Votre catalogue est vide", "Your catalogue is empty"),
                );
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
                                            crate::brand::money(
                                                &product.price,
                                                &self.default_currency,
                                                fr
                                            )
                                        ))
                                        .size(16.0)
                                        .strong()
                                        .color(accent()),
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

    fn language_settings(&mut self, ui: &mut egui::Ui, fr: bool) {
        let t = |f, e| if fr { f } else { e };
        crate::brand::section(ui, t("Langue et devise", "Language and currency"));
        crate::brand::card(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let mut saved = Ok(());
            egui::Grid::new("language_currency")
                .num_columns(2)
                .spacing([16.0, 10.0])
                .show(ui, |ui| {
                    ui.label(t("Langue de l’interface", "Interface language"));
                    if language_picker(ui, "interface_language", &mut self.language) {
                        saved = self
                            .store
                            .set_preference("ui.language", self.language.code());
                    }
                    ui.end_row();
                    ui.label(t(
                        "Langue des nouveaux documents",
                        "Language of new documents",
                    ));
                    if language_picker(
                        ui,
                        "document_language_default",
                        &mut self.default_document_language,
                    ) {
                        saved = self.store.set_preference(
                            "ui.document_language",
                            self.default_document_language.code(),
                        );
                    }
                    ui.end_row();
                    ui.label(t(
                        "Devise des nouveaux documents",
                        "Currency of new documents",
                    ));
                    if currency_picker(ui, "currency_default", &mut self.default_currency, fr) {
                        saved = self
                            .store
                            .set_preference("ui.currency", &self.default_currency);
                    }
                    ui.end_row();
                });
            ui.weak(t(
                "Chaque document garde sa propre langue et sa devise, modifiables dans l’éditeur. Les totaux du tableau de bord restent séparés par devise.",
                "Each document keeps its own language and currency, editable in the editor. Dashboard totals stay separate per currency.",
            ));
            if let Err(error) = saved {
                self.error = Some(error.to_string());
            }
        });
        ui.add_space(12.0);
    }

    fn apply_theme(&mut self, ctx: &egui::Context, id: &str) {
        crate::brand::set_theme(id);
        crate::brand::apply_style(ctx);
        let image = crate::brand::tint_logo(&self.logo_source);
        let (width, height) = image.dimensions();
        self.logo = ctx.load_texture(
            "facturecraft-mark",
            egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &image),
            egui::TextureOptions::LINEAR,
        );
        ctx.send_viewport_cmd(egui::ViewportCommand::Icon(Some(std::sync::Arc::new(
            egui::IconData {
                rgba: image.into_raw(),
                width,
                height,
            },
        ))));
        if let Err(error) = self.store.set_preference("ui.theme", id) {
            self.error = Some(error.to_string());
        }
    }

    fn appearance_settings(&mut self, ui: &mut egui::Ui, fr: bool) {
        let t = |f, e| if fr { f } else { e };
        crate::brand::section(ui, t("Apparence", "Appearance"));
        crate::brand::card(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(t(
                "Couleur de l’interface et de l’icône",
                "Interface and icon colour",
            ));
            let mut chosen = None;
            ui.horizontal_wrapped(|ui| {
                for theme in &crate::brand::THEMES {
                    let selected = crate::brand::theme().id == theme.id;
                    let (rect, response) =
                        ui.allocate_exact_size(egui::vec2(86.0, 70.0), egui::Sense::click());
                    let name = if fr { theme.fr } else { theme.en };
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, name)
                    });
                    let painter = ui.painter();
                    if selected || response.hovered() {
                        painter.rect_filled(
                            rect,
                            14.0,
                            egui::Color32::from_white_alpha(if selected { 230 } else { 140 }),
                        );
                    }
                    let center = egui::pos2(rect.center().x, rect.top() + 24.0);
                    painter.circle_filled(center, 15.0, crate::brand::swatch(theme));
                    if selected {
                        painter.circle_stroke(
                            center,
                            19.0,
                            egui::Stroke::new(2.0, crate::brand::swatch(theme)),
                        );
                    }
                    painter.text(
                        egui::pos2(rect.center().x, rect.bottom() - 13.0),
                        egui::Align2::CENTER_CENTER,
                        name,
                        egui::FontId::proportional(13.0),
                        INK,
                    );
                    if response
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                        && !selected
                    {
                        chosen = Some(theme.id);
                    }
                }
            });
            if let Some(id) = chosen {
                self.apply_theme(ui.ctx(), id);
            }
            ui.add_space(4.0);
            if ui
                .checkbox(
                    &mut self.page_turn,
                    t(
                        "Animation de page tournée entre les vues",
                        "Page-turn animation between views",
                    ),
                )
                .changed()
                && let Err(error) = self
                    .store
                    .set_preference("ui.page_turn", if self.page_turn { "on" } else { "off" })
            {
                self.error = Some(error.to_string());
            }
        });
        ui.add_space(12.0);
    }

    fn issuer_settings(&mut self, ui: &mut egui::Ui, fr: bool) {
        let t = |f, e| if fr { f } else { e };
        crate::brand::section(ui, t("Identité de l’émetteur", "Issuer details"));
        ui.label(t("Ces informations figurent sur vos devis et factures. Elles sont figées dans chaque document au moment de sa finalisation.", "These details appear on your quotes and invoices and are frozen into each document when it is finalized."));
        crate::brand::card(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                if self.logo_texture.is_none() {
                    let texture = self.store.logo().ok().flatten().and_then(|png| {
                        let image = image::load_from_memory(&png).ok()?.into_rgba8();
                        let size = [image.width() as usize, image.height() as usize];
                        Some(ui.ctx().load_texture(
                            "issuer-logo",
                            egui::ColorImage::from_rgba_unmultiplied(size, &image),
                            egui::TextureOptions::LINEAR,
                        ))
                    });
                    self.logo_texture = Some(texture);
                }
                match self.logo_texture.as_ref().and_then(Option::as_ref) {
                    Some(texture) => {
                        let size = texture.size_vec2();
                        let scale = (72.0 / size.x.max(size.y)).min(1.0);
                        ui.image((texture.id(), size * scale));
                    }
                    None => {
                        ui.weak(t("Aucun logo", "No logo"));
                    }
                }
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(t("Logo de l’entreprise", "Company logo")).strong(),
                    );
                    ui.weak(t(
                        "PNG ou JPEG, affiché en haut à droite des documents.",
                        "PNG or JPEG, shown at the top right of documents.",
                    ));
                    ui.horizontal(|ui| {
                        if ui.button(t("Choisir un logo…", "Choose a logo…")).clicked()
                            && let Some(path) = rfd::FileDialog::new()
                                .add_filter("Image", &["png", "jpg", "jpeg"])
                                .pick_file()
                        {
                            let saved = std::fs::read(&path)
                                .map_err(anyhow::Error::from)
                                .and_then(|bytes| crate::pdf::prepare_logo(&bytes))
                                .and_then(|png| self.store.save_logo(&png));
                            match saved {
                                Ok(()) => {
                                    self.logo_texture = None;
                                    self.error = None;
                                    self.notice = Some(t("Logo enregistré.", "Logo saved.").into());
                                }
                                Err(error) => {
                                    self.error = Some(format!(
                                        "{} {error}",
                                        t("Logo illisible :", "Unreadable logo:")
                                    ))
                                }
                            }
                        }
                        if matches!(self.logo_texture, Some(Some(_)))
                            && ui.button(t("Retirer le logo", "Remove logo")).clicked()
                        {
                            match self.store.clear_logo() {
                                Ok(()) => self.logo_texture = None,
                                Err(error) => self.error = Some(error.to_string()),
                            }
                        }
                    });
                });
            });
            ui.separator();
            egui::Grid::new("issuer_fields")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    for (label, field, rows) in [
                        (t("Société", "Company"), &mut self.issuer.name, 1),
                        (
                            t("Votre contact", "Contact person"),
                            &mut self.issuer.contact,
                            1,
                        ),
                        (t("Adresse", "Address"), &mut self.issuer.address, 2),
                        (t("Pays", "Country"), &mut self.issuer.country, 1),
                        (
                            t("N° SIREN", "Company number"),
                            &mut self.issuer.registration,
                            1,
                        ),
                        (t("Numéro de TVA", "VAT number"), &mut self.issuer.vat_id, 1),
                        (t("Téléphone", "Phone"), &mut self.issuer.phone, 1),
                        (t("E-mail", "Email"), &mut self.issuer.email, 1),
                    ] {
                        ui.label(label);
                        let edit = if rows > 1 {
                            crate::brand::area(field).desired_rows(rows)
                        } else {
                            crate::brand::field(field)
                        };
                        ui.add(edit.desired_width(ui.available_width()));
                        ui.end_row();
                    }
                });
        });
        ui.add_space(12.0);
        crate::brand::section(ui, t("Conditions par défaut", "Default conditions"));
        crate::brand::card(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.weak(t(
                "Reprises dans chaque nouveau document, puis modifiables document par document.",
                "Copied into each new document, then editable per document.",
            ));
            egui::Grid::new("default_terms")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    for (label, field) in [
                        (
                            t("Conditions de règlement", "Payment terms"),
                            &mut self.issuer.payment_terms,
                        ),
                        (
                            t("Mode de règlement", "Payment method"),
                            &mut self.issuer.payment_method,
                        ),
                        (
                            t("Validité du devis", "Quote validity"),
                            &mut self.issuer.quote_validity,
                        ),
                        (
                            t("Mentions complémentaires", "Additional notes"),
                            &mut self.issuer.terms,
                        ),
                    ] {
                        ui.label(label);
                        ui.add(crate::brand::field(field).desired_width(ui.available_width()));
                        ui.end_row();
                    }
                });
            ui.checkbox(
                &mut self.issuer.cgv_on_invoices,
                t(
                    "Imprimer aussi les CGV sur les factures",
                    "Also print the terms of sale on invoices",
                ),
            );
            ui.label(t(
                "Mention de pénalités de retard (factures)",
                "Late-payment clause (invoices)",
            ));
            ui.add(
                crate::brand::area(&mut self.issuer.late_penalties)
                    .hint_text(facturecraft_core::issuer::DEFAULT_LATE_PENALTIES)
                    .desired_rows(3)
                    .desired_width(f32::INFINITY),
            );
            ui.label(t(
                "Conditions générales de vente (page dédiée des devis, copiées dans chaque nouveau document)",
                "General terms of sale (dedicated page of quotes, copied into each new document)",
            ));
            ui.add(
                crate::brand::area(&mut self.issuer.cgv)
                    .desired_rows(8)
                    .desired_width(f32::INFINITY),
            );
        });
        ui.add_space(8.0);
        if ui
            .add(crate::brand::primary(t(
                "Enregistrer les paramètres",
                "Save settings",
            )))
            .clicked()
        {
            match self.store.save_issuer(&self.issuer) {
                Ok(()) => {
                    self.notice = Some(t("Paramètres enregistrés.", "Settings saved.").into());
                    self.error = None;
                }
                Err(error) => self.error = Some(error.to_string()),
            }
        }
        ui.add_space(18.0);
    }

    fn numbering_settings(&mut self, ui: &mut egui::Ui, fr: bool) {
        let t = |f, e| if fr { f } else { e };
        crate::brand::section(ui, t("Numérotation et émission", "Numbering and issuance"));
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
                        ui.label(egui::RichText::new(number).strong().color(accent()));
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
