//! Tabular export of all documents for spreadsheets and accounting tools.
use crate::Store;
use anyhow::Result;
use rust_decimal::Decimal;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct DocumentRow {
    pub id: i64,
    pub kind: String,
    pub number: String,
    /// `draft`, `issued`, `accepted`, `refused`, `cancelled` or `paid`.
    pub status: String,
    pub title: String,
    pub recipient: String,
    pub currency: String,
    pub language: String,
    pub issue_date: String,
    pub due_date: String,
    pub net: String,
    pub tax: String,
    pub gross: String,
    /// Empty for quotes.
    pub paid: String,
    /// Empty for quotes.
    pub outstanding: String,
}

const HEADER: [&str; 15] = [
    "id",
    "kind",
    "number",
    "status",
    "title",
    "recipient",
    "currency",
    "language",
    "issue_date",
    "due_date",
    "net",
    "tax",
    "gross",
    "paid",
    "outstanding",
];

impl DocumentRow {
    fn fields(&self) -> [String; 15] {
        [
            self.id.to_string(),
            self.kind.clone(),
            self.number.clone(),
            self.status.clone(),
            self.title.clone(),
            self.recipient.clone(),
            self.currency.clone(),
            self.language.clone(),
            self.issue_date.clone(),
            self.due_date.clone(),
            self.net.clone(),
            self.tax.clone(),
            self.gross.clone(),
            self.paid.clone(),
            self.outstanding.clone(),
        ]
    }
}

impl Store {
    pub fn document_rows(&self) -> Result<Vec<DocumentRow>> {
        let ids: Vec<(i64, String)> = {
            let mut query = self.0.prepare(
                "SELECT d.id, COALESCE(substr(b.issued_at,1,10),'') FROM drafts d
                 JOIN billing b ON b.id=d.id ORDER BY d.id",
            )?;
            query
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?
        };
        let mut rows = Vec::with_capacity(ids.len());
        for (id, issue_date) in ids {
            let snapshot = self.document_snapshot(id, false)?;
            let (draft, state, quote) = (snapshot.draft, snapshot.billing, snapshot.quote);
            let (net, tax, gross) = quote.totals(&draft.currency)?;
            let status = if let Some(closed) = &state.closed {
                closed.clone()
            } else if state.number.is_none() {
                "draft".into()
            } else if state.kind == "quote" {
                if state.accepted { "accepted" } else { "issued" }.into()
            } else if Decimal::from_str_exact(&state.outstanding)? <= Decimal::ZERO {
                "paid".into()
            } else {
                "issued".into()
            };
            let is_quote = state.kind == "quote";
            rows.push(DocumentRow {
                id,
                kind: state.kind,
                number: state.number.unwrap_or_default(),
                status,
                title: draft.title,
                recipient: quote.recipient,
                currency: draft.currency,
                language: draft.language,
                issue_date,
                due_date: quote.due_date,
                net,
                tax,
                gross,
                paid: if is_quote { String::new() } else { state.paid },
                outstanding: if is_quote {
                    String::new()
                } else {
                    state.outstanding
                },
            });
        }
        Ok(rows)
    }

    /// RFC 4180 CSV, UTF-8 with a byte order mark so Excel reads accents correctly.
    pub fn export_documents_csv(&self) -> Result<Vec<u8>> {
        let mut out = String::from('\u{feff}');
        push_record(&mut out, HEADER.iter().map(|s| s.to_string()));
        for row in self.document_rows()? {
            push_record(&mut out, row.fields());
        }
        Ok(out.into_bytes())
    }

    pub fn export_documents_json(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec_pretty(&self.document_rows()?)?)
    }
}

fn push_record(out: &mut String, fields: impl IntoIterator<Item = String>) {
    let mut first = true;
    for field in fields {
        if !first {
            out.push(',');
        }
        first = false;
        if field.contains([',', '"', '\n', '\r']) {
            out.push('"');
            out.push_str(&field.replace('"', "\"\""));
            out.push('"');
        } else {
            out.push_str(&field);
        }
    }
    out.push_str("\r\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quote::{Line, Quote};

    #[test]
    fn csv_quotes_special_characters_and_exports_amounts() -> Result<()> {
        let store = Store::open(std::path::Path::new(":memory:"))?;
        let quote = Quote {
            recipient: "Dupont, \"Fils\"\nSARL é".into(),
            lines: vec![Line {
                description: "Prestation".into(),
                price: "100".into(),
                vat: "20".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        store.save_quote(None, "Devis, \"A\"", "EUR", "fr", &quote)?;
        let csv = String::from_utf8(store.export_documents_csv()?)?;
        assert!(csv.starts_with('\u{feff}'));
        let body = csv.trim_start_matches('\u{feff}');
        assert!(body.starts_with("id,kind,number,status,title,"));
        assert!(body.contains("\"Devis, \"\"A\"\"\",\"Dupont, \"\"Fils\"\"\nSARL é\""));
        assert!(body.contains(",100.00,20.00,120.00,,\r\n"));
        let json: serde_json::Value = serde_json::from_slice(&store.export_documents_json()?)?;
        assert_eq!(json[0]["gross"], "120.00");
        assert_eq!(json[0]["status"], "draft");
        assert_eq!(json[0]["recipient"], "Dupont, \"Fils\"\nSARL é");
        Ok(())
    }

    #[test]
    fn invoice_status_follows_payments() -> Result<()> {
        let store = Store::open(std::path::Path::new(":memory:"))?;
        let quote = Quote {
            recipient: "C".into(),
            due_date: "2026-12-31".into(),
            lines: vec![Line {
                description: "P".into(),
                price: "100".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let invoice = store.create_invoice("F", "EUR", "en")?;
        store.save_quote(Some(invoice.id), "F", "EUR", "en", &quote)?;
        store.finalize_document(invoice.id, |_, _, n, _| {
            Ok(format!("%PDF-1.4 {n}").into_bytes())
        })?;
        let row = &store.document_rows()?[0];
        assert_eq!(
            (row.status.as_str(), row.outstanding.as_str()),
            ("issued", "100.00")
        );
        assert_eq!(row.issue_date.len(), 10);
        store.record_payment(invoice.id, "100", "2026-11-18", "wire", "R")?;
        let row = &store.document_rows()?[0];
        assert_eq!((row.status.as_str(), row.paid.as_str()), ("paid", "100"));
        Ok(())
    }
}
