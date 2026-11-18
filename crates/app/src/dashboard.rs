use crate::brand::{self, AMBER, INK, MUTED, accent, soft};
use eframe::egui::{self, RichText};
use facturecraft_core::dashboard::Dashboard;

pub enum Action {
    Year(i32),
    Currency(String),
}

pub fn show(ui: &mut egui::Ui, data: &Dashboard, fr: bool) -> Option<Action> {
    let t = |f: &'static str, e: &'static str| if fr { f } else { e };
    let mut action = None;
    brand::page_header(
        ui,
        t("Tableau de bord", "Dashboard"),
        t(
            "Vue d’ensemble de votre activité",
            "Overview of your activity",
        ),
        |ui| {
            if ui
                .button("▶")
                .on_hover_text(t("Année suivante", "Next year"))
                .clicked()
            {
                action = Some(Action::Year(data.year + 1));
            }
            ui.label(RichText::new(data.year.to_string()).size(18.0).strong());
            if ui
                .button("◀")
                .on_hover_text(t("Année précédente", "Previous year"))
                .clicked()
            {
                action = Some(Action::Year(data.year - 1));
            }
            if data.currencies.len() > 1 {
                egui::ComboBox::from_id_salt("dashboard_currency")
                    .selected_text(&data.currency)
                    .show_ui(ui, |ui| {
                        for currency in &data.currencies {
                            if ui
                                .selectable_label(*currency == data.currency, currency)
                                .clicked()
                            {
                                action = Some(Action::Currency(currency.clone()));
                            }
                        }
                    });
            }
        },
    );

    let money = |amount: &str| brand::money(amount, &data.currency, fr);
    let year = data.year.to_string();
    let tiles = [
        (
            data.quotes.to_string(),
            format!("{} {year}", t("Devis émis en", "Quotes issued in")),
        ),
        (
            data.invoices.to_string(),
            format!("{} {year}", t("Factures émises en", "Invoices issued in")),
        ),
        (
            money(&data.revenue_month),
            t("CA HT ce mois", "Revenue this month (excl. tax)").to_string(),
        ),
        (
            money(&data.outstanding),
            t("En attente de règlement", "Awaiting payment").to_string(),
        ),
    ];
    let columns = if ui.available_width() >= 760.0 { 4 } else { 2 };
    let width = (ui.available_width() - 12.0 * (columns as f32 - 1.0)) / columns as f32;
    egui::Grid::new("kpis")
        .num_columns(columns)
        .spacing([12.0, 12.0])
        .show(ui, |ui| {
            for (index, (value, label)) in tiles.iter().enumerate() {
                brand::card(ui, |ui| {
                    ui.set_width((width - 34.0).max(80.0));
                    ui.vertical(|ui| {
                        ui.label(RichText::new(value).size(24.0).strong().color(INK));
                        ui.label(RichText::new(label).color(MUTED));
                    });
                });
                if index % columns == columns - 1 {
                    ui.end_row();
                }
            }
        });
    ui.add_space(12.0);

    let full = (ui.available_width() - 34.0).max(80.0);
    brand::card(ui, |ui| {
        ui.set_width(full);
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                brand::card_title(ui, t("Chiffre d’affaires HT", "Revenue (excl. tax)"));
                ui.label(
                    RichText::new(money(&data.revenue_year))
                        .strong()
                        .color(accent()),
                );
            });
            chart(ui, data, fr);
        });
    });
    ui.add_space(12.0);
    brand::card(ui, |ui| {
        ui.set_width(full);
        ui.vertical(|ui| {
            brand::card_title(ui, t("Dernières activités", "Recent activity"));
            ui.add_space(6.0);
            if data.activity.is_empty() {
                ui.label(
                    RichText::new(t("Aucune activité pour l’instant.", "No activity yet."))
                        .color(MUTED),
                );
            }
            for item in &data.activity {
                activity(ui, item, &data.currency, fr);
            }
        });
    });
    ui.add_space(8.0);
    ui.label(
        RichText::new(t(
            "Factures moins avoirs, hors acomptes. Les documents de démonstration ne sont pas comptés ; le reste dû ne couvre que les documents émis dans FactureCraft.",
            "Invoices minus credit notes, excluding deposits. Demonstration documents are not counted; outstanding amounts cover documents issued in FactureCraft only.",
        ))
        .size(12.0)
        .color(MUTED),
    );
    action
}

fn chart(ui: &mut egui::Ui, data: &Dashboard, fr: bool) {
    let months = if fr {
        [
            "Jan", "Fév", "Mar", "Avr", "Mai", "Juin", "Juil", "Août", "Sep", "Oct", "Nov", "Déc",
        ]
    } else {
        [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ]
    };
    let values: Vec<f64> = data
        .monthly
        .iter()
        .map(|v| v.parse().unwrap_or(0.0))
        .collect();
    let max = values.iter().cloned().fold(0.0, f64::max).max(1.0);
    let (response, painter) = ui.allocate_painter(
        egui::vec2(ui.available_width(), 220.0),
        egui::Sense::hover(),
    );
    let plot = response.rect.shrink2(egui::vec2(14.0, 0.0));
    let (top, bottom) = (plot.top() + 26.0, plot.bottom() - 26.0);
    let step = plot.width() / 11.0;
    let points: Vec<egui::Pos2> = values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            egui::pos2(
                plot.left() + step * i as f32,
                bottom - (v / max) as f32 * (bottom - top),
            )
        })
        .collect();
    for fraction in [0.0, 0.5, 1.0] {
        let y = bottom - fraction * (bottom - top);
        painter.line_segment(
            [egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)],
            egui::Stroke::new(1.0, brand::border()),
        );
    }
    // ponytail: one polygon per segment keeps the fill convex for egui's tessellator.
    for pair in points.windows(2) {
        painter.add(egui::Shape::convex_polygon(
            vec![
                pair[0],
                pair[1],
                egui::pos2(pair[1].x, bottom),
                egui::pos2(pair[0].x, bottom),
            ],
            soft().gamma_multiply(0.55),
            egui::Stroke::NONE,
        ));
    }
    painter.add(egui::Shape::line(
        points.clone(),
        egui::Stroke::new(2.5, accent()),
    ));
    let hovered = response
        .hover_pos()
        .map(|pos| (((pos.x - plot.left()) / step).round().clamp(0.0, 11.0)) as usize);
    for (i, point) in points.iter().enumerate() {
        painter.circle_filled(*point, if hovered == Some(i) { 5.0 } else { 3.0 }, accent());
        painter.text(
            egui::pos2(point.x, plot.bottom() - 8.0),
            egui::Align2::CENTER_CENTER,
            months[i],
            egui::FontId::proportional(12.0),
            MUTED,
        );
    }
    if let Some(i) = hovered {
        let text = format!(
            "{} · {}",
            months[i],
            brand::money(&data.monthly[i], &data.currency, fr)
        );
        let galley =
            painter.layout_no_wrap(text, egui::FontId::proportional(13.0), egui::Color32::WHITE);
        let mut rect = egui::Rect::from_center_size(
            points[i] - egui::vec2(0.0, 22.0),
            galley.size() + egui::vec2(14.0, 8.0),
        );
        rect = rect.translate(egui::vec2(
            (plot.left() - rect.left()).max(0.0) + (plot.right() - rect.right()).min(0.0),
            (response.rect.top() - rect.top()).max(0.0),
        ));
        painter.rect_filled(rect, 6.0, INK);
        painter.galley(
            rect.center() - galley.size() / 2.0,
            galley,
            egui::Color32::WHITE,
        );
    }
}

fn activity(
    ui: &mut egui::Ui,
    item: &facturecraft_core::dashboard::Activity,
    currency: &str,
    fr: bool,
) {
    let t = |f: &'static str, e: &'static str| if fr { f } else { e };
    let (icon, title, color) = match item.kind.as_str() {
        "quote" => ("🗋", t("Devis", "Quote"), accent()),
        "deposit" => ("🗐", t("Acompte", "Deposit"), accent()),
        "credit" => ("↺", t("Avoir", "Credit note"), accent()),
        "payment" => ("✔", t("Règlement reçu", "Payment received"), AMBER),
        _ => ("🗐", t("Facture", "Invoice"), accent()),
    };
    let mut detail = brand::money(&item.amount, currency, fr);
    if !item.detail.is_empty() {
        detail = format!("{} · {detail}", item.detail);
    }
    ui.horizontal(|ui| {
        egui::Frame::new()
            .fill(soft())
            .corner_radius(16)
            .inner_margin(egui::Margin::symmetric(9, 6))
            .show(ui, |ui| ui.label(RichText::new(icon).color(color)));
        ui.label(
            RichText::new(format!("{title} {}", item.number))
                .strong()
                .color(INK),
        );
        ui.label(RichText::new(detail).color(MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(date(&item.date, fr)).size(12.5).color(MUTED));
        });
    });
    ui.add_space(4.0);
}

fn date(iso: &str, fr: bool) -> String {
    if fr && iso.len() == 10 {
        format!("{}/{}/{}", &iso[8..], &iso[5..7], &iso[..4])
    } else {
        iso.to_string()
    }
}
