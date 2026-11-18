//! Brand kit: colours, cards, status badges and amount formatting.
use eframe::egui::{self, Color32, RichText};

pub const PLUM: Color32 = Color32::from_rgb(116, 36, 116);
pub const CREAM: Color32 = Color32::from_rgb(252, 244, 234);
pub const PAPER: Color32 = Color32::from_rgb(255, 252, 247);
pub const INK: Color32 = Color32::from_rgb(40, 31, 43);
pub const MUTED: Color32 = Color32::from_rgb(112, 100, 114);
pub const LILAC: Color32 = Color32::from_rgb(234, 213, 244);
pub const AMBER: Color32 = Color32::from_rgb(255, 178, 34);
pub const BORDER: Color32 = Color32::from_rgb(236, 224, 230);

#[derive(Clone, Copy)]
pub enum Tone {
    Positive,
    Sent,
    Pending,
    Draft,
}

pub fn badge(ui: &mut egui::Ui, text: &str, tone: Tone) {
    let (fill, fg) = match tone {
        Tone::Positive => (
            Color32::from_rgb(229, 243, 220),
            Color32::from_rgb(52, 112, 38),
        ),
        Tone::Sent => (
            Color32::from_rgb(237, 228, 250),
            Color32::from_rgb(92, 66, 166),
        ),
        Tone::Pending => (
            Color32::from_rgb(255, 241, 212),
            Color32::from_rgb(150, 92, 0),
        ),
        Tone::Draft => (
            Color32::from_rgb(238, 235, 236),
            Color32::from_rgb(88, 84, 88),
        ),
    };
    egui::Frame::new()
        .fill(fill)
        .corner_radius(10)
        .inner_margin(egui::Margin::symmetric(9, 3))
        .show(ui, |ui| ui.label(RichText::new(text).size(12.5).color(fg)));
}

/// Frosted card: translucent white over the gradient, light rim and soft shadow.
pub fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(255, 255, 255, 214))
        .stroke(egui::Stroke::new(
            1.0,
            Color32::from_rgba_unmultiplied(255, 255, 255, 235),
        ))
        .corner_radius(18)
        .inner_margin(18)
        .shadow(egui::Shadow {
            offset: [0, 6],
            blur: 24,
            spread: 0,
            color: Color32::from_rgba_unmultiplied(74, 32, 84, 22),
        })
        .show(ui, add)
        .inner
}

pub fn primary(text: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text).color(Color32::WHITE).strong())
        .fill(PLUM)
        .corner_radius(20)
}

/// One-line input, taller than egui's default.
pub fn field(text: &mut dyn egui::TextBuffer) -> egui::TextEdit<'_> {
    egui::TextEdit::singleline(text).margin(egui::Margin::symmetric(12, 9))
}

pub fn area(text: &mut dyn egui::TextBuffer) -> egui::TextEdit<'_> {
    egui::TextEdit::multiline(text).margin(egui::Margin::symmetric(12, 9))
}

/// Rounded, airy widgets for the whole app.
pub fn apply_style(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::light();
    visuals.panel_fill = Color32::TRANSPARENT;
    visuals.window_fill = Color32::from_rgba_unmultiplied(255, 252, 255, 245);
    visuals.window_stroke = egui::Stroke::new(1.0, Color32::WHITE);
    visuals.window_corner_radius = 20.into();
    visuals.window_shadow = egui::Shadow {
        offset: [0, 12],
        blur: 40,
        spread: 0,
        color: Color32::from_rgba_unmultiplied(58, 22, 64, 50),
    };
    visuals.popup_shadow = visuals.window_shadow;
    visuals.menu_corner_radius = 14.into();
    visuals.selection.bg_fill = LILAC;
    visuals.selection.stroke = egui::Stroke::new(1.5, PLUM);
    visuals.text_edit_bg_color = Some(Color32::from_rgba_unmultiplied(255, 255, 255, 235));
    visuals.extreme_bg_color = Color32::from_rgb(252, 249, 252);
    visuals.hyperlink_color = PLUM;
    let soft = Color32::from_rgba_unmultiplied(255, 255, 255, 200);
    let widgets = &mut visuals.widgets;
    for (state, fill, stroke) in [
        (
            &mut widgets.inactive,
            soft,
            Color32::from_rgb(226, 212, 232),
        ),
        (
            &mut widgets.hovered,
            Color32::WHITE,
            Color32::from_rgb(196, 160, 206),
        ),
        (&mut widgets.active, LILAC, PLUM),
        (
            &mut widgets.open,
            Color32::WHITE,
            Color32::from_rgb(196, 160, 206),
        ),
    ] {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.bg_stroke = egui::Stroke::new(1.0, stroke);
        state.corner_radius = 12.into();
        state.expansion = 0.0;
    }
    widgets.noninteractive.corner_radius = 12.into();
    widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, BORDER);
    ctx.set_visuals(visuals);
    ctx.style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.spacing.button_padding = egui::vec2(16.0, 9.0);
        style.spacing.interact_size.y = 36.0;
        style.spacing.combo_width = 140.0;
        style.animation_time = 0.22;
    });
}

/// Soft cream-to-lilac gradient behind every panel.
pub fn paint_background(ctx: &egui::Context) {
    let rect = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::background());
    let mut mesh = egui::Mesh::default();
    let top = Color32::from_rgb(253, 246, 238);
    let bottom = Color32::from_rgb(243, 232, 247);
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), Color32::from_rgb(250, 240, 246));
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.colored_vertex(rect.left_bottom(), Color32::from_rgb(249, 238, 236));
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(mesh);
}

/// `"3840.5"` → `3 840,50 €` (fr) or `€3,840.50` (en).
pub fn money(amount: &str, currency: &str, fr: bool) -> String {
    let amount = amount.trim();
    let (sign, digits) = amount
        .strip_prefix('-')
        .map_or(("", amount), |rest| ("-", rest));
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    let fraction = format!("{fraction:0<2}");
    let separator = if fr { '\u{a0}' } else { ',' };
    let mut grouped = String::new();
    for (index, digit) in whole.chars().enumerate() {
        if index > 0 && (whole.len() - index) % 3 == 0 {
            grouped.push(separator);
        }
        grouped.push(digit);
    }
    let symbol = match currency.to_ascii_uppercase().as_str() {
        "EUR" => "€".to_string(),
        "USD" => "$".to_string(),
        "GBP" => "£".to_string(),
        other => other.to_string(),
    };
    if fr {
        format!("{sign}{grouped},{fraction}\u{a0}{symbol}")
    } else if symbol.chars().count() == 1 {
        format!("{sign}{symbol}{grouped}.{fraction}")
    } else {
        format!("{sign}{grouped}.{fraction} {symbol}")
    }
}

/// Glass pill behind the selected sidebar entry; glides to its new position.
pub fn nav_indicator(ui: &egui::Ui, target: egui::Rect) {
    let ctx = ui.ctx();
    let id = egui::Id::new("nav_indicator");
    let top = ctx.animate_value_with_time(id, target.top(), 0.28);
    let rect = egui::Rect::from_min_size(egui::pos2(target.left(), top), target.size());
    let painter = ui.painter();
    painter.add(
        egui::epaint::RectShape::filled(
            rect.translate(egui::vec2(0.0, 3.0)),
            14.0,
            Color32::from_black_alpha(40),
        )
        .with_blur_width(10.0),
    );
    painter.rect_filled(
        rect,
        14.0,
        Color32::from_rgba_unmultiplied(146, 58, 150, 230),
    );
    // Glass highlight on the upper half.
    let mut gloss = rect;
    gloss.set_height(rect.height() * 0.5);
    painter.rect_filled(
        gloss,
        egui::CornerRadius {
            nw: 14,
            ne: 14,
            sw: 0,
            se: 0,
        },
        Color32::from_white_alpha(22),
    );
    painter.rect_stroke(
        rect,
        14.0,
        egui::Stroke::new(1.0, Color32::from_white_alpha(60)),
        egui::StrokeKind::Inside,
    );
}

/// Sidebar entry: icon centred in a fixed column, label left-aligned after it.
pub fn nav_item(ui: &mut egui::Ui, icon: &str, label: &str, selected: bool) -> egui::Response {
    let size = egui::vec2(ui.available_width(), 40.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let enabled = ui.is_enabled();
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, selected, label)
    });
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let hover = ui.ctx().animate_bool_with_time(
            response.id.with("hover"),
            enabled && response.hovered() && !selected,
            0.15,
        );
        if hover > 0.0 {
            painter.rect_filled(rect, 14.0, Color32::from_white_alpha((22.0 * hover) as u8));
        }
        let color = if enabled {
            CREAM
        } else {
            CREAM.gamma_multiply(0.45)
        };
        painter.text(
            egui::pos2(rect.left() + 22.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(16.0),
            color,
        );
        painter.text(
            egui::pos2(rect.left() + 44.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(14.5),
            color,
        );
    }
    if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn formats_amounts() {
        assert_eq!(super::money("3840.5", "EUR", true), "3\u{a0}840,50\u{a0}€");
        assert_eq!(super::money("-1234567.00", "EUR", false), "-€1,234,567.00");
        assert_eq!(super::money("12", "CHF", false), "12.00 CHF");
    }
}
