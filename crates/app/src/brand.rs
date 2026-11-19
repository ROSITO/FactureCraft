//! Brand kit: colours, cards, status badges and amount formatting.
use eframe::egui::{self, Color32, RichText};

pub const CREAM: Color32 = Color32::from_rgb(252, 244, 234);
pub const PAPER: Color32 = Color32::from_rgb(255, 252, 247);
pub const INK: Color32 = Color32::from_rgb(40, 31, 43);
pub const MUTED: Color32 = Color32::from_rgb(112, 100, 114);
pub const AMBER: Color32 = Color32::from_rgb(255, 178, 34);

pub struct Theme {
    pub id: &'static str,
    pub fr: &'static str,
    pub en: &'static str,
    accent: [u8; 3],
    deep: [u8; 3],
}

/// Every interface colour derives from `accent` (actions, selection, tints) and
/// `deep` (sidebar). Status badges keep their own meaning-based colours.
pub const THEMES: [Theme; 5] = [
    Theme {
        id: "plum",
        fr: "Prune",
        en: "Plum",
        accent: [116, 36, 116],
        deep: [58, 22, 64],
    },
    Theme {
        id: "ocean",
        fr: "Océan",
        en: "Ocean",
        accent: [30, 94, 160],
        deep: [16, 38, 74],
    },
    Theme {
        id: "forest",
        fr: "Forêt",
        en: "Forest",
        accent: [36, 116, 82],
        deep: [17, 50, 39],
    },
    Theme {
        id: "coral",
        fr: "Corail",
        en: "Coral",
        accent: [192, 74, 70],
        deep: [76, 30, 34],
    },
    Theme {
        id: "slate",
        fr: "Ardoise",
        en: "Slate",
        accent: [72, 82, 102],
        deep: [30, 34, 44],
    },
];

static CURRENT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub fn set_theme(id: &str) {
    let index = THEMES.iter().position(|theme| theme.id == id).unwrap_or(0);
    CURRENT.store(index, std::sync::atomic::Ordering::Relaxed);
}

pub fn theme() -> &'static Theme {
    &THEMES[CURRENT.load(std::sync::atomic::Ordering::Relaxed)]
}

fn rgb([r, g, b]: [u8; 3]) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// `t = 0` gives `a`, `t = 1` gives `b`.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let channel = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color32::from_rgb(
        channel(a.r(), b.r()),
        channel(a.g(), b.g()),
        channel(a.b(), b.b()),
    )
}

pub fn with_alpha(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

pub fn accent() -> Color32 {
    rgb(theme().accent)
}

pub fn deep() -> Color32 {
    rgb(theme().deep)
}

/// Pale tint for selections and icon bubbles.
pub fn soft() -> Color32 {
    mix(accent(), Color32::WHITE, 0.84)
}

pub fn border() -> Color32 {
    mix(accent(), Color32::WHITE, 0.88)
}

pub fn swatch(theme: &Theme) -> Color32 {
    rgb(theme.accent)
}

#[derive(Clone, Copy)]
pub enum Tone {
    Positive,
    Sent,
    Pending,
    Draft,
    Closed,
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
        Tone::Closed => (
            Color32::from_rgb(252, 228, 228),
            Color32::from_rgb(168, 40, 52),
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
            color: with_alpha(deep(), 22),
        })
        .show(ui, add)
        .inner
}

pub fn primary(text: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text).color(Color32::WHITE).strong())
        .fill(accent())
        .corner_radius(20)
}

/// Header shared by every page: title, subtitle and actions aligned on the right.
pub fn page_header<R>(
    ui: &mut egui::Ui,
    title: &str,
    subtitle: &str,
    actions: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let mut result = None;
    if ui.available_width() < 600.0 {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(RichText::new(title).size(30.0).strong().color(INK));
            ui.label(RichText::new(subtitle).size(14.0).color(MUTED));
        });
        ui.horizontal(|ui| {
            result = Some(actions(ui));
        });
    } else {
        ui.horizontal(|ui| {
            ui.set_min_height(58.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(RichText::new(title).size(30.0).strong().color(INK));
                ui.label(RichText::new(subtitle).size(14.0).color(MUTED));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                result = Some(actions(ui));
            });
        });
    }
    ui.add_space(16.0);
    result.expect("header actions run once")
}

/// Title of a block inside a page.
pub fn section(ui: &mut egui::Ui, text: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(text).size(18.0).strong().color(INK));
    ui.add_space(2.0);
}

/// Title at the top of a card.
pub fn card_title(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(16.0).strong().color(INK));
}

/// Floating rounded block filling a side panel with a 12 px gap on every side.
/// Drawn inside the panel's own rect so its corners and shadow are never clipped.
pub fn glass_panel<R>(
    ui: &mut egui::Ui,
    fill: Color32,
    radius: u8,
    shadow: Option<Color32>,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let block = ui.max_rect().shrink(12.0);
    let painter = ui.painter();
    if let Some(color) = shadow {
        painter.add(
            egui::Shadow {
                offset: [0, 8],
                blur: 24,
                spread: 0,
                color,
            }
            .as_shape(block, radius),
        );
    }
    painter.rect(
        block,
        radius,
        fill,
        egui::Stroke::new(1.0, Color32::from_white_alpha(40)),
        egui::StrokeKind::Inside,
    );
    ui.scope_builder(egui::UiBuilder::new().max_rect(block.shrink(16.0)), add)
        .inner
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
        color: with_alpha(deep(), 50),
    };
    visuals.popup_shadow = visuals.window_shadow;
    visuals.menu_corner_radius = 14.into();
    visuals.selection.bg_fill = soft();
    visuals.selection.stroke = egui::Stroke::new(1.5, accent());
    visuals.text_edit_bg_color = Some(Color32::from_rgba_unmultiplied(255, 255, 255, 235));
    visuals.extreme_bg_color = Color32::from_rgb(252, 249, 252);
    visuals.hyperlink_color = accent();
    let glass = Color32::from_rgba_unmultiplied(255, 255, 255, 200);
    let widgets = &mut visuals.widgets;
    for (state, fill, stroke) in [
        (
            &mut widgets.inactive,
            glass,
            mix(accent(), Color32::WHITE, 0.8),
        ),
        (
            &mut widgets.hovered,
            Color32::WHITE,
            mix(accent(), Color32::WHITE, 0.55),
        ),
        (&mut widgets.active, soft(), accent()),
        (
            &mut widgets.open,
            Color32::WHITE,
            mix(accent(), Color32::WHITE, 0.55),
        ),
    ] {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.bg_stroke = egui::Stroke::new(1.0, stroke);
        state.corner_radius = 12.into();
        state.expansion = 0.0;
    }
    widgets.noninteractive.corner_radius = 12.into();
    widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, border());
    ctx.set_visuals(visuals);
    ctx.style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.spacing.button_padding = egui::vec2(16.0, 9.0);
        style.spacing.interact_size.y = 36.0;
        style.spacing.combo_width = 140.0;
        style.animation_time = 0.22;
    });
}

fn horizontal_gradient(painter: &egui::Painter, rect: egui::Rect, left: Color32, right: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), left);
    mesh.colored_vertex(rect.right_top(), right);
    mesh.colored_vertex(rect.right_bottom(), right);
    mesh.colored_vertex(rect.left_bottom(), left);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(mesh);
}

/// Page-turn transition over the central view. `progress` runs from 0 to 1.
/// Forward, the sheet turns from right to left and uncovers the new page from the
/// right edge; backward mirrors it. Content is clipped to the uncovered part.
pub fn page_turn(ui: &mut egui::Ui, progress: f32, forward: bool) {
    let eased = if progress < 0.5 {
        2.0 * progress * progress
    } else {
        1.0 - (-2.0 * progress + 2.0).powi(2) / 2.0
    };
    let area = ui.max_rect();
    let fold = if forward {
        area.right() - eased * area.width()
    } else {
        area.left() + eased * area.width()
    };
    let mut clip = ui.clip_rect();
    if forward {
        clip.min.x = clip.min.x.max(fold);
    } else {
        clip.max.x = clip.max.x.min(fold);
    }
    ui.set_clip_rect(clip);

    // The lifted sheet: its back is a curled band beside the fold, widest mid-turn.
    let lift = (eased * std::f32::consts::PI).sin();
    let band = 70.0 * lift + 6.0;
    let shadow = 36.0 * lift + 4.0;
    let painter = ui
        .ctx()
        .layer_painter(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new("page_turn"),
        ))
        .with_clip_rect(area.expand(2.0));
    let paper = Color32::from_rgb(253, 250, 253);
    let crease = mix(accent(), Color32::WHITE, 0.8);
    let dark = Color32::from_black_alpha((60.0 * lift + 12.0) as u8);
    let (top, bottom) = (area.top(), area.bottom());
    let span = |a: f32, b: f32| egui::Rect::from_x_y_ranges(a.min(b)..=a.max(b), top..=bottom);
    if forward {
        horizontal_gradient(&painter, span(fold - band, fold), paper, crease);
        horizontal_gradient(
            &painter,
            span(fold, fold + shadow),
            dark,
            Color32::TRANSPARENT,
        );
        painter.vline(
            fold - band,
            top..=bottom,
            egui::Stroke::new(1.0, Color32::from_white_alpha(200)),
        );
    } else {
        horizontal_gradient(&painter, span(fold, fold + band), crease, paper);
        horizontal_gradient(
            &painter,
            span(fold - shadow, fold),
            Color32::TRANSPARENT,
            dark,
        );
        painter.vline(
            fold + band,
            top..=bottom,
            egui::Stroke::new(1.0, Color32::from_white_alpha(200)),
        );
    }
}

/// Soft cream-to-lilac gradient behind every panel.
pub fn paint_background(ctx: &egui::Context) {
    let rect = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::background());
    let mut mesh = egui::Mesh::default();
    let cream = Color32::from_rgb(253, 246, 238);
    let top = cream;
    let bottom = mix(Color32::WHITE, accent(), 0.10);
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), mix(cream, accent(), 0.05));
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.colored_vertex(rect.left_bottom(), mix(cream, accent(), 0.03));
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
        with_alpha(mix(accent(), Color32::WHITE, 0.15), 230),
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

fn to_hsv(c: [u8; 3]) -> (f32, f32, f32) {
    let [r, g, b] = c.map(|v| f32::from(v) / 255.0);
    let max = r.max(g).max(b);
    let delta = max - r.min(g).min(b);
    let hue = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * ((g - b) / delta).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    (hue, if max == 0.0 { 0.0 } else { delta / max }, max)
}

fn from_hsv(h: f32, s: f32, v: f32) -> [u8; 3] {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match (h / 60.0) as u32 % 6 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [r, g, b].map(|channel| ((channel + v - c) * 255.0).round() as u8)
}

/// Recolours the logo's plum background with the current accent; the squirrel,
/// drawn in low-saturation tones, keeps its colours.
pub fn tint_logo(image: &image::RgbaImage) -> image::RgbaImage {
    let (base_h, base_s, base_v) = to_hsv(THEMES[0].accent);
    let (h, s, v) = to_hsv(theme().accent);
    let mut tinted = image.clone();
    if theme().id == THEMES[0].id {
        return tinted;
    }
    for pixel in tinted.pixels_mut() {
        let [r, g, b, a] = pixel.0;
        let (ph, ps, pv) = to_hsv([r, g, b]);
        let distance = (ph - base_h).abs().min(360.0 - (ph - base_h).abs());
        if ps > 0.25 && distance < 50.0 {
            let rgb = from_hsv(h, (ps * s / base_s).min(1.0), (pv * v / base_v).min(1.0));
            pixel.0 = [rgb[0], rgb[1], rgb[2], a];
        }
    }
    tinted
}

#[cfg(test)]
mod theme_tests {
    use super::*;

    #[test]
    fn logo_background_follows_the_theme() {
        let image = image::RgbaImage::from_fn(2, 1, |x, _| {
            if x == 0 {
                image::Rgba([116, 36, 116, 255])
            } else {
                image::Rgba([200, 180, 150, 255])
            }
        });
        set_theme("ocean");
        let tinted = tint_logo(&image);
        set_theme("plum");
        let [r, g, b, _] = tinted.get_pixel(0, 0).0;
        assert!(b > r && b > g, "plum became blue: {r} {g} {b}");
        assert_eq!(
            tinted.get_pixel(1, 0).0,
            [200, 180, 150, 255],
            "sepia is kept"
        );
        assert_eq!(from_hsv(300.0, 0.69, 0.4549), [116, 36, 116]);
    }
}
