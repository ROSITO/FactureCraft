use crate::{Draft, Store, idempotency, quote::Quote};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use rust_decimal::Decimal;
use serde::Serialize;

pub(crate) fn initialize(db: &Connection) -> Result<()> {
    db.execute_batch("PRAGMA foreign_keys=ON;
        BEGIN IMMEDIATE;
        CREATE TABLE IF NOT EXISTS billing (
            id INTEGER PRIMARY KEY REFERENCES drafts(id), kind TEXT NOT NULL DEFAULT 'quote',
            version INTEGER NOT NULL DEFAULT 1, source_id INTEGER REFERENCES drafts(id),
            accepted_date TEXT, number TEXT UNIQUE, pdf BLOB, issued_at TEXT);
        INSERT OR IGNORE INTO billing(id) SELECT id FROM drafts;
        CREATE TRIGGER IF NOT EXISTS billing_new AFTER INSERT ON drafts
          BEGIN INSERT INTO billing(id) VALUES(NEW.id); END;
        CREATE TRIGGER IF NOT EXISTS billing_edit BEFORE UPDATE ON drafts
          BEGIN SELECT RAISE(ABORT,'Finalized document is immutable') WHERE
            EXISTS(SELECT 1 FROM billing WHERE id=OLD.id AND number IS NOT NULL); END;
        CREATE TRIGGER IF NOT EXISTS billing_version AFTER UPDATE ON drafts
          BEGIN UPDATE billing SET version=version+1 WHERE id=NEW.id; END;
        CREATE TABLE IF NOT EXISTS billing_sequence(kind TEXT PRIMARY KEY, last INTEGER NOT NULL);
        CREATE TABLE IF NOT EXISTS billing_payment(id INTEGER PRIMARY KEY, document_id INTEGER NOT NULL REFERENCES billing(id),
            amount TEXT NOT NULL, date TEXT NOT NULL, method TEXT NOT NULL, reference TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS billing_allocation(deposit_id INTEGER PRIMARY KEY REFERENCES billing(id),
            invoice_id INTEGER NOT NULL REFERENCES billing(id), amount TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS issuer_profile(id INTEGER PRIMARY KEY CHECK(id=1), details TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS issuer_snapshot(document_id INTEGER PRIMARY KEY REFERENCES billing(id), details TEXT NOT NULL);
        COMMIT;")?;
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct BillingState {
    pub kind: String,
    pub number: Option<String>,
    pub accepted: bool,
    pub source_id: Option<i64>,
    pub paid: String,
    pub version: i64,
    pub deposits: String,
    pub credited: String,
    pub outstanding: String,
}

pub struct DocumentSnapshot {
    pub draft: Draft,
    pub billing: BillingState,
    pub quote: Quote,
    pub finalized_pdf: Option<Vec<u8>>,
}

impl Store {
    pub fn document_snapshot(&self, id: i64, include_pdf: bool) -> Result<DocumentSnapshot> {
        let transaction = self.0.unchecked_transaction()?;
        let draft = self.get(id)?.context("Document not found")?;
        let billing = self.billing_state(id)?;
        let quote = self.quote(id)?;
        let finalized_pdf = (include_pdf && billing.number.is_some())
            .then(|| self.finalized_pdf(id))
            .transpose()?;
        transaction.commit()?;
        Ok(DocumentSnapshot {
            draft,
            billing,
            quote,
            finalized_pdf,
        })
    }

    fn billing_transaction(&self) -> Result<Transaction<'_>> {
        Ok(Transaction::new_unchecked(
            &self.0,
            TransactionBehavior::Immediate,
        )?)
    }

    pub fn billing_state(&self, id: i64) -> Result<BillingState> {
        let mut state = self.0.query_row(
            "SELECT kind,number,accepted_date IS NOT NULL,source_id,version FROM billing WHERE id=?1",
            [id], |row| Ok(BillingState {kind: row.get(0)?, number: row.get(1)?, accepted: row.get(2)?,
                source_id: row.get(3)?, version: row.get(4)?, paid: String::new(), deposits: String::new(), credited:String::new(),outstanding:String::new()}))?;
        state.paid = self
            .sum_amounts(
                "SELECT amount FROM billing_payment WHERE document_id=?1",
                id,
            )?
            .to_string();
        state.deposits = self
            .sum_amounts(
                "SELECT amount FROM billing_allocation WHERE invoice_id=?1",
                id,
            )?
            .to_string();
        let mut credits = self.0.prepare(
            "SELECT id FROM billing WHERE kind='credit' AND number IS NOT NULL AND source_id=?1",
        )?;
        let mut credited = Decimal::ZERO;
        for id in credits.query_map([id], |r| r.get::<_, i64>(0))? {
            credited += self.gross(id?)?;
        }
        state.credited = credited.to_string();
        state.outstanding = (self.gross(id)?
            - credited
            - Decimal::from_str_exact(&state.deposits)?
            - Decimal::from_str_exact(&state.paid)?)
        .to_string();
        Ok(state)
    }

    fn sum_amounts(&self, sql: &str, id: i64) -> Result<Decimal> {
        let mut statement = self.0.prepare(sql)?;
        let mut sum = Decimal::ZERO;
        for raw in statement.query_map([id], |r| r.get::<_, String>(0))? {
            sum += Decimal::from_str_exact(&raw?)?;
        }
        Ok(sum)
    }

    fn require_version(&self, id: i64, expected: i64) -> Result<()> {
        ensure!(
            self.billing_state(id)?.version == expected,
            "Stale document version"
        );
        Ok(())
    }

    pub fn save_quote_versioned(
        &self,
        id: i64,
        expected_version: i64,
        title: &str,
        currency: &str,
        language: &str,
        quote: &Quote,
    ) -> Result<Draft> {
        let tx = self.billing_transaction()?;
        self.require_version(id, expected_version)?;
        let draft = self.write_quote(Some(id), title, currency, language, quote)?;
        tx.commit()?;
        Ok(draft)
    }

    pub fn create_quote_idempotent(
        &self,
        key: &str,
        title: &str,
        currency: &str,
        language: &str,
        quote: &Quote,
    ) -> Result<Draft> {
        let request = serde_json::to_string(&("create_quote", title, currency, language, quote))?;
        let tx = self.billing_transaction()?;
        if let Some(id) = idempotency::lookup(&tx, key, &request)? {
            return self.get(id)?.context("Document not found");
        }
        let draft = self.write_quote(None, title, currency, language, quote)?;
        idempotency::record(&tx, key, &request, draft.id)?;
        tx.commit()?;
        Ok(draft)
    }

    pub fn accept_quote(&self, id: i64, date: &str) -> Result<()> {
        self.validate_date(date)?;
        let changed = self.0.execute("UPDATE billing SET accepted_date=?2,version=version+1 WHERE id=?1 AND kind='quote' AND number IS NOT NULL AND accepted_date IS NULL",params![id,date])?;
        ensure!(
            changed == 1,
            "Expected a finalized quote awaiting acceptance"
        );
        Ok(())
    }

    fn validate_date(&self, date: &str) -> Result<()> {
        let valid: bool = self.0.query_row(
            "SELECT length(?1)=10 AND date(?1,'+0 days') IS ?1",
            [date],
            |row| row.get(0),
        )?;
        ensure!(valid, "Expected a valid date YYYY-MM-DD");
        Ok(())
    }

    pub fn derive_document(&self, id: i64, kind: &str, title: &str) -> Result<Draft> {
        self.derive(id, None, kind, title, None)
    }
    pub fn derive_document_versioned(
        &self,
        id: i64,
        expected_version: i64,
        kind: &str,
        title: &str,
    ) -> Result<Draft> {
        self.derive(id, Some(expected_version), kind, title, None)
    }
    pub fn derive_document_idempotent(
        &self,
        id: i64,
        expected_version: i64,
        kind: &str,
        title: &str,
        key: &str,
    ) -> Result<Draft> {
        self.derive(id, Some(expected_version), kind, title, Some(key))
    }
    fn derive(
        &self,
        id: i64,
        expected: Option<i64>,
        kind: &str,
        title: &str,
        key: Option<&str>,
    ) -> Result<Draft> {
        let tx = self.billing_transaction()?;
        let request = serde_json::to_string(&("derive", id, expected, kind, title))?;
        if let Some(key) = key
            && let Some(previous) = idempotency::lookup(&tx, key, &request)?
        {
            return self.get(previous)?.context("Document not found");
        }
        if let Some(expected) = expected {
            self.require_version(id, expected)?;
        }
        let source = self.get(id)?.context("Document not found")?;
        let state = self.billing_state(id)?;
        ensure!(
            state.number.is_some()
                && ((state.kind == "quote"
                    && state.accepted
                    && matches!(kind, "invoice" | "deposit"))
                    || (matches!(state.kind.as_str(), "invoice" | "deposit") && kind == "credit")),
            "Invalid document transition"
        );
        let draft = self.write_quote(
            None,
            title,
            &source.currency,
            &source.language,
            &self.quote(id)?,
        )?;
        tx.execute(
            "UPDATE billing SET kind=?1, source_id=?2 WHERE id=?3",
            params![kind, id, draft.id],
        )?;
        if let Some(key) = key {
            idempotency::record(&tx, key, &request, draft.id)?;
        }
        tx.commit()?;
        Ok(draft)
    }

    pub fn create_invoice(&self, title: &str, currency: &str, language: &str) -> Result<Draft> {
        let tx = self.billing_transaction()?;
        let draft = self.create(title, currency, language)?;
        tx.execute("UPDATE billing SET kind='invoice' WHERE id=?1", [draft.id])?;
        tx.commit()?;
        Ok(draft)
    }

    fn gross(&self, id: i64) -> Result<Decimal> {
        let draft = self.get(id)?.context("Document not found")?;
        Ok(Decimal::from_str_exact(
            &self.quote(id)?.totals(&draft.currency)?.2,
        )?)
    }

    pub fn allocate_deposit(&self, invoice: i64, deposit: i64) -> Result<()> {
        let tx = self.billing_transaction()?;
        let target = self.billing_state(invoice)?;
        let source = self.billing_state(deposit)?;
        ensure!(
            target.kind == "invoice"
                && target.number.is_none()
                && source.kind == "deposit"
                && source.number.is_some()
                && source.source_id.is_some()
                && source.source_id == target.source_id,
            "Expected linked invoice draft and finalized deposit"
        );
        ensure!(
            self.get(invoice)?.context("Invoice not found")?.currency
                == self.get(deposit)?.context("Deposit not found")?.currency,
            "Currency mismatch"
        );
        let amount = self.gross(deposit)? - Decimal::from_str_exact(&source.credited)?;
        ensure!(amount > Decimal::ZERO, "Deposit has no amount to allocate");
        ensure!(
            amount + Decimal::from_str_exact(&target.deposits)? <= self.gross(invoice)?,
            "Deposits exceed invoice total"
        );
        tx.execute(
            "INSERT INTO billing_allocation(deposit_id,invoice_id,amount) VALUES (?1,?2,?3)",
            params![deposit, invoice, amount.to_string()],
        )?;
        tx.execute(
            "UPDATE billing SET version=version+1 WHERE id=?1",
            [invoice],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn record_payment(
        &self,
        id: i64,
        amount: &str,
        date: &str,
        method: &str,
        reference: &str,
    ) -> Result<()> {
        let tx = self.billing_transaction()?;
        let state = self.billing_state(id)?;
        ensure!(
            state.number.is_some() && matches!(state.kind.as_str(), "invoice" | "deposit"),
            "Expected a finalized invoice or deposit"
        );
        self.validate_date(date)?;
        let currency: iso_currency::Currency = self
            .get(id)?
            .context("Document not found")?
            .currency
            .parse()
            .map_err(|_| anyhow::anyhow!("Unknown currency"))?;
        let amount = Decimal::from_str_exact(&amount.trim().replace(',', "."))?;
        ensure!(
            amount > Decimal::ZERO
                && amount.scale() <= u32::from(currency.exponent().context("Unknown precision")?)
                && !method.trim().is_empty(),
            "Invalid payment amount or method"
        );
        ensure!(
            amount <= Decimal::from_str_exact(&state.outstanding)?,
            "Payment exceeds outstanding balance"
        );
        tx.execute("INSERT INTO billing_payment(document_id,amount,date,method,reference) VALUES(?1,?2,?3,?4,?5)",params![id,amount.to_string(),date,method,reference])?;
        tx.commit()?;
        Ok(())
    }

    pub fn finalize_document(
        &self,
        id: i64,
        render: impl FnOnce(&Draft, &Quote, &str) -> Result<Vec<u8>>,
    ) -> Result<String> {
        self.finalize(id, None, render)
    }

    pub fn finalize_document_versioned(
        &self,
        id: i64,
        expected_version: i64,
        render: impl FnOnce(&Draft, &Quote, &str) -> Result<Vec<u8>>,
    ) -> Result<String> {
        self.finalize(id, Some(expected_version), render)
    }

    fn finalize(
        &self,
        id: i64,
        expected: Option<i64>,
        render: impl FnOnce(&Draft, &Quote, &str) -> Result<Vec<u8>>,
    ) -> Result<String> {
        let tx = self.billing_transaction()?;
        let state = self.billing_state(id)?;
        if let Some(number) = state.number {
            return Ok(number);
        }
        if let Some(expected) = expected {
            self.require_version(id, expected)?;
        }
        let draft = self.get(id)?.context("Document not found")?;
        let quote = self.quote(id)?;
        ensure!(
            !quote.recipient.trim().is_empty() && !quote.lines.is_empty(),
            "Recipient and at least one line are required"
        );
        ensure!(
            self.gross(id)? >= Decimal::from_str_exact(&state.deposits)?,
            "Deposits exceed invoice total"
        );
        if let Some(source) = state.source_id {
            ensure!(
                self.get(source)?.context("Source missing")?.currency == draft.currency,
                "Currency differs from source"
            );
        }
        if state.kind == "credit" {
            let source = state.source_id.context("Credit must refer to an invoice")?;
            let allocated: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM billing_allocation WHERE deposit_id=?1)",
                [source],
                |r| r.get(0),
            )?;
            ensure!(
                !allocated,
                "Allocated deposit cannot be credited separately"
            );
            let mut prior = Decimal::ZERO;
            let mut credits=tx.prepare("SELECT id FROM billing WHERE source_id=?1 AND kind='credit' AND number IS NOT NULL")?;
            for row in credits.query_map([source], |r| r.get::<_, i64>(0))? {
                prior += self.gross(row?)?;
            }
            ensure!(
                self.gross(id)? + prior
                    <= self.gross(source)?
                        - Decimal::from_str_exact(&self.billing_state(source)?.deposits)?,
                "Credits exceed invoiced amount"
            );
        }
        let prefix = match state.kind.as_str() {
            "quote" => "D",
            "invoice" => "F",
            "deposit" => "FA",
            "credit" => "A",
            _ => anyhow::bail!("Unknown document kind"),
        };
        let next:i64=tx.query_row("INSERT INTO billing_sequence(kind,last) VALUES(?1,1) ON CONFLICT(kind) DO UPDATE SET last=last+1 RETURNING last",[&state.kind],|row|row.get(0))?;
        let number = format!("TEST-{prefix}-{next:06}");
        let issuer = serde_json::to_string(&self.issuer()?)?;
        tx.execute(
            "INSERT INTO issuer_snapshot(document_id,details) VALUES(?1,?2)",
            params![id, issuer],
        )?;
        let pdf = render(&draft, &quote, &number)?;
        ensure!(pdf.starts_with(b"%PDF-"), "Renderer did not return a PDF");
        tx.execute("UPDATE billing SET number=?1,pdf=?2,issued_at=CURRENT_TIMESTAMP,version=version+1 WHERE id=?3",params![number,pdf,id])?;
        tx.commit()?;
        Ok(number)
    }
    pub fn finalized_pdf(&self, id: i64) -> Result<Vec<u8>> {
        self.0
            .query_row(
                "SELECT pdf FROM billing WHERE id=?1 AND number IS NOT NULL",
                [id],
                |row| row.get(0),
            )
            .optional()?
            .context("Finalized PDF unavailable")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quote::Line;
    fn example(price: &str) -> Quote {
        Quote {
            recipient: "Client de test".into(),
            lines: vec![Line {
                description: "Prestation".into(),
                price: price.into(),
                ..Default::default()
            }],
            ..Default::default()
        }
    }
    fn render(_: &Draft, _: &Quote, number: &str) -> Result<Vec<u8>> {
        Ok(format!("%PDF-1.4 {number}").into_bytes())
    }

    #[test]
    fn concurrent_confirmation_assigns_one_number() -> Result<()> {
        let path = std::env::temp_dir().join(format!(
            "facturecraft-race-{}-{}.sqlite3",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        let store = Store::open(&path)?;
        let id = store
            .save_quote(None, "Exemple", "EUR", "fr", &example("10"))?
            .id;
        let peer = Store::open(&path)?;
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let start = barrier.clone();
        let other = std::thread::spawn(move || {
            start.wait();
            peer.finalize_document(id, render)
        });
        barrier.wait();
        let first = store.finalize_document(id, render)?;
        assert_eq!(first, other.join().unwrap()?);
        assert_eq!(
            store.0.query_row(
                "SELECT last FROM billing_sequence WHERE kind='quote'",
                [],
                |r| r.get::<_, i64>(0)
            )?,
            1
        );
        drop(store);
        std::fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn simulation_distinguishes_payments_and_deposits_and_preserves_frozen_bytes() -> Result<()> {
        let store = Store::open(std::path::Path::new(":memory:"))?;
        let quote = store.create_quote_idempotent("q", "Devis", "EUR", "fr", &example("15000"))?;
        assert_eq!(
            quote.id,
            store
                .create_quote_idempotent("q", "Devis", "EUR", "fr", &example("15000"))?
                .id
        );
        assert!(
            store
                .finalize_document(quote.id, |_, _, _| anyhow::bail!("renderer failed"))
                .is_err()
        );
        assert_eq!(store.finalize_document(quote.id, render)?, "TEST-D-000001");
        let pdf = store.finalized_pdf(quote.id)?;
        assert_eq!(
            store.finalize_document(quote.id, |_, _, _| unreachable!())?,
            "TEST-D-000001"
        );
        assert!(
            store
                .save_quote(Some(quote.id), "Edited", "EUR", "fr", &example("1"))
                .is_err()
        );
        assert!(store.accept_quote(quote.id, "2026-02-31").is_err());
        store.accept_quote(quote.id, "2026-11-18")?;
        let invoice = store.derive_document(quote.id, "invoice", "Solde")?;
        for _ in 0..2 {
            let deposit = store.derive_document(quote.id, "deposit", "Acompte")?;
            store.save_quote_versioned(deposit.id, 1, "Acompte", "EUR", "fr", &example("4500"))?;
            assert!(
                store
                    .save_quote_versioned(deposit.id, 1, "Périmé", "EUR", "fr", &example("3"))
                    .is_err()
            );
            store.finalize_document(deposit.id, render)?;
            store.allocate_deposit(invoice.id, deposit.id)?;
            assert!(store.allocate_deposit(invoice.id, deposit.id).is_err());
        }
        let state = store.billing_state(invoice.id)?;
        assert_eq!(
            Decimal::from_str_exact(&state.deposits)?,
            Decimal::from(9000)
        );
        assert_eq!(state.paid, "0");
        assert!(
            store
                .finalize_document_versioned(invoice.id, 1, render)
                .is_err()
        );
        store.finalize_document_versioned(invoice.id, state.version, render)?;
        assert!(
            store
                .record_payment(invoice.id, "6000.01", "2026-11-18", "virement", "")
                .is_err()
        );
        store.record_payment(invoice.id, "6000", "2026-11-18", "virement", "exemple")?;
        assert_eq!(store.billing_state(invoice.id)?.paid, "6000");
        assert_eq!(store.finalized_pdf(quote.id)?, pdf);
        Ok(())
    }
}
