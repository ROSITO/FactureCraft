use anyhow::Result;
use facturecraft_core::quote::Quote;
use genpdf::{
    Alignment, Document, SimplePageDecorator,
    elements::{Break, Paragraph},
    fonts::{FontData, FontFamily},
    style::{Color, Style},
};

const FONT: &[u8] = include_bytes!("../../../assets/fonts/NotoSans-Regular.ttf");
const PLUM: Color = Color::Rgb(112, 36, 112);

pub fn kind_label(kind: &str, language: &str) -> &'static str {
    match (kind, language) {
        ("invoice", "fr") => "FACTURE",
        ("invoice", _) => "INVOICE",
        ("deposit", "fr") => "FACTURE D’ACOMPTE",
        ("deposit", _) => "DEPOSIT INVOICE",
        ("credit", "fr") => "AVOIR",
        ("credit", _) => "CREDIT NOTE",
        (_, "fr") => "DEVIS",
        _ => "QUOTE",
    }
}

pub fn draft_label(kind: &str, language: &str) -> String {
    format!(
        "{} · {}",
        kind_label(kind, language),
        if language == "fr" {
            "BROUILLON"
        } else {
            "DRAFT"
        }
    )
}

#[derive(Default)]
pub struct DocumentContext<'a> {
    pub issuer: Option<&'a facturecraft_core::issuer::Issuer>,
    pub applied_deposit: Option<(&'a str, &'a str)>,
    /// Issue date `YYYY-MM-DD`, printed on finalized documents.
    pub issued_on: Option<&'a str>,
}

pub fn render(
    label: &str,
    title: &str,
    currency: &str,
    language: &str,
    quote: &Quote,
    context: DocumentContext<'_>,
) -> Result<Vec<u8>> {
    let fr = language == "fr";
    let (net, tax, total) = quote.totals(currency)?;
    let currency = currency.trim().to_uppercase();
    let regular = FontData::new(FONT.to_vec(), None)?;
    let mut document = Document::new(FontFamily {
        regular: regular.clone(),
        bold: regular.clone(),
        italic: regular.clone(),
        bold_italic: regular,
    });
    document.set_title(title);
    document.set_paper_size(genpdf::Size::new(210, 297));
    document.set_font_size(10);
    document.set_line_spacing(1.25);
    let mut pages = SimplePageDecorator::new();
    pages.set_margins(18);
    let running_header = label.to_owned();
    pages.set_header(move |_| {
        let mut header = genpdf::elements::LinearLayout::vertical();
        header.push(
            Paragraph::default().styled_string(&running_header, Style::new().with_color(PLUM)),
        );
        header.push(Break::new(1.0));
        header
    });
    document.set_page_decorator(pages);
    document.push(
        Paragraph::default().styled_string(title, Style::new().with_font_size(22).with_color(PLUM)),
    );
    if let Some(date) = context.issued_on.filter(|date| date.len() == 10) {
        document.push(Paragraph::new(if fr {
            format!(
                "Date d’émission : {}/{}/{}",
                &date[8..],
                &date[5..7],
                &date[..4]
            )
        } else {
            format!("Issue date: {date}")
        }));
    }
    document.push(Break::new(1.0));
    if let Some(issuer) = context
        .issuer
        .filter(|issuer| !issuer.name.trim().is_empty())
    {
        document.push(Paragraph::default().styled_string(
            if fr { "Émetteur" } else { "Issuer" },
            Style::new().with_color(PLUM),
        ));
        for field in [
            &issuer.name,
            &issuer.address,
            &issuer.registration,
            &issuer.vat_id,
            &issuer.email,
        ] {
            for line in field.lines().filter(|line| !line.trim().is_empty()) {
                document.push(Paragraph::new(line));
            }
        }
        document.push(Break::new(1.0));
    }
    document.push(Paragraph::default().styled_string(
        if fr { "Destinataire" } else { "Recipient" },
        Style::new().with_color(PLUM),
    ));
    for field in [
        &quote.recipient,
        &quote.recipient_address,
        &quote.recipient_email,
    ] {
        for line in field.lines().filter(|line| !line.trim().is_empty()) {
            document.push(Paragraph::new(line));
        }
    }
    if !quote.valid_until.trim().is_empty() {
        document.push(Paragraph::new(format!(
            "{} {}",
            if fr {
                "Valable jusqu’au"
            } else {
                "Valid until"
            },
            quote.valid_until
        )));
    }
    document.push(Break::new(1.0));
    document.push(Paragraph::default().styled_string(
        if fr { "Prestations" } else { "Line items" },
        Style::new().with_font_size(14).with_color(PLUM),
    ));
    for (index, line) in quote.lines.iter().enumerate() {
        document.push(Break::new(0.55));
        let mut description = line.description.lines();
        if let Some(first) = description.next() {
            document.push(Paragraph::new(format!("{}. {first}", index + 1)));
        }
        for text in description {
            document.push(Paragraph::new(text));
        }
        document.push(Paragraph::new(format!(
            "{} {} × {} {}   |   {} {} %   |   {} {} %",
            line.quantity,
            line.unit,
            line.price,
            currency,
            if fr { "Remise" } else { "Discount" },
            line.discount,
            if fr { "TVA" } else { "Tax" },
            line.vat,
        )));
    }
    document.push(Break::new(1.25));
    if !quote.global_discount.trim().is_empty() && quote.global_discount.trim() != "0" {
        document.push(Paragraph::new(format!(
            "{} {} %",
            if fr {
                "Remise générale"
            } else {
                "Overall discount"
            },
            quote.global_discount
        )));
    }
    for bucket in quote.tax_breakdown(&currency)? {
        if bucket.exemption_reason.is_empty() {
            document.push(Paragraph::new(format!(
                "{} {} % : {} {}  ·  {} {}",
                if fr { "TVA" } else { "Tax" },
                bucket.rate,
                bucket.tax,
                currency,
                bucket.net,
                currency
            )));
        } else {
            document.push(Paragraph::new(format!(
                "{} : {}",
                if fr { "Exonération" } else { "Exemption" },
                bucket.exemption_reason
            )));
        }
    }
    for (label, amount) in [
        (if fr { "Total HT" } else { "Subtotal" }, net),
        (if fr { "TVA" } else { "Tax" }, tax),
        (if fr { "Total TTC" } else { "Total" }, total),
    ] {
        let mut line = Paragraph::default().styled_string(
            format!("{label}   {amount} {currency}"),
            Style::new().with_font_size(12).with_color(PLUM),
        );
        line.set_alignment(Alignment::Right);
        document.push(line);
    }
    if let Some((applied, outstanding)) =
        context.applied_deposit.filter(|(amount, _)| *amount != "0")
    {
        for (label, amount) in [
            (
                if fr {
                    "Acomptes imputés"
                } else {
                    "Deposits applied"
                },
                applied,
            ),
            (if fr { "Net à payer" } else { "Balance due" }, outstanding),
        ] {
            let mut line = Paragraph::new(format!("{label}   {amount} {currency}"));
            line.set_alignment(Alignment::Right);
            document.push(line);
        }
    }
    if !quote.terms.trim().is_empty() {
        document.push(Break::new(1.25));
        document.push(Paragraph::default().styled_string(
            if fr { "Conditions" } else { "Terms" },
            Style::new().with_color(PLUM),
        ));
        for line in quote.terms.lines() {
            document.push(Paragraph::new(line));
        }
    }
    let mut bytes = Vec::new();
    document.render(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use facturecraft_core::quote::Line;

    #[test]
    fn renders_a_long_french_draft() {
        let quote = Quote {
            recipient: "Éditions de démonstration".into(),
            lines: (0..75)
                .map(|number| Line {
                    description: format!(
                        "Prestation n° {number} — création, révision et préparation"
                    ),
                    quantity: "1,5".into(),
                    price: "19,99".into(),
                    discount: "10".into(),
                    vat: "20".into(),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };
        let bytes = render(
            &draft_label("quote", "fr"),
            "Devis de démonstration",
            "EUR",
            "fr",
            &quote,
            DocumentContext {
                issued_on: Some("2026-11-18"),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        assert!(bytes.windows(5).any(|bytes| bytes == b"%%EOF"));
    }
}
