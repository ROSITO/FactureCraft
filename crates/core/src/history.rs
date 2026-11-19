//! Imported Facture.net documents shown next to live documents, read-only.
use crate::{
    Draft, Store,
    quote::{Line, Quote},
};
use anyhow::{Context, Result};
use rusqlite::params;
use serde::Serialize;

/// One archived quote, invoice, deposit or credit note, as recorded by the source.
#[derive(Clone, Debug, Serialize)]
pub struct ArchivedListing {
    pub source: String,
    pub key: String,
    /// `quote`, `invoice`, `deposit` or `credit`.
    pub kind: &'static str,
    /// Source number, or `Provisoire` for an unfinalized source quote.
    pub number: String,
    /// `YYYY-MM-DD`, empty when the source has no date.
    pub date: String,
    pub recipient: String,
    pub currency: String,
    /// Amounts as recorded by the source, before and after tax.
    pub net: String,
    pub gross: String,
    /// `draft`, `finalized`, `signed`, `paid` or `rectified`, from the source.
    pub status: String,
    /// Key of the original PDF in the archive, when one is linked.
    pub pdf: Option<String>,
}

impl Store {
    /// Archived documents, newest first. Fast: reads the stored records only.
    pub fn archived_documents(&self) -> Result<Vec<ArchivedListing>> {
        if !self.has_archive()? {
            return Ok(Vec::new());
        }
        let mut query = self.0.prepare(
            "SELECT a.source, a.category, a.source_key,
                coalesce(json_extract(c,'$.type'),''),
                coalesce(json_extract(c,'$.uid'),''),
                CASE WHEN coalesce(json_extract(c,'$.createdAt'),0) > 0
                     THEN date(json_extract(c,'$.createdAt'),'unixepoch','localtime') ELSE '' END,
                coalesce(nullif(json_extract(c,'$.recipient.company'),''),
                    trim(coalesce(json_extract(c,'$.recipient.firstName'),'')||' '||coalesce(json_extract(c,'$.recipient.lastName'),''))),
                upper(coalesce(json_extract(c,'$.currency'),'EUR')),
                CAST(coalesce(json_extract(c,'$.pricePretax'),0) AS TEXT),
                CAST(coalesce(json_extract(c,'$.price'),json_extract(c,'$.pricePretax'),0) AS TEXT),
                coalesce(json_extract(c,'$.status'),''),
                json_extract(c,'$.signedAt') IS NOT NULL,
                (SELECT p.source_key FROM facture_net_archive p
                  WHERE p.source=a.source AND p.category='pdf'
                    AND p.record_link=a.category||':'||a.source_key LIMIT 1)
            FROM (SELECT *, CAST(content AS TEXT) AS c FROM facture_net_archive) a
            WHERE a.category IN ('quotations','invoices')
            ORDER BY 6 DESC, 5 DESC",
        )?;
        let rows = query.query_map([], |row| {
            let category: String = row.get(1)?;
            let kind: String = row.get(3)?;
            let number: String = row.get(4)?;
            let status: String = row.get(10)?;
            let signed: bool = row.get(11)?;
            let status = match (category.as_str(), status.as_str()) {
                ("quotations", _) if number.eq_ignore_ascii_case("provisoire") => "draft",
                ("quotations", _) if signed => "signed",
                ("quotations", _) => "finalized",
                (_, "paid") => "paid",
                (_, "rectified") => "rectified",
                _ => "finalized",
            };
            Ok(ArchivedListing {
                source: row.get(0)?,
                key: row.get(2)?,
                kind: crate::dashboard::archive_kind(&category, Some(kind.as_str())),
                number: if number.is_empty() {
                    "Provisoire".into()
                } else {
                    number
                },
                date: row.get(5)?,
                recipient: row.get(6)?,
                currency: row.get(7)?,
                net: decimal_text(&row.get::<_, String>(8)?),
                gross: decimal_text(&row.get::<_, String>(9)?),
                status: status.into(),
                pdf: row.get(12)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Archived document found by its source number (case-insensitive).
    pub fn archived_by_number(&self, number: &str) -> Result<ArchivedListing> {
        self.archived_documents()?
            .into_iter()
            .find(|doc| doc.number.eq_ignore_ascii_case(number.trim()))
            .with_context(|| format!("No archived document numbered {}", number.trim()))
    }

    /// New draft (quote, or invoice for an archived invoice) taking the recipient and
    /// the lines reconstructed from an archived document. The archive is unchanged.
    pub fn draft_from_archive(
        &self,
        listing: &ArchivedListing,
        defaults: Quote,
        title: &str,
    ) -> Result<Draft> {
        let quote = self.quote_from_archive(listing, defaults)?;
        let tx = self.0.unchecked_transaction()?;
        let draft = self.write_quote(None, title, &listing.currency, "fr", &quote)?;
        if listing.kind != "quote" {
            tx.execute(
                "UPDATE billing SET kind='invoice' WHERE id=?1",
                params![draft.id],
            )?;
        }
        tx.commit()?;
        self.get(draft.id)?.context("Draft not found")
    }

    /// Recipient and reconstructed lines of an archived document, as editable quote
    /// content, without saving anything. Lines printed with an amount only become 1 × amount.
    pub fn quote_from_archive(&self, listing: &ArchivedListing, defaults: Quote) -> Result<Quote> {
        let category = if listing.kind == "quote" {
            "quotations"
        } else {
            "invoices"
        };
        let record = self.archive_record(&listing.source, category, &listing.key)?;
        let recipient = &record["recipient"];
        let text = |key: &str| recipient[key].as_str().unwrap_or("").trim().to_string();
        let mut quote = defaults;
        quote.recipient = if text("company").is_empty() {
            format!("{} {}", text("firstName"), text("lastName"))
                .trim()
                .into()
        } else {
            text("company")
        };
        let city = format!(
            "{} {}",
            text(if recipient["zip_code"].is_string() {
                "zip_code"
            } else {
                "zipCode"
            }),
            text("city")
        );
        let mut address = vec![text("street")];
        if let Some(extra) = recipient["additional_address_lines"].as_array() {
            address.extend(extra.iter().filter_map(|l| l.as_str()).map(str::to_string));
        }
        address.push(city.trim().into());
        quote.recipient_address = address
            .into_iter()
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        quote.recipient_country = text("country");
        quote.recipient_registration = text("company_number");
        quote.recipient_email = text("email");
        // Untaxed source documents keep their exemption mention on the copied lines.
        let untaxed = record["taxAmount"].as_f64() == Some(0.0);
        let exemption = if untaxed {
            "TVA non applicable, art. 293 B du CGI"
        } else {
            ""
        };
        let reconstructed = self
            .reconcile_archive()?
            .into_iter()
            .find(|document| document.source == listing.source && document.key == listing.key)
            .map(|document| document.reconstructed_lines)
            .unwrap_or_default();
        quote.lines = reconstructed
            .into_iter()
            .map(|line| {
                // A line printed with an amount only becomes 1 × that amount, to be reviewed.
                let price = line.unit_price.unwrap_or_else(|| line.net.clone());
                let vat = line.vat_percent.unwrap_or_else(|| "0".into());
                Line {
                    description: line.description,
                    quantity: line.quantity.unwrap_or_else(|| "1".into()),
                    price: decimal_text(&price),
                    discount: line.discount_percent.unwrap_or_else(|| "0".into()),
                    exemption_reason: if decimal_text(&vat) == "0" {
                        exemption.into()
                    } else {
                        String::new()
                    },
                    vat,
                    ..Default::default()
                }
            })
            .collect();
        if quote.lines.is_empty() {
            quote.lines.push(Line {
                description: format!("Reprise de {}", listing.number),
                price: listing.net.clone(),
                exemption_reason: exemption.into(),
                ..Default::default()
            });
        }
        Ok(quote)
    }
}

/// `250.0` → `250`, `19.5` → `19.5`: JSON numbers turned into exact decimal text.
fn decimal_text(value: &str) -> String {
    let value = value.trim();
    match value.parse::<rust_decimal::Decimal>() {
        Ok(number) => number.normalize().to_string(),
        Err(_) => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_with_archive() -> Result<Store> {
        let store = Store::open(std::path::Path::new(":memory:"))?;
        let quote = r#"{"id":1,"uid":"D012700001","createdAt":1798761600,"signedAt":1798848000,"currency":"eur","pricePretax":1200.0,"price":1200.0,"taxAmount":0.0,"recipient":{"company":"Atelier Exemple","street":"12 rue de l'Exemple","zip_code":"75001","city":"PARIS","country":"France","company_number":"123456789","email":"contact@example.com"}}"#;
        let invoice = r#"{"id":2,"uid":"F012700001","type":"Facture","status":"paid","createdAt":1798934400,"currency":"eur","pricePretax":1200.0,"price":1440.0,"taxAmount":240.0,"recipient":{"company":"Atelier Exemple"}}"#;
        store.0.execute_batch(&format!(
            "INSERT INTO facture_net_archive VALUES
              ('s','quotations','1',CAST('{}' AS BLOB),NULL),
              ('s','invoices','2',CAST('{}' AS BLOB),NULL),
              ('s','pdf','devis/D012700001.pdf',CAST('%PDF-1.4' AS BLOB),'quotations:1');",
            quote.replace('\'', "''"),
            invoice.replace('\'', "''")
        ))?;
        Ok(store)
    }

    #[test]
    fn lists_archived_documents_with_status_and_pdf() -> Result<()> {
        let store = store_with_archive()?;
        let list = store.archived_documents()?;
        assert_eq!(list.len(), 2);
        let invoice = list.iter().find(|d| d.kind == "invoice").unwrap();
        assert_eq!(
            (invoice.number.as_str(), invoice.status.as_str()),
            ("F012700001", "paid")
        );
        assert_eq!(
            (invoice.net.as_str(), invoice.gross.as_str()),
            ("1200", "1440")
        );
        assert_eq!(invoice.currency, "EUR");
        let quote = list.iter().find(|d| d.kind == "quote").unwrap();
        assert_eq!(quote.status, "signed");
        assert_eq!(quote.recipient, "Atelier Exemple");
        assert_eq!(quote.pdf.as_deref(), Some("devis/D012700001.pdf"));
        assert!(invoice.pdf.is_none());
        assert_eq!(list[0].kind, "invoice", "newest first");
        Ok(())
    }

    #[test]
    fn agent_reuses_an_archived_invoice_as_an_idempotent_invoice_draft() -> Result<()> {
        let store = store_with_archive()?;
        let listing = store.archived_by_number("f012700001")?;
        let mut quote = store.quote_from_archive(&listing, Quote::default())?;
        assert_eq!(quote.recipient, "Atelier Exemple");
        quote.lines[0].description = "Abonnement 2027".into();
        let first = store.create_invoice_idempotent("k", "Facture 2027", "EUR", "fr", &quote)?;
        let again = store.create_invoice_idempotent("k", "Facture 2027", "EUR", "fr", &quote)?;
        assert_eq!(first.id, again.id, "replaying the key gives the same draft");
        let state = store.billing_state(first.id)?;
        assert_eq!(
            (state.kind.as_str(), state.number.is_none()),
            ("invoice", true)
        );
        assert_eq!(
            store.quote(first.id)?.due_date.len(),
            10,
            "default due date"
        );
        quote.due_date = "2027-02-30".into();
        assert!(
            store
                .create_invoice_idempotent("k2", "X", "EUR", "fr", &quote)
                .is_err()
        );
        assert!(store.archived_by_number("F999").is_err());
        Ok(())
    }

    #[test]
    fn archived_quote_becomes_an_editable_draft_and_the_archive_stays() -> Result<()> {
        let store = store_with_archive()?;
        let listing = store
            .archived_documents()?
            .into_iter()
            .find(|d| d.kind == "quote")
            .unwrap();
        let draft = store.draft_from_archive(&listing, Quote::default(), "Copie")?;
        let quote = store.quote(draft.id)?;
        assert_eq!(quote.recipient, "Atelier Exemple");
        assert_eq!(quote.recipient_address, "12 rue de l'Exemple\n75001 PARIS");
        assert_eq!(quote.recipient_registration, "123456789");
        assert_eq!(
            quote.lines.len(),
            1,
            "fallback line when lines are not reconstructed"
        );
        assert_eq!(quote.lines[0].price, "1200");
        assert_eq!(
            quote.lines[0].exemption_reason,
            "TVA non applicable, art. 293 B du CGI"
        );
        assert_eq!(store.billing_state(draft.id)?.kind, "quote");
        assert!(store.billing_state(draft.id)?.number.is_none());
        assert_eq!(store.archived_documents()?.len(), 2);
        Ok(())
    }
}
