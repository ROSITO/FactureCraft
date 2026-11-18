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
        CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY, value TEXT NOT NULL);
        COMMIT;")?;
    // Added after the first release: a refused or cancelled quote.
    let closed: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('billing') WHERE name='closed')",
        [],
        |row| row.get(0),
    )?;
    if !closed {
        db.execute_batch(
            "ALTER TABLE billing ADD COLUMN closed TEXT; ALTER TABLE billing ADD COLUMN closed_date TEXT;",
        )?;
    }
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
    /// `refused` or `cancelled` for a closed quote.
    pub closed: Option<String>,
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
            "SELECT kind,number,accepted_date IS NOT NULL,source_id,version,closed FROM billing WHERE id=?1",
            [id], |row| Ok(BillingState {kind: row.get(0)?, number: row.get(1)?, accepted: row.get(2)?,
                source_id: row.get(3)?, version: row.get(4)?, paid: String::new(), deposits: String::new(), credited:String::new(),outstanding:String::new(), closed: row.get(5)?}))?;
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
        let changed = self.0.execute("UPDATE billing SET accepted_date=?2,version=version+1 WHERE id=?1 AND kind='quote' AND number IS NOT NULL AND accepted_date IS NULL AND closed IS NULL",params![id,date])?;
        ensure!(
            changed == 1,
            "Expected a finalized quote awaiting acceptance"
        );
        Ok(())
    }

    pub(crate) fn validate_date(&self, date: &str) -> Result<()> {
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
                    && state.closed.is_none()
                    && matches!(kind, "invoice" | "deposit"))
                    || (matches!(state.kind.as_str(), "invoice" | "deposit") && kind == "credit")),
            "Invalid document transition"
        );
        let mut content = self.quote(id)?;
        if matches!(kind, "invoice" | "deposit") && content.due_date.trim().is_empty() {
            // Editable default; finalization requires a due date.
            content.due_date = self.date_in_days(30)?;
        }
        let draft = self.write_quote(None, title, &source.currency, &source.language, &content)?;
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
        render: impl FnOnce(&Draft, &Quote, &str, &str) -> Result<Vec<u8>>,
    ) -> Result<String> {
        self.finalize(id, None, render)
    }

    pub fn finalize_document_versioned(
        &self,
        id: i64,
        expected_version: i64,
        render: impl FnOnce(&Draft, &Quote, &str, &str) -> Result<Vec<u8>>,
    ) -> Result<String> {
        self.finalize(id, Some(expected_version), render)
    }

    fn finalize(
        &self,
        id: i64,
        expected: Option<i64>,
        render: impl FnOnce(&Draft, &Quote, &str, &str) -> Result<Vec<u8>>,
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
        if matches!(state.kind.as_str(), "invoice" | "deposit") {
            self.validate_date(quote.due_date.trim())
                .context("An invoice needs a due date (YYYY-MM-DD)")?;
        }
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
        let prefix = prefix(&state.kind)?;
        let (month, year, issued_on): (String, String, String) = tx.query_row(
            "SELECT strftime('%m','now','localtime'), strftime('%Y','now','localtime'), date('now','localtime')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        let number = if self.live_numbering()?.is_some() {
            let short_year = &year[2..];
            // Continue the imported series: never reuse a counter already present in the archive.
            let imported = self.archived_counter(prefix, short_year)?;
            let next: i64 = tx.query_row(
                "INSERT INTO billing_sequence(kind,last) VALUES(?1,?2+1)
                ON CONFLICT(kind) DO UPDATE SET last=max(last,?2)+1 RETURNING last",
                params![format!("{}:{year}", state.kind), imported],
                |row| row.get(0),
            )?;
            ensure!(next <= 99_999, "Yearly counter exhausted");
            format!("{prefix}{month}{short_year}{next:05}")
        } else {
            let next: i64 = tx.query_row("INSERT INTO billing_sequence(kind,last) VALUES(?1,1) ON CONFLICT(kind) DO UPDATE SET last=last+1 RETURNING last", [&state.kind], |row| row.get(0))?;
            format!("TEST-{prefix}-{next:06}")
        };
        ensure!(
            !self.archived_number_exists(&number)?,
            "Number {number} already exists in the imported archive"
        );
        let issuer = serde_json::to_string(&self.issuer()?)?;
        tx.execute(
            "INSERT INTO issuer_snapshot(document_id,details) VALUES(?1,?2)",
            params![id, issuer],
        )?;
        let pdf = render(&draft, &quote, &number, &issued_on)?;
        ensure!(pdf.starts_with(b"%PDF-"), "Renderer did not return a PDF");
        tx.execute("UPDATE billing SET number=?1,pdf=?2,issued_at=CURRENT_TIMESTAMP,version=version+1 WHERE id=?3",params![number,pdf,id])?;
        tx.commit()?;
        Ok(number)
    }
    /// Date from which finalization assigns legal numbers, if enabled.
    pub fn live_numbering(&self) -> Result<Option<String>> {
        Ok(self
            .0
            .query_row(
                "SELECT value FROM settings WHERE key='live_numbering_since'",
                [],
                |row| row.get(0),
            )
            .optional()?)
    }

    /// Human decision: FactureCraft becomes the only issuer and continues the series.
    pub fn enable_live_numbering(&self) -> Result<()> {
        let issuer = self.issuer()?;
        ensure!(
            [&issuer.name, &issuer.address, &issuer.registration]
                .iter()
                .all(|field| !field.trim().is_empty()),
            "Issuer name, address and registration number are required before real issuance"
        );
        self.0.execute(
            "INSERT OR IGNORE INTO settings(key,value) VALUES('live_numbering_since',date('now','localtime'))",
            [],
        )?;
        Ok(())
    }

    /// Allowed only while no legal number has been assigned.
    pub fn disable_live_numbering(&self) -> Result<()> {
        let tx = self.billing_transaction()?;
        let issued: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM billing WHERE number IS NOT NULL AND number NOT LIKE 'TEST-%')",
            [],
            |row| row.get(0),
        )?;
        ensure!(!issued, "Real numbers have already been issued");
        tx.execute("DELETE FROM settings WHERE key='live_numbering_since'", [])?;
        tx.commit()?;
        Ok(())
    }

    /// Next legal number per document kind for this month, as finalization would assign it.
    pub fn next_numbers(&self) -> Result<Vec<(String, String)>> {
        let (month, year): (String, String) = self.0.query_row(
            "SELECT strftime('%m','now','localtime'), strftime('%Y','now','localtime')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let short_year = &year[2..];
        let mut numbers = Vec::new();
        for kind in ["quote", "invoice", "deposit", "credit"] {
            let prefix = prefix(kind)?;
            let local: i64 = self
                .0
                .query_row(
                    "SELECT last FROM billing_sequence WHERE kind=?1",
                    [format!("{kind}:{year}")],
                    |row| row.get(0),
                )
                .optional()?
                .unwrap_or(0);
            let next = local.max(self.archived_counter(prefix, short_year)?) + 1;
            numbers.push((kind.into(), format!("{prefix}{month}{short_year}{next:05}")));
        }
        Ok(numbers)
    }

    fn archive_numbers(&self) -> Result<Vec<String>> {
        if !self.has_archive()? {
            return Ok(Vec::new());
        }
        let mut query = self.0.prepare(
            "SELECT json_extract(CAST(content AS TEXT),'$.uid') FROM facture_net_archive
            WHERE category IN ('quotations','invoices')",
        )?;
        let rows = query.query_map([], |row| row.get::<_, Option<String>>(0))?;
        Ok(rows
            .filter_map(|row| row.transpose())
            .collect::<rusqlite::Result<_>>()?)
    }

    pub(crate) fn has_archive(&self) -> Result<bool> {
        Ok(self.0.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='facture_net_archive')",
            [],
            |row| row.get(0),
        )?)
    }

    fn archived_counter(&self, prefix: &str, short_year: &str) -> Result<i64> {
        Ok(self
            .archive_numbers()?
            .iter()
            .filter_map(|number| parse_number(number))
            .filter(|(p, _, year, _)| *p == prefix && *year == short_year)
            .map(|(_, _, _, counter)| counter)
            .max()
            .unwrap_or(0))
    }

    fn archived_number_exists(&self, number: &str) -> Result<bool> {
        Ok(self.archive_numbers()?.iter().any(|n| n == number))
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

fn prefix(kind: &str) -> Result<&'static str> {
    Ok(match kind {
        "quote" => "D",
        "invoice" => "F",
        "deposit" => "FA",
        "credit" => "A",
        _ => anyhow::bail!("Unknown document kind"),
    })
}

/// Splits `<prefix><mm><yy><counter>` as used by Facture.net, e.g. `FA012600001`.
fn parse_number(number: &str) -> Option<(&str, &str, &str, i64)> {
    let split = number.find(|c: char| c.is_ascii_digit())?;
    let (prefix, digits) = number.split_at(split);
    (digits.len() >= 9 && digits.bytes().all(|b| b.is_ascii_digit())).then(|| {
        (
            prefix,
            &digits[..2],
            &digits[2..4],
            digits[4..].parse().unwrap_or(0),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quote::Line;
    fn example(price: &str) -> Quote {
        Quote {
            recipient: "Client de test".into(),
            due_date: "2026-12-31".into(),
            lines: vec![Line {
                description: "Prestation".into(),
                price: price.into(),
                ..Default::default()
            }],
            ..Default::default()
        }
    }
    fn render(_: &Draft, _: &Quote, number: &str, _: &str) -> Result<Vec<u8>> {
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
                .finalize_document(quote.id, |_, _, _, _| anyhow::bail!("renderer failed"))
                .is_err()
        );
        assert_eq!(store.finalize_document(quote.id, render)?, "TEST-D-000001");
        let pdf = store.finalized_pdf(quote.id)?;
        assert_eq!(
            store.finalize_document(quote.id, |_, _, _, _| unreachable!())?,
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

    #[test]
    fn live_numbering_continues_the_imported_series() -> Result<()> {
        let store = Store::open(std::path::Path::new(":memory:"))?;
        let (month, year): (String, String) = store.0.query_row(
            "SELECT strftime('%m','now','localtime'), substr(strftime('%Y','now','localtime'),3)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        store.0.execute_batch(&format!(
            "CREATE TABLE facture_net_archive (source TEXT, category TEXT, source_key TEXT, content BLOB, record_link TEXT);
            INSERT INTO facture_net_archive VALUES
              ('s','invoices','1',CAST('{{\"uid\":\"F01{year}00003\",\"type\":\"Facture\",\"createdAt\":1,\"pricePretax\":100.5}}' AS BLOB),NULL),
              ('s','invoices','2',CAST('{{\"uid\":\"F019900042\"}}' AS BLOB),NULL);"
        ))?;
        let quote = store.save_quote(None, "Facture", "EUR", "fr", &example("200"))?;
        store
            .0
            .execute("UPDATE billing SET kind='invoice' WHERE id=?1", [quote.id])?;
        assert!(
            store.enable_live_numbering().is_err(),
            "issuer is incomplete"
        );
        store.save_issuer(&crate::issuer::Issuer {
            name: "Émetteur".into(),
            address: "Adresse".into(),
            registration: "000".into(),
            ..Default::default()
        })?;
        store.enable_live_numbering()?;
        let expected = format!("F{month}{year}00004");
        assert!(
            store
                .next_numbers()?
                .contains(&("invoice".into(), expected.clone()))
        );
        let mut dated = String::new();
        let number = store.finalize_document(quote.id, |_, _, number, date| {
            dated = date.into();
            render(&quote_draft(), &Quote::default(), number, date)
        })?;
        assert_eq!(number, expected);
        assert_eq!(dated.len(), 10);
        assert!(
            store.disable_live_numbering().is_err(),
            "a legal number exists"
        );
        let board = store.dashboard(dated[..4].parse()?, "eur")?;
        assert_eq!(board.invoices, 1);
        assert_eq!(board.revenue_year, "200.00");
        assert_eq!(board.outstanding, "200.00");
        assert_eq!(board.activity[0].number, expected);
        assert_eq!(
            store.dashboard_currency("chf")?,
            "EUR",
            "opens where the data is"
        );
        assert_eq!(store.dashboard_currency("eur")?, "EUR");
        Ok(())
    }

    fn quote_draft() -> Draft {
        Draft {
            id: 0,
            title: String::new(),
            currency: "EUR".into(),
            language: "fr".into(),
        }
    }
}
