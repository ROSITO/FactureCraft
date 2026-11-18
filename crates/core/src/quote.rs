use crate::{Draft, Store};
use anyhow::{Context, Result, ensure};
use rusqlite::{OptionalExtension, params};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct Quote {
    pub recipient: String,
    #[serde(default)]
    pub recipient_email: String,
    #[serde(default)]
    pub recipient_address: String,
    #[serde(default)]
    pub recipient_country: String,
    /// Company number (SIREN or equivalent).
    #[serde(default)]
    pub recipient_registration: String,
    #[serde(default)]
    pub payment_terms: String,
    #[serde(default)]
    pub payment_method: String,
    /// General terms of sale for this document, copied from the issuer's defaults.
    #[serde(default)]
    pub cgv: String,
    /// Payment due date `YYYY-MM-DD`, required to finalize an invoice.
    #[serde(default)]
    pub due_date: String,
    /// Date or period of the service, when it differs from the issue date.
    #[serde(default)]
    pub service_date: String,
    #[serde(default)]
    pub valid_until: String,
    #[serde(default)]
    pub terms: String,
    #[serde(default)]
    pub global_discount: String,
    /// Private labels for filtering and search; never printed.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Private notes; never printed.
    #[serde(default)]
    pub notes: String,
    /// Set when the amounts were converted from another currency.
    #[serde(default)]
    pub conversion: Option<Conversion>,
    pub lines: Vec<Line>,
}

/// Traceability of an explicit currency conversion: `1 from = rate to`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Conversion {
    pub from: String,
    pub to: String,
    pub rate: String,
    /// Date of the rate, `YYYY-MM-DD`.
    pub date: String,
    /// Where the rate comes from, e.g. a central bank.
    pub source: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Line {
    pub description: String,
    pub quantity: String,
    pub price: String,
    pub discount: String,
    pub vat: String,
    #[serde(default)]
    pub unit: String,
    #[serde(default)]
    pub exemption_reason: String,
}

impl Default for Line {
    fn default() -> Self {
        Self {
            description: String::new(),
            quantity: "1".into(),
            price: "0".into(),
            discount: "0".into(),
            vat: "0".into(),
            unit: String::new(),
            exemption_reason: String::new(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct TaxBucket {
    pub rate: String,
    pub exemption_reason: String,
    pub net: String,
    pub tax: String,
}

/// Splits comma-separated tags, trimming and dropping empty or repeated entries.
pub fn parse_tags(text: &str) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    for tag in text.split(',').map(str::trim).filter(|tag| !tag.is_empty()) {
        if !tags.iter().any(|known| known.eq_ignore_ascii_case(tag)) {
            tags.push(tag.into());
        }
    }
    tags
}

impl Quote {
    /// Whether the lowercase `query` matches a tag or the private notes.
    pub fn matches_private(&self, query: &str) -> bool {
        self.notes.to_lowercase().contains(query)
            || self
                .tags
                .iter()
                .any(|tag| tag.to_lowercase().contains(query))
    }

    pub fn totals(&self, currency: &str) -> Result<(String, String, String)> {
        let precision = currency_precision(currency)?;
        let mut net = Decimal::ZERO;
        let mut vat = Decimal::ZERO;
        for bucket in self.tax_breakdown(currency)? {
            net += Decimal::from_str_exact(&bucket.net)?;
            vat += Decimal::from_str_exact(&bucket.tax)?;
        }
        let format = |value: Decimal| format!("{:.*}", precision as usize, value);
        Ok((format(net), format(vat), format(net + vat)))
    }

    /// Net amount of each line after its own discount, before the global discount.
    pub fn line_nets(&self, currency: &str) -> Result<Vec<String>> {
        let precision = currency_precision(currency)?;
        self.lines
            .iter()
            .map(|line| {
                Ok(format!(
                    "{:.*}",
                    precision as usize,
                    line_net(line, precision)?
                ))
            })
            .collect()
    }

    pub fn tax_breakdown(&self, currency: &str) -> Result<Vec<TaxBucket>> {
        use std::collections::BTreeMap;
        let precision = currency_precision(currency)?;
        let hundred = Decimal::from(100);
        let global = if self.global_discount.trim().is_empty() {
            Decimal::ZERO
        } else {
            number(&self.global_discount)?
        };
        ensure!(global <= hundred, "Invalid global discount");
        let mut buckets = BTreeMap::<(Decimal, &str), (Decimal, Decimal)>::new();
        let mut cumulative_net = Decimal::ZERO;
        let mut allocated_discount = Decimal::ZERO;
        for line in &self.lines {
            ensure!(
                !line.description.trim().is_empty(),
                "A line needs a description"
            );
            let rate = number(&line.vat)?;
            ensure!(rate <= hundred, "Percentages must be from 0 to 100");
            ensure!(
                rate.is_zero() || line.exemption_reason.trim().is_empty(),
                "Exemption requires a zero tax rate"
            );
            let line_net = line_net(line, precision)?;
            cumulative_net += line_net;
            // Cumulative rounding allocates the full discount down to the last minor unit,
            // with tied remainders resolved by the persisted line order.
            let next_discount = (cumulative_net * global / hundred).round_dp(precision);
            let net = line_net - (next_discount - allocated_discount);
            allocated_discount = next_discount;
            let bucket = buckets
                .entry((rate.normalize(), line.exemption_reason.trim()))
                .or_default();
            bucket.0 += net;
            bucket.1 += (net * rate / hundred).round_dp(precision);
        }
        let format = |value: Decimal| format!("{:.*}", precision as usize, value);
        Ok(buckets
            .into_iter()
            .map(|((rate, reason), (net, tax))| TaxBucket {
                rate: rate.to_string(),
                exemption_reason: reason.into(),
                net: format(net),
                tax: format(tax),
            })
            .collect())
    }
}

fn line_net(line: &Line, precision: u32) -> Result<Decimal> {
    let hundred = Decimal::from(100);
    let discount = number(&line.discount)?;
    ensure!(discount <= hundred, "Percentages must be from 0 to 100");
    Ok(
        (number(&line.quantity)? * number(&line.price)? * (hundred - discount) / hundred)
            .round_dp(precision),
    )
}

fn currency_precision(currency: &str) -> Result<u32> {
    let currency: iso_currency::Currency = currency
        .trim()
        .to_ascii_uppercase()
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid currency"))?;
    Ok(u32::from(
        currency
            .exponent()
            .context("Currency has no standard precision")?,
    ))
}

fn number(input: &str) -> Result<Decimal> {
    let value = Decimal::from_str_exact(&input.trim().replace(',', "."))?;
    ensure!(
        value >= Decimal::ZERO && value <= Decimal::from(1_000_000_000) && value.scale() <= 6,
        "Expected a non-negative number with up to six decimal places"
    );
    Ok(value)
}

impl Store {
    /// Converts the unit prices of `quote` from `from` to `to` at `rate` (`1 from = rate to`).
    ///
    /// Prices are rounded to six decimals, then totals follow the precision of the new currency.
    /// The rate, its date and its source are kept in [`Quote::conversion`]. A document is
    /// converted at most once so that the trace always starts from its original currency.
    pub fn convert_quote(
        &self,
        quote: &Quote,
        from: &str,
        to: &str,
        rate: &str,
        date: &str,
        source: &str,
    ) -> Result<Quote> {
        ensure!(
            quote.conversion.is_none(),
            "This document was already converted; discard the changes to start again"
        );
        let (from, to) = (
            from.trim().to_ascii_uppercase(),
            to.trim().to_ascii_uppercase(),
        );
        currency_precision(&from)?;
        currency_precision(&to)?;
        ensure!(from != to, "Choose a different currency");
        self.validate_date(date.trim())?;
        ensure!(!source.trim().is_empty(), "The rate source is required");
        let rate = Decimal::from_str_exact(&rate.trim().replace(',', "."))?.normalize();
        ensure!(
            rate > Decimal::ZERO && rate <= Decimal::from(1_000_000) && rate.scale() <= 8,
            "Expected a positive rate with up to eight decimal places"
        );
        let mut converted = quote.clone();
        for line in &mut converted.lines {
            line.price = (number(&line.price)? * rate)
                .round_dp(6)
                .normalize()
                .to_string();
        }
        converted.totals(&to)?;
        converted.conversion = Some(Conversion {
            from,
            to,
            rate: rate.to_string(),
            date: date.trim().into(),
            source: source.trim().into(),
        });
        Ok(converted)
    }

    pub fn quote(&self, id: i64) -> Result<Quote> {
        let json: Option<String> = self
            .0
            .query_row(
                "SELECT details FROM quote_details WHERE draft_id=?1",
                [id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(json
            .as_deref()
            .map(serde_json::from_str)
            .transpose()?
            .unwrap_or_default())
    }

    pub fn save_quote(
        &self,
        id: Option<i64>,
        title: &str,
        currency: &str,
        language: &str,
        quote: &Quote,
    ) -> Result<Draft> {
        let tx = self.0.unchecked_transaction()?;
        let draft = self.write_quote(id, title, currency, language, quote)?;
        tx.commit()?;
        Ok(draft)
    }

    pub(crate) fn write_quote(
        &self,
        id: Option<i64>,
        title: &str,
        currency: &str,
        language: &str,
        quote: &Quote,
    ) -> Result<Draft> {
        quote.totals(currency)?;
        let currency = currency.trim().to_ascii_uppercase();
        let title = title.trim();
        ensure!(
            !title.is_empty() && title.chars().count() <= 200,
            "Title must contain 1–200 characters"
        );
        ensure!(
            matches!(language, "fr" | "en"),
            "Supported document languages: fr, en"
        );
        let draft = if let Some(id) = id {
            ensure!(
                self.0.execute(
                    "UPDATE drafts SET title=?1,currency=?2,language=?3 WHERE id=?4",
                    params![title, currency, language, id]
                )? == 1,
                "Draft not found"
            );
            self.get(id)?.context("Draft not found")?
        } else {
            self.create(title, &currency, language)?
        };
        self.0.execute(
            "INSERT INTO quote_details(draft_id,details) VALUES (?1,?2)
            ON CONFLICT(draft_id) DO UPDATE SET details=excluded.details",
            params![draft.id, serde_json::to_string(quote)?],
        )?;
        Ok(draft)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocates_global_discount_across_tax_rates_in_minor_units() {
        let quote = Quote {
            global_discount: "50".into(),
            lines: vec![
                Line {
                    description: "A".into(),
                    price: "0.05".into(),
                    vat: "20".into(),
                    ..Default::default()
                },
                Line {
                    description: "B".into(),
                    price: "0.05".into(),
                    exemption_reason: "Motif de test".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let buckets = quote.tax_breakdown("EUR").unwrap();
        assert_eq!(buckets[0].exemption_reason, "Motif de test");
        assert_eq!(buckets[0].net, "0.02");
        assert_eq!(buckets[1].net, "0.03");
        assert_eq!(
            quote.totals("EUR").unwrap(),
            ("0.05".into(), "0.01".into(), "0.06".into())
        );
    }

    #[test]
    fn tags_and_notes_roundtrip_and_match() {
        assert_eq!(parse_tags(" a, B ,,a,c d "), ["a", "B", "c d"]);
        let store = Store::open(std::path::Path::new(":memory:")).unwrap();
        let quote = Quote {
            recipient: "X".into(),
            tags: parse_tags("urgent, 2026"),
            notes: "Relancer Lundi".into(),
            lines: vec![Line {
                description: "A".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let draft = store.save_quote(None, "T", "EUR", "fr", &quote).unwrap();
        let back = store.quote(draft.id).unwrap();
        assert_eq!(back.tags, ["urgent", "2026"]);
        assert!(back.matches_private("lundi") && back.matches_private("urg"));
        assert!(!back.matches_private("zzz"));
        // Documents saved before tags existed still load.
        let old: Quote = serde_json::from_str(r#"{"recipient":"X","lines":[]}"#).unwrap();
        assert!(old.tags.is_empty() && old.notes.is_empty());
    }

    #[test]
    fn conversion_rounds_prices_then_totals_and_keeps_the_trace() {
        let store = Store::open(std::path::Path::new(":memory:")).unwrap();
        let quote = Quote {
            recipient: "X".into(),
            lines: vec![
                Line {
                    description: "A".into(),
                    quantity: "3".into(),
                    price: "19.99".into(),
                    vat: "20".into(),
                    ..Default::default()
                },
                Line {
                    description: "B".into(),
                    price: "100".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let converted = store
            .convert_quote(&quote, "eur", "chf", "0,9312", "2026-11-18", " BCE ")
            .unwrap();
        // 19.99 x 0.9312 = 18.614688 (kept to six decimals); 3 x that = 55.844064 -> 55.84.
        assert_eq!(converted.lines[0].price, "18.614688");
        assert_eq!(converted.lines[1].price, "93.12");
        assert_eq!(
            converted.line_nets("CHF").unwrap(),
            ["55.84".to_string(), "93.12".to_string()]
        );
        assert_eq!(
            converted.totals("CHF").unwrap(),
            ("148.96".into(), "11.17".into(), "160.13".into())
        );
        let trace = converted.conversion.as_ref().unwrap();
        assert_eq!(
            (
                trace.from.as_str(),
                trace.to.as_str(),
                trace.rate.as_str(),
                trace.source.as_str()
            ),
            ("EUR", "CHF", "0.9312", "BCE")
        );
        // The original is untouched and the trace survives a save.
        assert!(quote.conversion.is_none() && quote.lines[0].price == "19.99");
        let draft = store
            .save_quote(None, "T", "CHF", "fr", &converted)
            .unwrap();
        assert_eq!(
            store.quote(draft.id).unwrap().conversion,
            converted.conversion
        );
        // Zero-decimal currencies round the totals to whole units.
        let yen = store
            .convert_quote(&quote, "EUR", "JPY", "162.5", "2026-11-18", "BCE")
            .unwrap();
        assert_eq!(yen.totals("JPY").unwrap().0, "25995");
    }

    #[test]
    fn conversion_rejects_invalid_input() {
        let store = Store::open(std::path::Path::new(":memory:")).unwrap();
        let quote = Quote::default();
        let convert = |from, to, rate, date, source| {
            store
                .convert_quote(&quote, from, to, rate, date, source)
                .is_err()
        };
        assert!(convert("EUR", "EUR", "1", "2026-11-18", "BCE"));
        assert!(convert("EUR", "XXX1", "1", "2026-11-18", "BCE"));
        assert!(convert("EUR", "CHF", "0", "2026-11-18", "BCE"));
        assert!(convert("EUR", "CHF", "-1", "2026-11-18", "BCE"));
        assert!(convert("EUR", "CHF", "abc", "2026-11-18", "BCE"));
        assert!(convert("EUR", "CHF", "0.9", "18/11/2026", "BCE"));
        assert!(convert("EUR", "CHF", "0.9", "2026-11-18", "  "));
        let once = store
            .convert_quote(&quote, "EUR", "CHF", "0.9", "2026-11-18", "BCE")
            .unwrap();
        assert!(
            store
                .convert_quote(&once, "CHF", "USD", "1.1", "2026-11-18", "BCE")
                .is_err()
        );
    }

    #[test]
    fn calculates_discount_and_tax_at_currency_precision() {
        let quote = Quote {
            recipient: "Example".into(),
            lines: vec![Line {
                description: "Work".into(),
                quantity: "1,5".into(),
                price: "19.99".into(),
                discount: "10".into(),
                vat: "20".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert_eq!(
            quote.totals("EUR").unwrap(),
            ("26.99".into(), "5.40".into(), "32.39".into())
        );
        assert_eq!(
            quote.totals("JPY").unwrap(),
            ("27".into(), "5".into(), "32".into())
        );
    }
}
