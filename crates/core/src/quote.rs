use crate::{Draft, Store};
use anyhow::{Context, Result, ensure};
use rusqlite::{OptionalExtension, params};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Default, Deserialize, Serialize)]
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
    #[serde(default)]
    pub valid_until: String,
    #[serde(default)]
    pub terms: String,
    #[serde(default)]
    pub global_discount: String,
    pub lines: Vec<Line>,
}

#[derive(Deserialize, Serialize)]
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

impl Quote {
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
