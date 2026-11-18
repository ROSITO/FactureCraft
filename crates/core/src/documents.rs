//! Everyday document actions that never touch a finalized document's content.
use crate::{Draft, Store};
use anyhow::{Context, Result, bail, ensure};
use rusqlite::params;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Payment {
    pub id: i64,
    pub amount: String,
    pub date: String,
    pub method: String,
    pub reference: String,
}

impl Store {
    /// Deletes a draft. Finalized documents are kept forever.
    pub fn delete_draft(&self, id: i64) -> Result<()> {
        let tx = self.0.unchecked_transaction()?;
        let number: Option<String> = tx
            .query_row("SELECT number FROM billing WHERE id=?1", [id], |row| {
                row.get(0)
            })
            .context("Document not found")?;
        ensure!(number.is_none(), "A finalized document cannot be deleted");
        let derived: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM billing WHERE source_id=?1)",
            [id],
            |row| row.get(0),
        )?;
        ensure!(!derived, "Other documents refer to this draft");
        let has_requests: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='draft_requests')",
            [],
            |row| row.get(0),
        )?;
        if has_requests {
            tx.execute("DELETE FROM draft_requests WHERE draft_id=?1", [id])?;
        }
        for sql in [
            "DELETE FROM billing_allocation WHERE invoice_id=?1",
            "DELETE FROM billing_payment WHERE document_id=?1",
            "DELETE FROM issuer_snapshot WHERE document_id=?1",
            "DELETE FROM quote_details WHERE draft_id=?1",
            "DELETE FROM billing WHERE id=?1",
            "DELETE FROM drafts WHERE id=?1",
        ] {
            tx.execute(sql, [id])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// New draft with the same content: a quote from a quote, an invoice from an invoice.
    pub fn duplicate_document(&self, id: i64, title: &str) -> Result<Draft> {
        let source = self.get(id)?.context("Document not found")?;
        let kind = self.billing_state(id)?.kind;
        ensure!(
            matches!(kind.as_str(), "quote" | "invoice"),
            "Only quotes and invoices can be duplicated"
        );
        let mut quote = self.quote(id)?;
        quote.due_date.clear();
        quote.service_date.clear();
        let tx = self.0.unchecked_transaction()?;
        let draft = self.write_quote(None, title, &source.currency, &source.language, &quote)?;
        tx.execute(
            "UPDATE billing SET kind=?1 WHERE id=?2",
            params![kind, draft.id],
        )?;
        tx.commit()?;
        Ok(draft)
    }

    /// Records that a finalized quote was refused or cancelled.
    pub fn close_quote(&self, id: i64, status: &str, date: &str) -> Result<()> {
        ensure!(
            matches!(status, "refused" | "cancelled"),
            "Status must be refused or cancelled"
        );
        self.validate_date(date)?;
        let changed = self.0.execute(
            "UPDATE billing SET closed=?2, closed_date=?3, version=version+1
            WHERE id=?1 AND kind='quote' AND number IS NOT NULL AND accepted_date IS NULL AND closed IS NULL",
            params![id, status, date],
        )?;
        if changed != 1 {
            bail!("Expected a finalized quote that is neither accepted nor closed");
        }
        Ok(())
    }

    pub fn payments(&self, document: i64) -> Result<Vec<Payment>> {
        let mut query = self.0.prepare(
            "SELECT id,amount,date,method,reference FROM billing_payment WHERE document_id=?1 ORDER BY date,id",
        )?;
        let rows = query.query_map([document], |row| {
            Ok(Payment {
                id: row.get(0)?,
                amount: row.get(1)?,
                date: row.get(2)?,
                method: row.get(3)?,
                reference: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Removes a payment recorded by mistake; the invoice itself is unchanged.
    pub fn delete_payment(&self, payment: i64) -> Result<()> {
        ensure!(
            self.0
                .execute("DELETE FROM billing_payment WHERE id=?1", [payment])?
                == 1,
            "Payment not found"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quote::{Line, Quote};

    fn quote() -> Quote {
        Quote {
            recipient: "Client".into(),
            due_date: "2026-12-31".into(),
            lines: vec![Line {
                description: "Prestation".into(),
                price: "100".into(),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn pdf(_: &Draft, _: &Quote, number: &str, _: &str) -> Result<Vec<u8>> {
        Ok(format!("%PDF-1.4 {number}").into_bytes())
    }

    #[test]
    fn drafts_can_be_deleted_and_duplicated_but_not_finalized_documents() -> Result<()> {
        let store = Store::open(std::path::Path::new(":memory:"))?;
        let draft = store.create_quote_idempotent("k", "Devis", "EUR", "fr", &quote())?;
        let copy = store.duplicate_document(draft.id, "Copie")?;
        assert_eq!(store.quote(copy.id)?.lines.len(), 1);
        assert!(store.quote(copy.id)?.due_date.is_empty());
        store.delete_draft(copy.id)?;
        assert!(store.get(copy.id)?.is_none());

        store.finalize_document(draft.id, pdf)?;
        assert!(store.delete_draft(draft.id).is_err());
        store.close_quote(draft.id, "refused", "2026-11-18")?;
        assert_eq!(
            store.billing_state(draft.id)?.closed.as_deref(),
            Some("refused")
        );
        assert!(store.accept_quote(draft.id, "2026-11-18").is_err());
        assert!(
            store
                .close_quote(draft.id, "cancelled", "2026-11-18")
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn invoices_need_a_due_date_and_payments_can_be_corrected() -> Result<()> {
        let store = Store::open(std::path::Path::new(":memory:"))?;
        let mut content = quote();
        content.due_date.clear();
        let invoice = store.create_invoice("Facture", "EUR", "fr")?;
        store.save_quote(Some(invoice.id), "Facture", "EUR", "fr", &content)?;
        assert!(store.finalize_document(invoice.id, pdf).is_err());
        content.due_date = "2026-12-31".into();
        store.save_quote(Some(invoice.id), "Facture", "EUR", "fr", &content)?;
        store.finalize_document(invoice.id, pdf)?;
        store.record_payment(invoice.id, "40", "2026-11-18", "virement", "VIR-1")?;
        let payments = store.payments(invoice.id)?;
        assert_eq!(payments[0].reference, "VIR-1");
        store.delete_payment(payments[0].id)?;
        assert_eq!(store.billing_state(invoice.id)?.outstanding, "100.00");
        Ok(())
    }
}
