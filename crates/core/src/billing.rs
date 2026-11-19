use crate::{Draft, Store, idempotency, quote::Quote};
use anyhow::{Context, Result, ensure};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use rust_decimal::Decimal;
use serde::Serialize;

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

    /// Invoice draft from an accepted quote keeping only the lines at `line_indexes`.
    pub fn derive_invoice_with_lines(
        &self,
        id: i64,
        expected_version: i64,
        line_indexes: &[usize],
        title: &str,
    ) -> Result<Draft> {
        let shape = Shape::Lines(line_indexes.to_vec());
        self.derive_shaped(id, Some(expected_version), "invoice", title, None, &shape)
    }

    /// Deposit draft for a share of an accepted quote (see [`Share`]).
    pub fn derive_deposit(
        &self,
        id: i64,
        expected_version: i64,
        share: &Share,
        title: &str,
    ) -> Result<Draft> {
        let shape = Shape::Deposit(share.clone());
        self.derive_shaped(id, Some(expected_version), "deposit", title, None, &shape)
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
        self.derive_shaped(id, expected, kind, title, key, &Shape::All)
    }
    fn derive_shaped(
        &self,
        id: i64,
        expected: Option<i64>,
        kind: &str,
        title: &str,
        key: Option<&str>,
        shape: &Shape,
    ) -> Result<Draft> {
        let tx = self.billing_transaction()?;
        let mut request = serde_json::to_string(&("derive", id, expected, kind, title))?;
        if !matches!(shape, Shape::All) {
            request.push_str(&format!("{shape:?}"));
        }
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
        if kind == "credit" && state.kind == "invoice" && matches!(shape, Shape::All) {
            self.remaining_credit_lines(id, &source.currency, &source.language, &mut content)?;
        }
        match shape {
            Shape::All => {}
            Shape::Lines(indexes) => {
                content.lines = select_lines(&content.lines, indexes)?;
            }
            Shape::Deposit(share) => {
                let number = state.number.as_deref().unwrap_or_default();
                deposit_lines(
                    &mut content,
                    &source.currency,
                    &source.language,
                    number,
                    share,
                )?;
            }
        }
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

    /// Build a credit for the remaining invoice amount by tax rate and exemption reason.
    /// Called inside the same immediate transaction as draft creation and version checking.
    fn remaining_credit_lines(
        &self,
        invoice: i64,
        currency: &str,
        language: &str,
        content: &mut Quote,
    ) -> Result<()> {
        use std::collections::BTreeMap;
        type Key = (Decimal, String);
        let mut remaining = BTreeMap::<Key, (Decimal, Decimal)>::new();
        let add = |map: &mut BTreeMap<Key, (Decimal, Decimal)>,
                   quote: &Quote,
                   sign: Decimal|
         -> Result<()> {
            for bucket in quote.tax_breakdown(currency)? {
                let key = (
                    Decimal::from_str_exact(&bucket.rate)?.normalize(),
                    bucket.exemption_reason,
                );
                let values = map.entry(key).or_default();
                values.0 += sign * Decimal::from_str_exact(&bucket.net)?;
                values.1 += sign * Decimal::from_str_exact(&bucket.tax)?;
            }
            Ok(())
        };
        add(&mut remaining, content, Decimal::ONE)?;
        let mut deposits = self
            .0
            .prepare("SELECT deposit_id FROM billing_allocation WHERE invoice_id=?1")?;
        let deposits = deposits
            .query_map([invoice], |r| r.get::<_, i64>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for deposit in deposits {
            add(&mut remaining, &self.quote(deposit)?, -Decimal::ONE)?;
            let mut credits = self.0.prepare("SELECT id FROM billing WHERE source_id=?1 AND kind='credit' AND number IS NOT NULL")?;
            let credits = credits
                .query_map([deposit], |r| r.get::<_, i64>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            for credit in credits {
                add(&mut remaining, &self.quote(credit)?, Decimal::ONE)?;
            }
        }
        let mut credits = self.0.prepare(
            "SELECT id FROM billing WHERE source_id=?1 AND kind='credit' AND number IS NOT NULL",
        )?;
        let credits = credits
            .query_map([invoice], |r| r.get::<_, i64>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for credit in credits {
            add(&mut remaining, &self.quote(credit)?, -Decimal::ONE)?;
        }
        let mut lines = Vec::new();
        for ((rate, reason), (net, tax)) in &remaining {
            ensure!(
                *net >= Decimal::ZERO && *tax >= Decimal::ZERO,
                "Remaining credit tax breakdown requires manual reconciliation"
            );
            if net.is_zero() && tax.is_zero() {
                continue;
            }
            lines.push(crate::quote::Line {
                description: if language == "en" {
                    "Remaining creditable amount"
                } else {
                    "Montant restant à créditer"
                }
                .into(),
                price: net.to_string(),
                vat: rate.to_string(),
                exemption_reason: reason.clone(),
                ..Default::default()
            });
        }
        ensure!(!lines.is_empty(), "No amount remains to credit");
        content.lines = lines;
        content.global_discount.clear();
        let actual = content.tax_breakdown(currency)?;
        for bucket in actual {
            let key = (
                Decimal::from_str_exact(&bucket.rate)?.normalize(),
                bucket.exemption_reason,
            );
            let expected = remaining.get(&key).context("Missing credit tax bucket")?;
            ensure!(
                Decimal::from_str_exact(&bucket.net)? == expected.0
                    && Decimal::from_str_exact(&bucket.tax)? == expected.1,
                "Remaining credit tax breakdown requires manual reconciliation"
            );
        }
        Ok(())
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
        if state.kind == "credit"
            && let Some(source) = state.source_id
        {
            tx.execute("UPDATE billing SET version=version+1 WHERE id=?1", [source])?;
        }
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

/// How a derived document is built from its source.
#[derive(Debug)]
enum Shape {
    All,
    Lines(Vec<usize>),
    Deposit(Share),
}

/// Part of a quote billed by a deposit: a percentage of its gross total or a gross amount.
#[derive(Clone, Debug)]
pub enum Share {
    Percent(String),
    Amount(String),
}

fn select_lines(
    lines: &[crate::quote::Line],
    indexes: &[usize],
) -> Result<Vec<crate::quote::Line>> {
    let mut sorted = indexes.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    ensure!(!sorted.is_empty(), "Select at least one line");
    ensure!(
        sorted.iter().all(|&index| index < lines.len()),
        "Unknown line selected"
    );
    Ok(sorted
        .into_iter()
        .map(|index| lines[index].clone())
        .collect())
}

/// Replaces the lines of `content` by the requested share of the quote.
///
/// The share is applied to the net of every tax bucket (already net of the global discount), so
/// each bucket keeps its own rate or exemption reason and VAT stays exact. A quote with a single
/// rate, or none, yields a single line.
fn deposit_lines(
    content: &mut Quote,
    currency: &str,
    language: &str,
    number: &str,
    share: &Share,
) -> Result<()> {
    let hundred = Decimal::from(100);
    let parse = |text: &str| -> Result<Decimal> {
        Ok(Decimal::from_str_exact(&text.trim().replace(',', "."))?)
    };
    let buckets = content.tax_breakdown(currency)?;
    let gross = Decimal::from_str_exact(&content.totals(currency)?.2)?;
    ensure!(gross > Decimal::ZERO, "The quote has no amount to bill");
    let ratio = match share {
        Share::Percent(text) => parse(text)? / hundred,
        Share::Amount(text) => parse(text)? / gross,
    };
    ensure!(
        ratio > Decimal::ZERO && ratio <= Decimal::ONE,
        "A deposit must be above 0 and at most the quote total"
    );
    let percent = (ratio * hundred).round_dp(2).normalize();
    let multiple = buckets.len() > 1;
    let mut lines = Vec::new();
    for bucket in buckets {
        let net = Decimal::from_str_exact(&bucket.net)?;
        let rate = Decimal::from_str_exact(&bucket.rate)?;
        let mut description = if language == "en" {
            format!("Deposit of {percent}% on quote {number}")
        } else {
            format!("Acompte de {percent} % sur devis {number}")
        };
        if multiple {
            description.push_str(&format!(
                " ({} {rate} %)",
                if language == "en" { "VAT" } else { "TVA" }
            ));
        }
        lines.push(crate::quote::Line {
            description,
            // Unit prices carry up to six decimals; the line total rounds to the currency.
            price: (net * ratio).round_dp(6).normalize().to_string(),
            vat: rate.to_string(),
            exemption_reason: bucket.exemption_reason,
            ..Default::default()
        });
    }
    content.lines = lines;
    content.global_discount.clear();
    Ok(())
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
    fn credit_defaults_to_remaining_tax_buckets_after_deposit_and_prior_credit() -> Result<()> {
        let store = Store::open(std::path::Path::new(":memory:"))?;
        let mut quote = example("100");
        quote.lines[0].vat = "20".into();
        quote.lines.push(Line {
            description: "Reduced rate".into(),
            price: "100".into(),
            vat: "10".into(),
            ..Default::default()
        });
        let source = store.save_quote(None, "Quote", "EUR", "fr", &quote)?;
        store.finalize_document(source.id, render)?;
        store.accept_quote(source.id, "2026-11-18")?;
        let version = store.billing_state(source.id)?.version;
        let deposit =
            store.derive_deposit(source.id, version, &Share::Percent("30".into()), "Deposit")?;
        store.finalize_document(deposit.id, render)?;
        let invoice = store.derive_document(source.id, "invoice", "Invoice")?;
        store.allocate_deposit(invoice.id, deposit.id)?;
        store.finalize_document(invoice.id, render)?;
        let version = store.billing_state(invoice.id)?.version;
        assert!(
            store
                .derive_document_versioned(invoice.id, version - 1, "credit", "Stale")
                .is_err()
        );
        let first = store.derive_document_versioned(invoice.id, version, "credit", "Credit")?;
        let first_quote = store.quote(first.id)?;
        assert_eq!(
            first_quote.totals("EUR")?,
            ("140.00".into(), "21.00".into(), "161.00".into())
        );
        let mut partial = first_quote;
        partial.lines.retain(|line| line.vat == "20");
        partial.lines[0].price = "10".into();
        store.save_quote_versioned(first.id, 1, "Partial", "EUR", "fr", &partial)?;
        store.finalize_document(first.id, render)?;
        assert!(
            store
                .derive_document_versioned(invoice.id, version, "credit", "Stale after credit")
                .is_err()
        );
        let next = store.derive_document(invoice.id, "credit", "Remaining")?;
        let next_quote = store.quote(next.id)?;
        assert_eq!(
            next_quote.totals("EUR")?,
            ("130.00".into(), "19.00".into(), "149.00".into())
        );
        store.finalize_document(next.id, render)?;
        assert!(
            store
                .derive_document(invoice.id, "credit", "Excess")
                .is_err()
        );
        Ok(())
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

    fn accepted_quote(store: &Store, quote: &Quote) -> Result<i64> {
        let id = store.save_quote(None, "Devis", "EUR", "fr", quote)?.id;
        store.finalize_document(id, render)?;
        store.accept_quote(id, "2026-11-18")?;
        Ok(id)
    }

    fn mixed_quote() -> Quote {
        let line = |description: &str, price: &str, vat: &str, reason: &str| crate::quote::Line {
            description: description.into(),
            price: price.into(),
            vat: vat.into(),
            exemption_reason: reason.into(),
            ..Default::default()
        };
        Quote {
            recipient: "Client de test".into(),
            lines: vec![
                line("A", "100", "20", ""),
                line("B", "50", "10", ""),
                line("C", "30", "0", "Motif de test"),
            ],
            ..Default::default()
        }
    }

    #[test]
    fn invoice_keeps_only_selected_lines() -> Result<()> {
        let store = Store::open(std::path::Path::new(":memory:"))?;
        let id = accepted_quote(&store, &mixed_quote())?;
        let version = store.billing_state(id)?.version;
        assert!(
            store
                .derive_invoice_with_lines(id, version, &[], "F")
                .is_err()
        );
        assert!(
            store
                .derive_invoice_with_lines(id, version, &[3], "F")
                .is_err()
        );
        assert!(
            store
                .derive_invoice_with_lines(id, version + 1, &[0], "F")
                .is_err()
        );
        let invoice = store.derive_invoice_with_lines(id, version, &[2, 0, 0], "F")?;
        let content = store.quote(invoice.id)?;
        assert_eq!(content.lines.len(), 2);
        assert_eq!(content.lines[0].description, "A");
        assert_eq!(
            content.totals("EUR")?,
            ("130.00".into(), "20.00".into(), "150.00".into())
        );
        let state = store.billing_state(invoice.id)?;
        assert_eq!(
            (state.kind.as_str(), state.source_id),
            ("invoice", Some(id))
        );
        Ok(())
    }

    #[test]
    fn deposit_percentage_follows_each_tax_bucket() -> Result<()> {
        let store = Store::open(std::path::Path::new(":memory:"))?;
        let id = accepted_quote(&store, &mixed_quote())?;
        let version = store.billing_state(id)?.version;
        // Quote: net 180.00, VAT 25.00, gross 205.00.
        let deposit = store.derive_deposit(id, version, &Share::Percent("30".into()), "Acompte")?;
        let content = store.quote(deposit.id)?;
        assert_eq!(content.lines.len(), 3);
        assert_eq!(
            content.lines[1].description,
            "Acompte de 30 % sur devis TEST-D-000001 (TVA 10 %)"
        );
        assert_eq!(content.lines[0].exemption_reason, "Motif de test");
        // 30 % of each bucket: 15 + 30 + 9 net, VAT 1.50 + 6.00.
        assert_eq!(
            content.totals("EUR")?,
            ("54.00".into(), "7.50".into(), "61.50".into())
        );
        assert!(!content.due_date.is_empty());
        assert!(
            store
                .derive_deposit(id, version, &Share::Percent("0".into()), "A")
                .is_err()
        );
        assert!(
            store
                .derive_deposit(id, version, &Share::Percent("100.01".into()), "A")
                .is_err()
        );
        assert!(
            store
                .derive_deposit(id, version, &Share::Amount("205.01".into()), "A")
                .is_err()
        );
        let full = store.derive_deposit(id, version, &Share::Percent("100".into()), "A")?;
        assert_eq!(store.quote(full.id)?.totals("EUR")?.2, "205.00");
        Ok(())
    }

    #[test]
    fn deposit_amount_on_single_rate_quote_is_exact_to_the_cent() -> Result<()> {
        let store = Store::open(std::path::Path::new(":memory:"))?;
        let id = accepted_quote(&store, &example("100"))?; // 100.00 at 0 % without exemption
        let version = store.billing_state(id)?.version;
        let deposit = store.derive_deposit(id, version, &Share::Amount("33,33".into()), "A")?;
        let content = store.quote(deposit.id)?;
        assert_eq!(content.lines.len(), 1);
        assert_eq!(
            content.lines[0].description,
            "Acompte de 33.33 % sur devis TEST-D-000001"
        );
        assert_eq!(content.totals("EUR")?.2, "33.33");
        let vat_quote = Quote {
            lines: vec![crate::quote::Line {
                description: "A".into(),
                price: "100".into(),
                vat: "20".into(),
                ..Default::default()
            }],
            ..example("1")
        };
        let id = accepted_quote(&store, &vat_quote)?;
        let version = store.billing_state(id)?.version;
        let deposit = store.derive_deposit(id, version, &Share::Amount("50".into()), "A")?;
        // 50.00 gross is 41.666667 net, rounded to 41.67 with 8.33 VAT.
        assert_eq!(
            store.quote(deposit.id)?.totals("EUR")?,
            ("41.67".into(), "8.33".into(), "50.00".into())
        );
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
            "INSERT INTO facture_net_archive VALUES
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
