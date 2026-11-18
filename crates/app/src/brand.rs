//! Brand kit: colours, cards, status badges and amount formatting.
use eframe::egui::{self, Color32, RichText};

pub const PLUM: Color32 = Color32::from_rgb(116, 36, 116);
pub const DEEP_PLUM: Color32 = Color32::from_rgb(58, 22, 64);
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

/// White rounded card used across pages.
pub fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(Color32::WHITE)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(12)
        .inner_margin(16)
        .show(ui, add)
        .inner
}

pub fn primary(text: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text).color(Color32::WHITE).strong())
        .fill(PLUM)
        .corner_radius(8)
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

#[cfg(test)]
mod tests {
    #[test]
    fn formats_amounts() {
        assert_eq!(super::money("3840.5", "EUR", true), "3\u{a0}840,50\u{a0}€");
        assert_eq!(super::money("-1234567.00", "EUR", false), "-€1,234,567.00");
        assert_eq!(super::money("12", "CHF", false), "12.00 CHF");
    }
}
