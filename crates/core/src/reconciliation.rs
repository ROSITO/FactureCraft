//! Read-only reconstruction of historical documents from immutable archive records.
//! Never promotes source statuses to authoritative accounting state or invents lines.
use crate::Store;
use anyhow::Result;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::Serialize;
use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Serialize)]
pub struct HistoricalDocument {
    pub source: String,
    pub category: String,
    pub key: String,
    pub number: Option<String>,
    pub kind: String,
    /// Source values remain decimal strings; no floating point calculations.
    pub net: Option<String>,
    pub tax: Option<String>,
    pub gross: Option<String>,
    pub currency: Option<String>,
    /// Frozen source recipient, not guessed from a current contact's name.
    pub recipient_snapshot: Value,
    pub source_status: Value,
    pub source_paid_at: Value,
    pub source_deposits_amount: Value,
    pub pdf_keys: Vec<String>,
    pub reconstructed_lines: Vec<HistoricalLine>,
    pub line_evidence: Option<String>,
    pub deposit_deductions: Vec<HistoricalDeduction>,
    pub unresolved: Vec<String>,
}

#[derive(Serialize)]
pub struct HistoricalLine {
    pub source_type: Option<String>,
    pub description: String,
    /// None means the original printed only an amount (not an inferred 1 × amount).
    pub quantity: Option<String>,
    pub unit_price: Option<String>,
    pub discount_percent: Option<String>,
    pub vat_percent: Option<String>,
    pub net: String,
}

#[derive(Serialize)]
pub struct HistoricalDeduction {
    pub description: String,
    pub reference_number: Option<String>,
    pub source_record_key: Option<String>,
    pub amount: String,
}

impl Store {
    /// Deterministic and idempotent: reads existing evidence, makes no database writes.
    /// Reconstructed summaries remain separate from live invoices and payment balances.
    pub fn reconcile_archive(&self) -> Result<Vec<HistoricalDocument>> {
        let archive = self.archive()?;
        let mut documents = Vec::new();
        for item in archive
            .iter()
            .filter(|i| matches!(i.category.as_str(), "quotations" | "invoices"))
        {
            let record = self.archive_record(&item.source, &item.category, &item.key)?;
            let link = format!("{}:{}", item.category, item.key);
            let pdf_keys: Vec<_> = archive
                .iter()
                .filter(|p| {
                    p.source == item.source
                        && p.category == "pdf"
                        && p.record_link.as_deref() == Some(link.as_str())
                })
                .map(|p| p.key.clone())
                .collect();
            let kind = if item.category == "quotations" {
                "quote"
            } else {
                match record["type"].as_str() {
                    Some("Facture") => "invoice",
                    Some("Facture d’acompte") | Some("Facture d'acompte") => "deposit",
                    Some("Avoir") => "credit",
                    _ => "unknown",
                }
            };
            let mut unresolved = vec![
                "itemized_lines_not_reconstructed".into(),
                "payment_history_incomplete".into(),
            ];
            if pdf_keys.is_empty() {
                unresolved.push("original_pdf_missing".into());
            }
            if pdf_keys.len() > 1 {
                unresolved.push("multiple_original_pdfs".into());
            }
            if kind == "unknown" {
                unresolved.push("document_kind_unknown".into());
            }
            if !record["recipient"].is_object() {
                unresolved.push("recipient_snapshot_missing".into());
            }
            if matches!(kind, "invoice" | "deposit" | "credit") {
                unresolved.push("business_relationships_unconfirmed".into());
            }
            let fields = ["pricePretax", "taxAmount", "price", "currency"];
            for field in fields {
                if scalar(&record[field]).is_none() {
                    unresolved.push(format!("missing_{field}"));
                }
            }
            let mut reconstructed_lines = Vec::new();
            let mut line_evidence = None;
            let mut deposit_deductions = Vec::new();
            if pdf_keys.len() == 1 {
                let result = self
                    .archive_pdf(&item.source, &pdf_keys[0])
                    .map_err(|_| "original_pdf_unreadable")
                    .and_then(|pdf| pdf_layout(&pdf))
                    .and_then(|text| extract_lines(&text, &record));
                match result {
                    Ok((lines, convention, deductions)) => {
                        if lines
                            .iter()
                            .any(|line| line.quantity.is_none() || line.unit_price.is_none())
                        {
                            unresolved
                                .push("line_quantity_and_unit_price_absent_in_original".into());
                        }
                        deposit_deductions = deductions;
                        for deduction in &mut deposit_deductions {
                            let mut candidates = Vec::new();
                            if let Some(reference) = &deduction.reference_number {
                                for archived in archive
                                    .iter()
                                    .filter(|a| a.source == item.source && a.category == "invoices")
                                {
                                    let candidate = self.archive_record(
                                        &archived.source,
                                        &archived.category,
                                        &archived.key,
                                    )?;
                                    let matches = candidate["uid"].as_str()
                                        == Some(reference.as_str())
                                        && matches!(
                                            candidate["type"].as_str(),
                                            Some("Facture d’acompte" | "Facture d'acompte")
                                        )
                                        && candidate["currency"] == record["currency"]
                                        && scalar(&candidate["price"]).and_then(|v| number(&v))
                                            == number(&deduction.amount).map(|v| -v);
                                    if matches {
                                        candidates.push(archived.key.clone());
                                    }
                                }
                            }
                            if candidates.len() == 1 {
                                deduction.source_record_key = candidates.pop();
                            } else {
                                unresolved
                                    .push("deposit_deduction_source_mapping_ambiguous".into());
                            }
                        }
                        if lines.iter().any(|line| line.vat_percent.is_none()) {
                            unresolved.push("line_vat_not_reconstructed".into());
                        }
                        reconstructed_lines = lines;
                        line_evidence = Some(format!("pdf_layout_verified:{convention}"));
                        unresolved.retain(|reason| reason != "itemized_lines_not_reconstructed");
                    }
                    Err(reason) => unresolved.push(reason.into()),
                }
            }
            documents.push(HistoricalDocument {
                source: item.source.clone(),
                category: item.category.clone(),
                key: item.key.clone(),
                number: record["uid"].as_str().map(str::to_owned),
                kind: kind.into(),
                net: scalar(&record["pricePretax"]),
                tax: scalar(&record["taxAmount"]),
                gross: scalar(&record["price"]),
                currency: scalar(&record["currency"]),
                recipient_snapshot: record["recipient"].clone(),
                source_status: record["status"].clone(),
                source_paid_at: record["paidAt"].clone(),
                source_deposits_amount: record["depositsAmount"].clone(),
                pdf_keys,
                reconstructed_lines,
                line_evidence,
                deposit_deductions,
                unresolved,
            });
        }
        Ok(documents)
    }
}
// Poppler runs locally and receives archived bytes on stdin. No filenames, private
// text or tool diagnostics are included in errors. Missing tooling is an explicit gap.
fn pdf_layout(pdf: &[u8]) -> std::result::Result<String, &'static str> {
    if !pdf.starts_with(b"%PDF-") || pdf.len() > 20 * 1024 * 1024 {
        return Err("pdf_text_unavailable");
    }
    let mut process = Command::new("pdftotext")
        .args(["-layout", "-", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "pdf_text_tool_unavailable")?;
    let mut input = process.stdin.take().ok_or("pdf_text_unavailable")?;
    let bytes = pdf.to_vec();
    let writer = std::thread::spawn(move || input.write_all(&bytes));
    let output = process
        .wait_with_output()
        .map_err(|_| "pdf_text_unavailable")?;
    if writer.join().ok().and_then(|r| r.ok()).is_none()
        || !output.status.success()
        || output.stdout.len() > 2 * 1024 * 1024
    {
        return Err("pdf_text_unavailable");
    }
    String::from_utf8(output.stdout).map_err(|_| "pdf_text_unavailable")
}

fn number(text: &str) -> Option<Decimal> {
    let clean: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '€' && *c != '%')
        .collect();
    if clean.is_empty()
        || !clean
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, ',' | '.' | '-'))
    {
        return None;
    }
    Decimal::from_str_exact(&clean.replace(',', ".")).ok()
}

fn columns(line: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut whitespace = None;
    for (index, character) in line.char_indices() {
        if character.is_whitespace() {
            whitespace.get_or_insert(index);
        } else if let Some(begin) = whitespace.take()
            && line[begin..index].chars().count() >= 2
        {
            let value = line[start..begin].trim();
            if !value.is_empty() {
                parts.push(value);
            }
            start = index;
        }
    }
    let tail = line[start..].trim();
    if !tail.is_empty() {
        parts.push(tail);
    }
    parts
}

fn extract_lines(
    text: &str,
    record: &Value,
) -> std::result::Result<(Vec<HistoricalLine>, &'static str, Vec<HistoricalDeduction>), &'static str>
{
    if !record["currency"]
        .as_str()
        .is_some_and(|c| c.eq_ignore_ascii_case("EUR"))
    {
        return Err("line_currency_unsupported");
    }
    let uid = record["uid"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("line_source_number_missing")?;
    if !text.split_whitespace().any(|word| word == uid) {
        return Err("line_pdf_number_mismatch");
    }
    let mut active = false;
    let mut header = (false, false, false);
    let mut rows: Vec<HistoricalLine> = Vec::new();
    let mut description_columns = (0, 0);
    let mut amount_only = false;
    let mut deductions = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.contains("Description")
            && line.contains("Total")
            && !line.contains("Prix unitaire")
            && !line.contains("Quantité")
            && matches!(
                record["type"].as_str(),
                Some("Facture d’acompte" | "Facture d'acompte")
            )
        {
            active = true;
            amount_only = true;
            description_columns = (raw.find("Description").unwrap(), raw.find("Total").unwrap());
            continue;
        }
        if line.contains("Description")
            && line.contains("Prix unitaire")
            && line.contains("Quantité")
            && line.contains("Total")
        {
            active = true;
            amount_only = false;
            description_columns = (
                raw.find("Description").unwrap(),
                raw.find("Prix unitaire").unwrap(),
            );
            header = (
                line.starts_with("Type"),
                line.contains("TVA"),
                line.contains("Réduction"),
            );
            continue;
        }
        if !active || line.is_empty() {
            continue;
        }
        if line == "TVA non applicable, art. 293 B du CGI"
            && record["vatExempt"].as_bool() == Some(true)
        {
            active = false;
            continue;
        }
        let cells = columns(raw);
        let lower = cells[..cells.len().saturating_sub(1)]
            .join(" ")
            .to_lowercase();
        let summary = matches!(
            lower.as_str(),
            "total" | "total ht" | "total ttc" | "sous-total" | "sous-total ht" | "total à payer"
        ) || lower.starts_with("tva (")
            || lower.starts_with("réduction (")
            || lower.starts_with("remise (");
        if summary && cells.len() >= 2 && number(cells[cells.len() - 1]).is_some() {
            active = false;
            continue;
        }
        // A printed deduction remains separate evidence, never a negative sale line.
        if cells.len() == 2
            && (cells[0].starts_with("Facture d’acompte ")
                || cells[0].starts_with("Facture d'acompte "))
        {
            let value = number(cells[1])
                .filter(|v| *v < Decimal::ZERO)
                .ok_or("deposit_deduction_amount_ambiguous")?;
            let reference = cells[0].split_whitespace().find(|word| {
                word.strip_prefix("FA").is_some_and(|digits| {
                    digits.len() >= 9 && digits.bytes().all(|b| b.is_ascii_digit())
                })
            });
            deductions.push(HistoricalDeduction {
                description: cells[0].into(),
                reference_number: reference.map(str::to_owned),
                source_record_key: None,
                amount: value.to_string(),
            });
            continue;
        }
        // Description and Type continuations must stay inside their measured columns.
        let leading = raw.chars().take_while(|c| c.is_whitespace()).count();
        let end = raw.trim_end().chars().count();
        if end <= description_columns.1
            && let Some(previous) = rows.last_mut()
        {
            if cells.len() == 1 && leading >= description_columns.0 {
                previous.description.push('\n');
                previous.description.push_str(line);
                continue;
            }
            if header.0 && cells.len() == 2 && leading < description_columns.0 {
                let description: String = raw.chars().skip(description_columns.0).collect();
                let source_type: String = raw.chars().take(description_columns.0).collect();
                if !description.trim().is_empty()
                    && let Some(previous_type) = previous.source_type.as_mut()
                {
                    previous_type.push('\n');
                    previous_type.push_str(source_type.trim());
                    previous.description.push('\n');
                    previous.description.push_str(description.trim());
                    continue;
                }
            }
        }
        if amount_only {
            if cells.len() != 2 {
                return Err("amount_only_line_layout_ambiguous");
            }
            let net = number(cells[1])
                .filter(|v| *v >= Decimal::ZERO)
                .ok_or("amount_only_line_amount_ambiguous")?;
            rows.push(HistoricalLine {
                source_type: None,
                description: cells[0].into(),
                quantity: None,
                unit_price: None,
                discount_percent: None,
                vat_percent: (record["vatExempt"].as_bool() == Some(true)).then(|| "0".into()),
                net: net.to_string(),
            });
            continue;
        }
        let blank_discount = header.2
            && !header.1
            && cells.len() >= 4
            && cells[cells.len() - 3..]
                .iter()
                .all(|cell| number(cell).is_some())
            && !cells[cells.len() - 2].contains('%');
        let count = 3 + usize::from(header.1) + usize::from(header.2 && !blank_discount);
        if cells.len() <= count {
            return Err("line_layout_ambiguous_or_wrapped");
        }
        let split = cells.len() - count;
        if split > if header.0 { 2 } else { 1 } {
            return Err("line_description_ambiguous");
        }
        let values: Option<Vec<Decimal>> = cells[split..].iter().map(|c| number(c)).collect();
        let values = values.ok_or("line_layout_ambiguous_or_wrapped")?;
        let price = values[0];
        let quantity = values[1];
        let net = *values.last().unwrap();
        if price < Decimal::ZERO || quantity < Decimal::ZERO || net < Decimal::ZERO {
            return Err("line_negative_quantity");
        }
        let discount = if header.2 && !blank_discount {
            if !cells[split + 2].contains('%') {
                return Err("line_discount_unit_ambiguous");
            }
            Some(values[2])
        } else {
            None
        };
        let rate = if header.1 {
            Some(values[2])
        } else if record["vatExempt"].as_bool() == Some(true) {
            Some(Decimal::ZERO)
        } else {
            None
        };
        if header.1 && !cells[split + 2].contains('%') {
            return Err("line_vat_unit_ambiguous");
        }
        if header.1 && header.2 {
            return Err("line_combined_discount_tax_layout_unsupported");
        }
        if discount.is_some_and(|d| d < Decimal::ZERO || d > Decimal::from(100))
            || rate.is_some_and(|r| r < Decimal::ZERO || r > Decimal::from(100))
        {
            return Err("line_rate_out_of_range");
        }
        let computed =
            (price * quantity * (Decimal::ONE - discount.unwrap_or_default() / Decimal::from(100)))
                .round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);
        if computed != net {
            return Err("line_arithmetic_mismatch");
        }
        rows.push(HistoricalLine {
            source_type: (header.0 && split == 2).then(|| cells[0].to_owned()),
            description: cells[split - 1].to_owned(),
            quantity: Some(quantity.to_string()),
            unit_price: Some(price.to_string()),
            discount_percent: discount.map(|d| d.to_string()),
            vat_percent: rate.map(|d| d.to_string()),
            net: net.to_string(),
        });
    }
    if active {
        return Err("line_table_end_ambiguous");
    }
    if rows.is_empty() {
        return Err("line_table_not_found");
    }
    let sum: Decimal = rows.iter().map(|r| number(&r.net).unwrap()).sum();
    let expected = scalar(&record["pricePretax"])
        .and_then(|v| number(&v))
        .ok_or("line_source_net_missing")?;
    let discount = scalar(&record["reductionAmount"])
        .and_then(|v| number(&v))
        .unwrap_or_default();
    if rows.iter().all(|row| row.vat_percent.is_some()) {
        let mut buckets = std::collections::BTreeMap::<Decimal, Decimal>::new();
        for row in &rows {
            *buckets
                .entry(number(row.vat_percent.as_deref().unwrap()).unwrap())
                .or_default() += number(&row.net).unwrap();
        }
        // Global discounts may change tax bases; do not infer their allocation.
        if discount == Decimal::ZERO {
            let calculated_tax: Decimal = buckets
                .iter()
                .map(|(rate, net)| {
                    (*rate * *net / Decimal::from(100))
                        .round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
                })
                .sum();
            let source_tax = scalar(&record["taxAmount"])
                .and_then(|v| number(&v))
                .ok_or("line_source_tax_missing")?;
            if calculated_tax != source_tax {
                return Err("line_source_tax_mismatch");
            }
        }
    }
    let convention = if sum == expected {
        "line_sum_equals_source_net"
    } else if discount > Decimal::ZERO && sum - discount == expected {
        "line_sum_less_global_discount_equals_source_net"
    } else {
        return Err("line_source_total_mismatch");
    };
    let deducted: Decimal = deductions.iter().map(|d| -number(&d.amount).unwrap()).sum();
    let source_deposits = scalar(&record["depositsAmount"])
        .and_then(|v| number(&v))
        .unwrap_or_default();
    if deducted != source_deposits {
        return Err("deposit_deductions_source_total_mismatch");
    }
    Ok((rows, convention, deductions))
}

fn scalar(value: &Value) -> Option<String> {
    match value {
        Value::String(s) if !s.trim().is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reconstruction_is_idempotent_and_does_not_invent_payments_or_lines() -> Result<()> {
        let connection = rusqlite::Connection::open_in_memory()?;
        connection.execute_batch("CREATE TABLE facture_net_archive(source TEXT,category TEXT,source_key TEXT,content BLOB,record_link TEXT);")?;
        let record = serde_json::to_vec(
            &serde_json::json!({"uid":"TEST-1","type":"Facture d’acompte",
            "pricePretax":100,"taxAmount":0,"price":100,"currency":"EUR","recipient":{"company":"Example"},"paidAt":null}),
        )?;
        connection.execute(
            "INSERT INTO facture_net_archive VALUES ('test','invoices','1',?1,NULL)",
            [&record],
        )?;
        connection.execute("INSERT INTO facture_net_archive VALUES ('test','pdf','factures_acomptes/TEST-1.pdf',X'00','invoices:1')", [])?;
        let store = Store(connection);
        let first = store.reconcile_archive()?;
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].kind, "deposit");
        assert!(first[0].source_paid_at.is_null());
        assert_eq!(first[0].gross.as_deref(), Some("100"));
        assert_eq!(first[0].pdf_keys.len(), 1);
        assert!(
            first[0]
                .unresolved
                .contains(&"business_relationships_unconfirmed".into())
        );
        assert_eq!(
            serde_json::to_value(&first)?,
            serde_json::to_value(store.reconcile_archive()?)?
        );
        let count: i64 =
            store
                .0
                .query_row("SELECT COUNT(*) FROM facture_net_archive", [], |r| r.get(0))?;
        assert_eq!(count, 2);
        Ok(())
    }

    fn source() -> Value {
        serde_json::json!({"uid":"TEST-1", "currency":"eur", "pricePretax":100, "taxAmount":0, "vatExempt":true})
    }

    #[test]
    fn verifies_single_line_layout_and_preserves_exact_fields() {
        let text = "Facture TEST-1\nType   Description             Prix unitaire HT    Quantité     Total HT\nService   Travail exemple      50,00 €             2            100,00 €\nTotal HT   100,00 €";
        let (lines, convention, _) = extract_lines(text, &source()).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].description, "Travail exemple");
        assert_eq!(lines[0].source_type.as_deref(), Some("Service"));
        assert_eq!(lines[0].quantity.as_deref(), Some("2"));
        assert_eq!(lines[0].vat_percent.as_deref(), Some("0"));
        assert_eq!(convention, "line_sum_equals_source_net");
    }

    #[test]
    fn rejects_partial_wrapped_mismatched_and_wrong_identity_evidence() {
        let text = "TEST-1\nDescription     Prix unitaire HT    Quantité    Total HT\nTravail         50,00 €             2           100,00 €\nTotal HT    100,00 €";
        assert!(extract_lines(&text.replace("50,00", "40,00"), &source()).is_err());
        assert!(
            extract_lines(
                &text.replace("Total HT    100", "texte de continuation\nTotal HT    100"),
                &source()
            )
            .is_err()
        );
        assert!(extract_lines(&text.replace("TEST-1", "TEST-2"), &source()).is_err());
        let mut record = source();
        record["pricePretax"] = serde_json::json!(101);
        assert!(extract_lines(text, &record).is_err());
    }

    #[test]
    fn accepts_continuation_only_inside_description_column() {
        let text = "TEST-1\n    Description                  Prix unitaire HT    Quantité    Total HT\n    Travail                      50,00 €             2           100,00 €\n    Suite du descriptif\nTotal HT    100,00 €";
        let (lines, _, _) = extract_lines(text, &source()).unwrap();
        assert_eq!(lines[0].description, "Travail\nSuite du descriptif");
        assert!(extract_lines(&text.replace("    Suite", "Suite"), &source()).is_err());
    }

    #[test]
    fn verifies_fractional_prices_and_percentage_discount() {
        let text = "TEST-1\nDescription     Prix unitaire HT    Quantité    Réduction    Total HT\nTravail         55,555 €            2           10%          100,00 €\nTotal HT    100,00 €";
        let (lines, _, _) = extract_lines(text, &source()).unwrap();
        assert_eq!(lines[0].unit_price.as_deref(), Some("55.555"));
        assert_eq!(lines[0].discount_percent.as_deref(), Some("10"));
        assert!(extract_lines(&text.replace("10%", "10 €"), &source()).is_err());
    }

    #[test]
    fn amount_only_deposit_preserves_absent_quantity_and_unit_price() {
        let mut record = source();
        record["type"] = serde_json::json!("Facture d’acompte");
        let text = "TEST-1\nDescription                                       Total HT\nAcompte sur devis exemple                         100,00 €\nTotal    100,00 €";
        let (rows, _, _) = extract_lines(text, &record).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].quantity.is_none());
        assert!(rows[0].unit_price.is_none());
        assert_eq!(rows[0].net, "100.00");
        record["type"] = serde_json::json!("Facture");
        assert!(extract_lines(text, &record).is_err());
    }

    #[test]
    fn zero_quantity_is_historical_evidence_and_is_not_dropped() {
        let text = "TEST-1\nDescription           Prix unitaire HT   Quantité   Total HT\nPrestation            100,00 €           1          100,00 €\nOption non retenue    500,00 €           0          0,00 €\nTotal HT    100,00 €";
        let (rows, _, _) = extract_lines(text, &source()).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].quantity.as_deref(), Some("0"));
        assert_eq!(rows[1].net, "0.00");
        assert!(
            extract_lines(
                &text.replace("0          0,00", "0          1,00"),
                &source()
            )
            .is_err()
        );
    }

    #[test]
    fn preserves_wrapped_type_and_description_in_separate_columns() {
        let text = "TEST-1\nType    Description                  Prix unitaire HT   Quantité   Total HT\nService    Travail exemple           50,00 €           2          100,00 €\nsuite   Suite descriptive\nTotal HT    100,00 €";
        let (rows, _, _) = extract_lines(text, &source()).unwrap();
        assert_eq!(rows[0].source_type.as_deref(), Some("Service\nsuite"));
        assert_eq!(rows[0].description, "Travail exemple\nSuite descriptive");
    }

    #[test]
    fn blank_discount_cell_stays_absent_and_each_row_reconciles() {
        let mut record = source();
        record["pricePretax"] = serde_json::json!(190);
        let text = "TEST-1\nDescription     Prix unitaire HT    Quantité    Réduction    Total HT\nTravail A       100,00 €            1           10%          90,00 €\nTravail B       100,00 €            1                        100,00 €\nTotal HT    190,00 €";
        let (rows, _, _) = extract_lines(text, &record).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].discount_percent.as_deref(), Some("10"));
        assert!(rows[1].discount_percent.is_none());
    }

    #[test]
    fn deductions_are_separate_and_must_equal_exported_deposit_total() {
        let mut record = source();
        record["depositsAmount"] = serde_json::json!(30);
        let text = "TEST-1\nDescription     Prix unitaire HT    Quantité    Total HT\nPrestation      100,00 €            1           100,00 €\nFacture d’acompte FA112600001 du 18 novembre 2026    -30,00 €\nTotal    100,00 €";
        let (rows, _, deductions) = extract_lines(text, &record).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(deductions.len(), 1);
        assert_eq!(
            deductions[0].reference_number.as_deref(),
            Some("FA112600001")
        );
        assert_eq!(deductions[0].amount, "-30.00");
        record["depositsAmount"] = serde_json::json!(40);
        assert!(extract_lines(text, &record).is_err());
    }

    #[test]
    fn zero_source_tax_does_not_invent_an_unprinted_rate() {
        let mut record = source();
        record["vatExempt"] = serde_json::json!(false);
        let text = "TEST-1\nDescription     Prix unitaire HT    Quantité    Total HT\nPrestation      100,00 €            1           100,00 €\nTotal    100,00 €";
        let (rows, _, _) = extract_lines(text, &record).unwrap();
        assert!(rows[0].vat_percent.is_none());
    }

    #[test]
    fn absent_amounts_are_not_zero() {
        assert_eq!(scalar(&Value::Null), None);
        assert_eq!(scalar(&serde_json::json!(0)), Some("0".into()));
    }
}
