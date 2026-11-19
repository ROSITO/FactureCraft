//! A4 document layout: header with logo, issuer and recipient blocks, line table,
//! totals, then conditions and general terms of sale on their own pages.
use anyhow::{Context, Result};
use facturecraft_core::{issuer::Issuer, quote::Quote};
use printpdf::{
    Color, ColorBits, ColorSpace, Image, ImageXObject, Line, Mm, PdfDocument, Point, Px, Rgb,
};
use std::io::BufWriter;

const FONT: &[u8] = include_bytes!("../../../assets/fonts/NotoSans-Regular.ttf");
const BOLD_FONT: &[u8] = include_bytes!("../../../assets/fonts/NotoSans-Bold.ttf");
const PAGE_W: f64 = 210.0;
const PAGE_H: f64 = 297.0;
const LEFT: f64 = 10.0;
const RIGHT: f64 = PAGE_W - 12.0;
/// Lowest baseline for content; the footer sits below.
const BOTTOM: f64 = PAGE_H - 22.0;
const LINE: f64 = 4.6;
/// Noto Sans runs wider than the reference layout; this size keeps the same line breaks.
const TABLE_FONT: f64 = 8.0;

type Rgb8 = (u8, u8, u8);
const INK: Rgb8 = (58, 58, 58);
const LABEL: Rgb8 = (125, 125, 125);
const HEADER: Rgb8 = (72, 72, 72);
const ALTERNATE: Rgb8 = (247, 247, 247);
const RULE: Rgb8 = (226, 226, 226);
const WHITE: Rgb8 = (255, 255, 255);

pub struct Document<'a> {
    /// `quote`, `invoice`, `deposit` or `credit`.
    pub kind: &'a str,
    /// `None` for a draft.
    pub number: Option<&'a str>,
    /// `YYYY-MM-DD`: issue date, or today for a draft.
    pub date: &'a str,
    pub title: &'a str,
    pub currency: &'a str,
    pub language: &'a str,
    pub quote: &'a Quote,
    pub issuer: &'a Issuer,
    /// PNG bytes.
    pub logo: Option<&'a [u8]>,
    /// Deposits applied and balance due, for a final invoice.
    pub applied_deposit: Option<(&'a str, &'a str)>,
}

pub fn kind_name(kind: &str, fr: bool) -> &'static str {
    match (kind, fr) {
        ("invoice", true) => "Facture",
        ("invoice", false) => "Invoice",
        ("deposit", true) => "Facture d’acompte",
        ("deposit", false) => "Deposit invoice",
        ("credit", true) => "Avoir",
        ("credit", false) => "Credit note",
        (_, true) => "Devis",
        _ => "Quote",
    }
}

#[derive(Clone, Copy)]
enum Align {
    Left,
    Right,
}

enum Op {
    Text {
        x: f64,
        y: f64,
        size: f64,
        text: String,
        color: Rgb8,
        bold: bool,
        align: Align,
    },
    Rect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        color: Rgb8,
    },
    Image {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    },
}

enum RichLine {
    /// Runs of `(text, bold)`; `true` when the line belongs to a subheading.
    Text(Vec<(String, bool)>, bool),
    Gap,
}

/// Positions are millimetres from the top-left corner; text `y` is the baseline.
struct Layout<'f> {
    face: ttf_parser::Face<'f>,
    bold: ttf_parser::Face<'f>,
    pages: Vec<Vec<Op>>,
    y: f64,
}

impl Layout<'_> {
    fn width(&self, text: &str, size: f64) -> f64 {
        self.measure(text, size, false)
    }

    fn measure(&self, text: &str, size: f64, bold: bool) -> f64 {
        let face = if bold { &self.bold } else { &self.face };
        let units = f64::from(face.units_per_em());
        let advance: f64 = text
            .chars()
            .map(|c| {
                face.glyph_index(c)
                    .and_then(|id| face.glyph_hor_advance(id))
                    .map_or(units * 0.5, f64::from)
            })
            .sum();
        advance / units * size * 25.4 / 72.0
    }

    /// Greedy word wrap; each `\n` starts a new line, empty lines are kept.
    fn wrap(&self, text: &str, size: f64, max: f64) -> Vec<String> {
        let mut lines = Vec::new();
        for paragraph in text.lines() {
            let mut line = String::new();
            for word in paragraph.split_whitespace() {
                let candidate = if line.is_empty() {
                    word.to_string()
                } else {
                    format!("{line} {word}")
                };
                if self.width(&candidate, size) <= max || line.is_empty() {
                    line = candidate;
                } else {
                    lines.push(std::mem::replace(&mut line, word.to_string()));
                }
            }
            lines.push(line);
        }
        lines
    }

    /// Wraps text written with the light markup used in documents:
    /// `**bold**`, a line starting with `# ` for a subheading, and a blank line
    /// between paragraphs.
    fn rich(&self, text: &str, size: f64, max: f64) -> Vec<RichLine> {
        let mut lines = Vec::new();
        for raw in text.lines() {
            let raw = raw.trim_end();
            if raw.trim().is_empty() {
                if !matches!(lines.last(), None | Some(RichLine::Gap)) {
                    lines.push(RichLine::Gap);
                }
                continue;
            }
            let (heading, body) = match raw.trim_start().strip_prefix("# ") {
                Some(title) => (true, title.trim()),
                None => (false, raw),
            };
            // Words with their weight and whether a space precedes them.
            let mut words: Vec<(String, bool, bool)> = Vec::new();
            let mut space = false;
            for (index, span) in body.split("**").enumerate() {
                let bold = heading || index % 2 == 1;
                let mut word = String::new();
                for c in span.chars() {
                    if c.is_whitespace() {
                        if !word.is_empty() {
                            words.push((std::mem::take(&mut word), bold, space));
                        }
                        // Each word records whether whitespace preceded it, so spans
                        // glued together (`**prix**,`) stay glued.
                        space = true;
                    } else {
                        word.push(c);
                    }
                }
                if !word.is_empty() {
                    words.push((word, bold, space));
                    space = false;
                }
            }
            let mut line: Vec<(String, bool)> = Vec::new();
            let mut width = 0.0;
            for (word, bold, spaced) in words {
                let gap = if spaced && !line.is_empty() {
                    self.width(" ", size)
                } else {
                    0.0
                };
                let advance = self.measure(&word, size, bold);
                if !line.is_empty() && width + gap + advance > max {
                    lines.push(RichLine::Text(std::mem::take(&mut line), heading));
                    width = 0.0;
                } else if gap > 0.0 {
                    width += gap;
                    // The space belongs to the text before it, in that text's weight.
                    if let Some((text, _)) = line.last_mut() {
                        text.push(' ');
                    }
                }
                width += advance;
                match line.last_mut() {
                    Some((text, weight)) if *weight == bold => text.push_str(&word),
                    _ => line.push((word, bold)),
                }
            }
            if !line.is_empty() {
                lines.push(RichLine::Text(line, heading));
            }
        }
        while matches!(lines.last(), Some(RichLine::Gap)) {
            lines.pop();
        }
        lines
    }

    fn runs(&mut self, x: f64, size: f64, runs: &[(String, bool)], color: Rgb8) {
        let mut x = x;
        for (text, bold) in runs {
            if !text.trim().is_empty() {
                self.text(x, size, text, color, *bold, Align::Left);
            }
            x += self.measure(text, size, *bold);
        }
    }

    /// Flowing text across pages; `gap` separates paragraphs.
    fn flow(&mut self, text: &str, size: f64, line: f64, gap: f64) {
        for item in self.rich(text, size, RIGHT - LEFT) {
            match item {
                RichLine::Gap => self.y += gap,
                RichLine::Text(runs, heading) => {
                    if heading && !self.ensure(line * 2.0) {
                        self.y += gap * 0.6;
                    } else if !heading {
                        self.ensure(line);
                    }
                    let size = if heading { size + 1.0 } else { size };
                    self.runs(LEFT, size, &runs, INK);
                    self.y += if heading { line + 1.0 } else { line };
                }
            }
        }
    }

    fn page(&mut self) -> &mut Vec<Op> {
        self.pages.last_mut().expect("layout always has a page")
    }

    fn text(&mut self, x: f64, size: f64, text: &str, color: Rgb8, bold: bool, align: Align) {
        let y = self.y;
        self.page().push(Op::Text {
            x,
            y,
            size,
            text: text.to_string(),
            color,
            bold,
            align,
        });
    }

    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, color: Rgb8) {
        self.page().push(Op::Rect { x, y, w, h, color });
    }

    fn new_page(&mut self) {
        self.pages.push(Vec::new());
        self.y = 20.0;
    }

    /// Starts a new page when `height` millimetres do not fit; returns whether it did.
    fn ensure(&mut self, height: f64) -> bool {
        if self.y + height > BOTTOM {
            self.new_page();
            return true;
        }
        false
    }

    fn heading(&mut self, text: &str) {
        self.text(LEFT, 14.0, text, INK, false, Align::Left);
        self.y += 9.0;
    }
}

pub fn render(document: &Document<'_>) -> Result<Vec<u8>> {
    let fr = document.language == "fr";
    let t = |f: &'static str, e: &'static str| if fr { f } else { e };
    let quote = document.quote;
    let currency = document.currency.trim().to_uppercase();
    let (net, _, total) = quote.totals(&currency)?;
    let money = |amount: &str| crate::brand::money(&amount.replace(',', "."), &currency, fr);
    let face = ttf_parser::Face::parse(FONT, 0).context("Invalid embedded font")?;
    let bold = ttf_parser::Face::parse(BOLD_FONT, 0).context("Invalid embedded font")?;
    let mut layout = Layout {
        face,
        bold,
        pages: vec![Vec::new()],
        y: 0.0,
    };
    let reference = document.number.unwrap_or(t("Provisoire", "Draft"));
    let heading = format!("{} {reference}", kind_name(document.kind, fr));

    // Header
    layout.y = 20.0;
    layout.text(LEFT, 18.0, &heading, INK, false, Align::Left);
    layout.y = 28.5;
    layout.text(
        LEFT,
        13.5,
        &long_date(document.date, fr),
        LABEL,
        false,
        Align::Left,
    );
    // Small lines under the date: invoice dates, then the demonstration notice.
    let invoice = matches!(document.kind, "invoice" | "deposit");
    let mut notes: Vec<String> = Vec::new();
    if invoice && !quote.service_date.trim().is_empty() {
        notes.push(format!(
            "{} {}",
            t("Date de prestation :", "Service date:"),
            short_date(quote.service_date.trim(), fr)
        ));
    }
    if invoice && !quote.due_date.trim().is_empty() {
        notes.push(format!(
            "{} {}",
            t("Échéance :", "Due date:"),
            short_date(quote.due_date.trim(), fr)
        ));
    }
    if reference.starts_with("TEST-") {
        notes.push(
            t(
                "Document de démonstration, sans valeur légale",
                "Demonstration document, not legally valid",
            )
            .into(),
        );
    }
    layout.y = 34.0;
    for note in &notes {
        layout.text(LEFT, 8.5, note, LABEL, false, Align::Left);
        layout.y += 4.4;
    }
    let header_bottom = layout.y;
    if let Some((w, h)) = document.logo.and_then(logo_size) {
        layout.page().push(Op::Image {
            x: RIGHT - w,
            y: 10.0,
            w,
            h,
        });
    }

    // Issuer and recipient
    let issuer = document.issuer;
    let sender = [
        (t("Société :", "Company:"), issuer.name.as_str(), true),
        (t("Votre contact :", "Contact:"), &issuer.contact, false),
        (t("Adresse :", "Address:"), &issuer.address, false),
        (t("Pays :", "Country:"), &issuer.country, false),
        (
            t("N° SIREN :", "Company number:"),
            &issuer.registration,
            false,
        ),
        (t("Numéro de TVA :", "VAT number:"), &issuer.vat_id, false),
        (t("Numéro de téléphone :", "Phone:"), &issuer.phone, false),
        (t("Adresse email :", "Email:"), &issuer.email, false),
    ];
    let recipient = [
        (t("Société :", "Company:"), quote.recipient.as_str(), true),
        (t("Adresse :", "Address:"), &quote.recipient_address, false),
        (t("Pays :", "Country:"), &quote.recipient_country, false),
        (
            t("Numéro d’entreprise :", "Company number:"),
            &quote.recipient_registration,
            false,
        ),
        (
            t("Adresse email :", "Email:"),
            &quote.recipient_email,
            false,
        ),
    ];
    let top = (header_bottom + 6.0).max(42.0);
    let mut bottom: f64 = top;
    for (x, title, rows) in [
        (LEFT, t("Émetteur", "Issuer"), &sender[..]),
        (108.0, t("Destinataire", "Recipient"), &recipient[..]),
    ] {
        layout.y = top;
        layout.text(x, 14.0, title, INK, false, Align::Left);
        layout.y += 9.0;
        for (label, value, bold) in rows {
            let lines = layout.wrap(value.trim(), 8.5, 58.0);
            if lines.iter().all(|line| line.is_empty()) {
                continue;
            }
            layout.text(x, 7.5, label, LABEL, false, Align::Left);
            for line in lines {
                layout.text(x + 33.0, 8.5, &line, INK, *bold, Align::Left);
                layout.y += LINE;
            }
            layout.y += 1.0;
        }
        bottom = bottom.max(layout.y);
    }
    layout.y = bottom + 8.0;

    // Line items
    layout.ensure(30.0);
    layout.heading(t("Détail", "Details"));
    let show_discount = quote
        .lines
        .iter()
        .any(|line| !matches!(line.discount.trim(), "" | "0"));
    let show_tax = quote
        .lines
        .iter()
        .any(|line| !matches!(line.vat.trim(), "" | "0"));
    let mut columns: Vec<(&str, f64)> = vec![(t("Total", "Total"), RIGHT)];
    let mut edge = RIGHT - 18.0;
    columns.push((t("Quantité", "Quantity"), edge));
    edge -= 15.0;
    if show_tax {
        columns.push((t("TVA", "Tax"), edge));
        edge -= 14.0;
    }
    if show_discount {
        columns.push((t("Remise", "Discount"), edge));
        edge -= 15.0;
    }
    columns.push((t("Prix unitaire", "Unit price"), edge));
    let description_width = edge - 18.0 - (LEFT + 2.5);
    let table_header = |layout: &mut Layout<'_>| {
        let y = layout.y;
        layout.rect(LEFT, y, RIGHT - LEFT, 8.0, HEADER);
        layout.y = y + 5.3;
        layout.text(
            LEFT + 2.5,
            TABLE_FONT,
            t("Description", "Description"),
            WHITE,
            false,
            Align::Left,
        );
        for (index, (label, right)) in columns.iter().enumerate() {
            layout.text(right - 1.0, TABLE_FONT, label, WHITE, false, Align::Right);
            // Separator on the left of each numeric column, as in the header band.
            let left = columns
                .get(index + 1)
                .map_or(right - 20.0, |(_, next)| *next);
            layout.rect(left + 0.5, y, 0.25, 8.0, (110, 110, 110));
        }
        layout.y = y + 8.0;
    };
    table_header(&mut layout);
    let nets = quote.line_nets(&currency)?;
    for (index, (line, line_total)) in quote.lines.iter().zip(&nets).enumerate() {
        let mut text = line.description.lines();
        let title = text.next().unwrap_or("");
        let details = text.collect::<Vec<_>>().join("\n");
        let title_lines = layout.rich(title, TABLE_FONT, description_width);
        let detail_lines = layout.rich(details.trim_matches('\n'), TABLE_FONT, description_width);
        let fill = (index % 2 == 1).then_some(ALTERNATE);
        if layout.ensure(5.0 + LINE * title_lines.len().max(1) as f64) {
            table_header(&mut layout);
        }
        let mut chunk = Chunk::start(&layout);
        layout.y += 6.8;
        let mut values = vec![money(line_total), quantity(&line.quantity, fr)];
        if show_tax {
            values.push(format!("{} %", line.vat.trim()));
        }
        if show_discount {
            values.push(format!("{} %", line.discount.trim()));
        }
        values.push(money(&line.price));
        for ((_, right), value) in columns.iter().zip(&values) {
            layout.text(right - 1.0, TABLE_FONT, value, INK, false, Align::Right);
        }
        for item in &title_lines {
            if let RichLine::Text(runs, _) = item {
                layout.runs(LEFT + 2.5, TABLE_FONT, runs, INK);
                layout.y += LINE;
            }
        }
        if !detail_lines.is_empty() {
            layout.y += LINE;
        }
        for item in &detail_lines {
            let RichLine::Text(runs, heading) = item else {
                layout.y += 2.0;
                continue;
            };
            if layout.y + 2.5 > BOTTOM {
                chunk.close(&mut layout, fill);
                layout.new_page();
                table_header(&mut layout);
                chunk = Chunk::start(&layout);
                layout.y += 4.8;
            }
            let size = if *heading {
                TABLE_FONT + 0.5
            } else {
                TABLE_FONT
            };
            layout.runs(LEFT + 2.5, size, runs, INK);
            layout.y += LINE;
        }
        layout.y -= 1.5;
        chunk.close(&mut layout, fill);
    }

    // Totals
    let mut rows: Vec<(String, String, bool)> = Vec::new();
    let global = quote.global_discount.trim();
    if !matches!(global, "" | "0") {
        let mut before = rust_decimal::Decimal::ZERO;
        for value in &nets {
            before += value.parse::<rust_decimal::Decimal>().unwrap_or_default();
        }
        let after: rust_decimal::Decimal = net.parse().unwrap_or_default();
        rows.push((
            t("Total HT avant remise", "Subtotal before discount").into(),
            money(&before.to_string()),
            false,
        ));
        rows.push((
            format!("{} {global} %", t("Remise", "Discount")),
            money(&(after - before).to_string()),
            false,
        ));
    }
    let buckets = quote.tax_breakdown(&currency)?;
    if buckets.iter().any(|b| b.rate != "0") {
        rows.push((t("Total HT", "Subtotal").into(), money(&net), false));
        for bucket in buckets.iter().filter(|b| b.rate != "0") {
            rows.push((
                format!("{} ({} %)", t("TVA", "Tax"), bucket.rate),
                money(&bucket.tax),
                false,
            ));
        }
        rows.push((t("Total TTC", "Total").into(), money(&total), true));
    } else {
        rows.push((t("Total", "Total").into(), money(&total), true));
    }
    if let Some((applied, outstanding)) = document.applied_deposit.filter(|(a, _)| *a != "0") {
        rows.push((
            t("Acomptes imputés", "Deposits applied").into(),
            money(&format!("-{applied}")),
            false,
        ));
        rows.push((
            t("Net à payer", "Balance due").into(),
            money(outstanding),
            true,
        ));
    }
    let mut reasons: Vec<&str> = buckets
        .iter()
        .map(|b| b.exemption_reason.as_str())
        .filter(|r| !r.is_empty())
        .collect();
    reasons.dedup();
    layout.ensure(6.0 + 5.0 * reasons.len() as f64 + 5.5 * rows.len() as f64);
    layout.y += 6.0;
    for reason in reasons {
        layout.text(RIGHT, 7.5, reason, LABEL, false, Align::Right);
        layout.y += 5.0;
    }
    for (label, amount, bold) in &rows {
        layout.text(RIGHT - 24.0, 8.5, label, INK, *bold, Align::Right);
        layout.text(RIGHT - 1.0, 8.5, amount, INK, *bold, Align::Right);
        layout.y += 5.5;
    }
    if let Some(conversion) = &quote.conversion {
        let rate = rust_decimal::Decimal::from_str_exact(&conversion.rate).map_or_else(
            |_| conversion.rate.clone(),
            |rate| rate.normalize().to_string(),
        );
        let line = if fr {
            format!(
                "Montants convertis de {} au taux 1 {} = {} {} ({}, {})",
                conversion.from,
                conversion.from,
                rate.replace('.', ","),
                conversion.to,
                conversion.source,
                short_date(&conversion.date, true)
            )
        } else {
            format!(
                "Amounts converted from {} at 1 {} = {} {} ({}, {})",
                conversion.from,
                conversion.from,
                rate,
                conversion.to,
                conversion.source,
                conversion.date
            )
        };
        layout.ensure(5.0);
        layout.y += 1.0;
        layout.text(RIGHT, 7.5, &line, LABEL, false, Align::Right);
        layout.y += 4.0;
    }
    if invoice || document.kind == "credit" {
        // Mandatory on B2B invoices: late-payment penalties and recovery indemnity.
        // Credit notes repeat it so the printed text matches their Factur-X notes.
        layout.y += 3.0;
        for line in layout.wrap(issuer.late_penalties(document.language), 7.5, RIGHT - LEFT) {
            layout.ensure(3.6);
            layout.text(LEFT, 7.5, &line, LABEL, false, Align::Left);
            layout.y += 3.6;
        }
    }

    // Conditions
    let mut conditions = vec![
        (
            t("Conditions de règlement :", "Payment terms:"),
            quote.payment_terms.trim(),
        ),
        (
            t("Mode de règlement :", "Payment method:"),
            quote.payment_method.trim(),
        ),
    ];
    if document.kind == "quote" {
        conditions.push((
            t("Validité du devis :", "Quote valid for:"),
            quote.valid_until.trim(),
        ));
    }
    conditions.retain(|(_, value)| !value.is_empty());
    if !conditions.is_empty() || !quote.terms.trim().is_empty() {
        layout.new_page();
        layout.heading(t("Conditions", "Terms"));
        for (label, value) in &conditions {
            layout.text(LEFT, 8.5, label, INK, true, Align::Left);
            let x = LEFT + layout.measure(label, 8.5, true) + 2.0;
            layout.text(x, 8.5, value, INK, false, Align::Left);
            layout.y += LINE;
        }
        if !quote.terms.trim().is_empty() {
            if !conditions.is_empty() {
                layout.y += 2.0;
            }
            layout.flow(quote.terms.trim(), 7.8, 3.75, 2.0);
        }
    }

    // General terms of sale
    // A document keeps its own copy; older drafts fall back to the issuer's text.
    let cgv = if quote.cgv.trim().is_empty() {
        issuer.cgv.trim()
    } else {
        quote.cgv.trim()
    };
    let terms_of_sale = document.kind == "quote" || (invoice && issuer.cgv_on_invoices);
    if terms_of_sale && !cgv.is_empty() {
        layout.new_page();
        layout.heading(t("Conditions générales de vente", "General terms of sale"));
        layout.flow(cgv, 7.8, 3.9, 2.4);
    }

    // Footer
    let count = layout.pages.len();
    for (index, page) in layout.pages.iter_mut().enumerate() {
        let number = format!(
            "{} {} {} {count}",
            t("Page", "Page"),
            index + 1,
            t("sur", "of")
        );
        for (x, text, align) in [
            (LEFT, heading.clone(), Align::Left),
            (RIGHT, number, Align::Right),
        ] {
            page.push(Op::Text {
                x,
                y: PAGE_H - 10.0,
                size: 7.5,
                text,
                color: LABEL,
                bold: false,
                align,
            });
        }
    }
    emit(document, &layout)
}

/// Part of a table row on one page; its background is drawn behind its text.
struct Chunk {
    page: usize,
    op: usize,
    top: f64,
}

impl Chunk {
    fn start(layout: &Layout<'_>) -> Self {
        Self {
            page: layout.pages.len() - 1,
            op: layout.pages.last().map_or(0, Vec::len),
            top: layout.y,
        }
    }

    fn close(self, layout: &mut Layout<'_>, fill: Option<Rgb8>) {
        let height = layout.y - self.top;
        if let Some(color) = fill {
            layout.pages[self.page].insert(
                self.op,
                Op::Rect {
                    x: LEFT,
                    y: self.top,
                    w: RIGHT - LEFT,
                    h: height,
                    color,
                },
            );
        }
        let y = layout.y;
        layout.rect(LEFT, y - 0.1, RIGHT - LEFT, 0.2, RULE);
    }
}

fn emit(document: &Document<'_>, layout: &Layout<'_>) -> Result<Vec<u8>> {
    let title = if document.title.trim().is_empty() {
        "FactureCraft"
    } else {
        document.title
    };
    let (pdf, first_page, first_layer) = PdfDocument::new(title, Mm(PAGE_W), Mm(PAGE_H), "content");
    // Default PDF/X-3 conformance embeds a ~650 KB colour profile; a plain PDF needs none.
    let pdf = pdf.with_conformance(printpdf::PdfConformance::Custom(
        printpdf::CustomPdfConformance::default(),
    ));
    // Embed only the glyphs this document uses.
    let mut regular_text = String::new();
    let mut bold_text = String::new();
    for op in layout.pages.iter().flatten() {
        if let Op::Text { text, bold, .. } = op {
            if *bold {
                &mut bold_text
            } else {
                &mut regular_text
            }
            .push_str(text);
        }
    }
    let font =
        pdf.add_external_font(crate::subset::subset(FONT, regular_text.chars())?.as_slice())?;
    let bold_font =
        pdf.add_external_font(crate::subset::subset(BOLD_FONT, bold_text.chars())?.as_slice())?;
    let logo = document.logo.map(decode_logo).transpose()?;
    for (index, ops) in layout.pages.iter().enumerate() {
        let layer = if index == 0 {
            pdf.get_page(first_page).get_layer(first_layer)
        } else {
            let (page, layer) = pdf.add_page(Mm(PAGE_W), Mm(PAGE_H), "content");
            pdf.get_page(page).get_layer(layer)
        };
        for op in ops {
            match op {
                Op::Rect { x, y, w, h, color } => {
                    layer.set_fill_color(rgb(*color));
                    let (top, bottom) = (PAGE_H - y, PAGE_H - y - h);
                    layer.add_shape(Line {
                        points: vec![
                            (Point::new(Mm(*x), Mm(top)), false),
                            (Point::new(Mm(x + w), Mm(top)), false),
                            (Point::new(Mm(x + w), Mm(bottom)), false),
                            (Point::new(Mm(*x), Mm(bottom)), false),
                        ],
                        is_closed: true,
                        has_fill: true,
                        has_stroke: false,
                        is_clipping_path: false,
                    });
                }
                Op::Text {
                    x,
                    y,
                    size,
                    text,
                    color,
                    bold,
                    align,
                } => {
                    let x = match align {
                        Align::Left => *x,
                        Align::Right => x - layout.measure(text, *size, *bold),
                    };
                    let font = if *bold { &bold_font } else { &font };
                    layer.set_fill_color(rgb(*color));
                    layer.use_text(text.as_str(), *size, Mm(x), Mm(PAGE_H - y), font);
                }
                Op::Image { x, y, w, h } => {
                    if let Some(image) = &logo {
                        let dpi = image.width.0 as f64 * 25.4 / w;
                        Image::from(image.clone()).add_to_layer(
                            layer.clone(),
                            Some(Mm(*x)),
                            Some(Mm(PAGE_H - y - h)),
                            None,
                            None,
                            None,
                            Some(dpi),
                        );
                    }
                }
            }
        }
    }
    let mut bytes = Vec::new();
    pdf.save(&mut BufWriter::new(&mut bytes))?;
    // printpdf embeds fonts uncompressed; deflate every stream before returning.
    let mut compressed = lopdf::Document::load_mem(&bytes)?;
    compressed.compress();
    let mut output = Vec::new();
    compressed.save_to(&mut output)?;
    Ok(output)
}

fn rgb((r, g, b): Rgb8) -> Color {
    Color::Rgb(Rgb::new(
        f64::from(r) / 255.0,
        f64::from(g) / 255.0,
        f64::from(b) / 255.0,
        None,
    ))
}

/// Logo box: at most 24 mm wide and 22 mm high, keeping proportions.
fn logo_size(png: &[u8]) -> Option<(f64, f64)> {
    let (w, h) = image::load_from_memory(png).ok()?.into_rgba8().dimensions();
    let ratio = f64::from(w) / f64::from(h.max(1));
    Some(if ratio * 22.0 > 24.0 {
        (24.0, 24.0 / ratio)
    } else {
        (22.0 * ratio, 22.0)
    })
}

/// Decodes the PNG and flattens transparency onto white.
fn decode_logo(png: &[u8]) -> Result<ImageXObject> {
    let rgba = image::load_from_memory(png)?.into_rgba8();
    let (width, height) = rgba.dimensions();
    let mut data = Vec::with_capacity(width as usize * height as usize * 3);
    for pixel in rgba.pixels() {
        let [r, g, b, a] = pixel.0;
        let alpha = u16::from(a);
        for channel in [r, g, b] {
            data.push(((u16::from(channel) * alpha + 255 * (255 - alpha)) / 255) as u8);
        }
    }
    Ok(ImageXObject {
        width: Px(width as usize),
        height: Px(height as usize),
        color_space: ColorSpace::Rgb,
        bits_per_component: ColorBits::Bit8,
        interpolate: true,
        image_data: data,
        image_filter: None,
        clipping_bbox: None,
    })
}

/// Resizes an uploaded logo (PNG or JPEG) to at most 600 px and re-encodes it as PNG.
pub fn prepare_logo(bytes: &[u8]) -> Result<Vec<u8>> {
    let image = image::load_from_memory(bytes).context("Unsupported image")?;
    let image = if image.width().max(image.height()) > 600 {
        image.thumbnail(600, 600)
    } else {
        image
    };
    let mut png = Vec::new();
    image.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)?;
    Ok(png)
}

fn quantity(value: &str, fr: bool) -> String {
    let value = value.trim();
    if fr {
        value.replace('.', ",")
    } else {
        value.replace(',', ".")
    }
}

/// `2026-12-18` → `18/12/2026` in French; other text is kept as typed.
fn short_date(value: &str, fr: bool) -> String {
    let parts: Vec<&str> = value.split('-').collect();
    if fr && parts.len() == 3 && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit())) {
        format!("{}/{}/{}", parts[2], parts[1], parts[0])
    } else {
        value.to_string()
    }
}

fn long_date(iso: &str, fr: bool) -> String {
    const FR: [&str; 12] = [
        "janvier",
        "février",
        "mars",
        "avril",
        "mai",
        "juin",
        "juillet",
        "août",
        "septembre",
        "octobre",
        "novembre",
        "décembre",
    ];
    const EN: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let parts: Vec<&str> = iso.split('-').collect();
    let (Some(year), Some(month), Some(day)) = (
        parts.first(),
        parts
            .get(1)
            .and_then(|m| m.parse::<usize>().ok())
            .filter(|m| (1..=12).contains(m)),
        parts.get(2).and_then(|d| d.parse::<u32>().ok()),
    ) else {
        return iso.to_string();
    };
    if fr {
        format!("{day} {} {year}", FR[month - 1])
    } else {
        format!("{} {day}, {year}", EN[month - 1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use facturecraft_core::quote::Line as QuoteLine;

    #[test]
    fn renders_a_long_french_quote_with_logo_and_terms() {
        let quote = Quote {
            recipient: "Éditions de démonstration".into(),
            recipient_address: "1 rue Exemple\n75000 Paris".into(),
            payment_terms: "30 jours".into(),
            valid_until: "30 jours".into(),
            lines: (0..75)
                .map(|number| QuoteLine {
                    description: format!(
                        "Prestation n° {number}\nCréation, révision et préparation d’un livrable détaillé sur plusieurs lignes de texte pour vérifier le retour à la ligne."
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
        let mut logo = Vec::new();
        image::RgbaImage::from_pixel(40, 20, image::Rgba([0, 0, 0, 128]))
            .write_to(
                &mut std::io::Cursor::new(&mut logo),
                image::ImageFormat::Png,
            )
            .unwrap();
        let issuer = Issuer {
            name: "Exemple".into(),
            cgv: "1. Objet\nTexte.\n\n2. Prix\nTexte.".into(),
            ..Default::default()
        };
        let bytes = render(&Document {
            kind: "quote",
            number: None,
            date: "2026-11-18",
            title: "Devis de démonstration",
            currency: "EUR",
            language: "fr",
            quote: &quote,
            issuer: &issuer,
            logo: Some(&logo),
            applied_deposit: None,
        })
        .unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        assert!(bytes.windows(5).any(|bytes| bytes == b"%%EOF"));
        assert_eq!(long_date("2026-10-25", true), "25 octobre 2026");
    }

    #[test]
    fn renders_a_converted_quote_with_private_fields_in_both_languages() {
        let quote = Quote {
            recipient: "Client".into(),
            tags: vec!["interne".into()],
            notes: "note privée".into(),
            conversion: Some(facturecraft_core::quote::Conversion {
                from: "EUR".into(),
                to: "CHF".into(),
                rate: "0.9312".into(),
                date: "2026-11-18".into(),
                source: "BCE".into(),
            }),
            lines: vec![QuoteLine {
                description: "Prestation".into(),
                price: "93.12".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let issuer = Issuer::default();
        let render_in = |language| {
            render(&Document {
                kind: "quote",
                number: None,
                date: "2026-11-18",
                title: "Devis",
                currency: "CHF",
                language,
                quote: &quote,
                issuer: &issuer,
                logo: None,
                applied_deposit: None,
            })
            .unwrap()
        };
        let (fr, en) = (render_in("fr"), render_in("en"));
        assert!(fr.starts_with(b"%PDF-") && en.starts_with(b"%PDF-"));
        // The converted line adds content compared with the same quote without conversion.
        let plain = Quote {
            conversion: None,
            ..quote.clone()
        };
        let without = render(&Document {
            kind: "quote",
            number: None,
            date: "2026-11-18",
            title: "Devis",
            currency: "CHF",
            language: "fr",
            quote: &plain,
            issuer: &issuer,
            logo: None,
            applied_deposit: None,
        })
        .unwrap();
        assert_ne!(fr, without);
    }

    #[test]
    fn light_markup_gives_bold_runs_headings_and_paragraphs() {
        let face = ttf_parser::Face::parse(FONT, 0).unwrap();
        let bold = ttf_parser::Face::parse(BOLD_FONT, 0).unwrap();
        let layout = Layout {
            face,
            bold,
            pages: vec![Vec::new()],
            y: 0.0,
        };
        let lines = layout.rich(
            "**Forfait :** 8 000 €, **net**.\n\n\n# Titre\nsuite",
            8.0,
            500.0,
        );
        let RichLine::Text(runs, false) = &lines[0] else {
            panic!("first line")
        };
        assert_eq!(
            runs,
            &vec![
                ("Forfait : ".to_string(), true),
                ("8 000 €, ".to_string(), false),
                ("net".to_string(), true),
                (".".to_string(), false),
            ]
        );
        assert!(
            matches!(lines[1], RichLine::Gap),
            "blank lines collapse to one gap"
        );
        assert!(
            matches!(&lines[2], RichLine::Text(runs, true) if runs[0] == ("Titre".to_string(), true))
        );
        assert!(matches!(&lines[3], RichLine::Text(_, false)));
        assert_eq!(lines.len(), 4);
        let narrow = layout.rich("un deux trois quatre", 8.0, 12.0);
        assert!(narrow.len() > 1, "wraps to the width");
    }

    #[test]
    fn uploaded_logo_is_resized_to_png() {
        let mut jpeg = Vec::new();
        image::RgbImage::from_pixel(1200, 600, image::Rgb([20, 20, 20]))
            .write_to(
                &mut std::io::Cursor::new(&mut jpeg),
                image::ImageFormat::Jpeg,
            )
            .unwrap();
        let png = prepare_logo(&jpeg).unwrap();
        let resized = image::load_from_memory(&png).unwrap();
        assert!(png.starts_with(b"\x89PNG"));
        assert_eq!((resized.width(), resized.height()), (600, 300));
        assert_eq!(logo_size(&png), Some((24.0, 12.0)));
    }
}

#[cfg(test)]
mod acceptance {
    use super::*;
    use facturecraft_core::{Store, contacts::Contact, issuer::Issuer, quote::Line};

    #[test]
    fn actual_pdfs_follow_quote_invoice_payment_credit_and_restore() -> Result<()> {
        let folder = tempfile::tempdir()?;
        let store = Store::open(&folder.path().join("acceptance.sqlite3"))?;
        store.save_issuer(&Issuer {
            name: "Atelier Exemple".into(),
            address: "1 rue du Test, 75001 Paris".into(),
            registration: "123456789".into(),
            country: "FR".into(),
            ..Default::default()
        })?;
        store.save_contact(&Contact {
            name: "Client fictif".into(),
            email: "client@example.invalid".into(),
            address: "2 rue du Test".into(),
            country: "FR".into(),
            ..Default::default()
        })?;
        assert_eq!(store.contacts()?.len(), 1);
        let quote = Quote {
            recipient: "Client fictif".into(),
            recipient_address: "2 rue du Test".into(),
            due_date: "2026-12-18".into(),
            lines: vec![Line {
                description: "Prestation fictive".into(),
                price: "1000".into(),
                vat: "20".into(),
                ..Default::default()
            }],
            notes: "PRIVATE-NOTE-NEVER-PRINT".into(),
            ..Default::default()
        };
        let draft = store.save_quote(None, "Devis fictif", "EUR", "fr", &quote)?;
        let issue = |id| -> Result<Vec<u8>> {
            let state = store.billing_state(id)?;
            let issuer = store.issuer()?;
            store.finalize_document(id, |draft, quote, number, date| {
                render(&Document {
                    kind: &state.kind,
                    number: Some(number),
                    date,
                    title: &draft.title,
                    currency: &draft.currency,
                    language: &draft.language,
                    quote,
                    issuer: &issuer,
                    logo: None,
                    applied_deposit: (state.kind == "invoice")
                        .then_some((&state.deposits, &state.outstanding)),
                })
            })?;
            store.finalized_pdf(id)
        };
        let original = issue(draft.id)?;
        assert!(original.starts_with(b"%PDF-"));
        store.accept_quote(draft.id, "2026-11-18")?;
        let invoice = store.derive_document(draft.id, "invoice", "Facture fictive")?;
        let invoice_pdf = issue(invoice.id)?;
        store.record_payment(invoice.id, "300", "2026-11-18", "transfer", "test")?;
        assert_eq!(store.billing_state(invoice.id)?.outstanding, "900.00");
        let credit = store.derive_document(invoice.id, "credit", "Avoir fictif")?;
        issue(credit.id)?;
        assert_eq!(store.finalized_pdf(invoice.id)?, invoice_pdf);
        let backup = folder.path().join("backup.sqlite3");
        store.snapshot(&backup)?;
        let restored_path = folder.path().join("restored.sqlite3");
        facturecraft_core::backup::restore_snapshot(&backup, &restored_path)?;
        let restored = Store::open(&restored_path)?;
        assert_eq!(restored.finalized_pdf(draft.id)?, original);
        assert_eq!(restored.finalized_pdf(invoice.id)?, invoice_pdf);
        assert_eq!(restored.payments(invoice.id)?.len(), 1);
        Ok(())
    }
}
