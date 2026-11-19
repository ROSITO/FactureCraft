use crate::Store;
use anyhow::{Result, ensure};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Issuer {
    pub name: String,
    pub address: String,
    pub email: String,
    pub registration: String,
    pub vat_id: String,
    /// Creditor IBAN for structured bank-transfer exports.
    pub iban: String,
    pub terms: String,
    pub contact: String,
    pub country: String,
    pub phone: String,
    /// Defaults copied into new documents.
    pub payment_terms: String,
    pub payment_method: String,
    pub quote_validity: String,
    /// General terms of sale, printed on their own page of quotes.
    pub cgv: String,
    /// Also print the general terms of sale on invoices.
    pub cgv_on_invoices: bool,
    /// Late-payment clause printed on invoices; empty uses [`DEFAULT_LATE_PENALTIES`].
    pub late_penalties: String,
}

/// Mentions required on French B2B invoices (Code de commerce, art. L441-10 and D441-5).
pub const DEFAULT_LATE_PENALTIES_EN: &str = "Late payment: penalties at the European Central Bank refinancing rate plus 10 points and a fixed recovery indemnity of EUR 40 (French Commercial Code, art. L441-10 and D441-5). No discount for early payment.";

pub const DEFAULT_LATE_PENALTIES: &str = "En cas de retard de paiement, pénalités au taux de refinancement de la BCE majoré de 10 points et indemnité forfaitaire pour frais de recouvrement de 40 € (art. L441-10 et D441-5 du Code de commerce). Pas d’escompte pour paiement anticipé.";

impl Issuer {
    /// The issuer's clause, or the default in the document language.
    pub fn late_penalties(&self, language: &str) -> &str {
        if self.late_penalties.trim().is_empty() {
            if language == "en" {
                DEFAULT_LATE_PENALTIES_EN
            } else {
                DEFAULT_LATE_PENALTIES
            }
        } else {
            self.late_penalties.trim()
        }
    }
}

impl Store {
    pub fn issuer(&self) -> Result<Issuer> {
        let json: Option<String> = self
            .0
            .query_row("SELECT details FROM issuer_profile WHERE id=1", [], |r| {
                r.get(0)
            })
            .optional()?;
        Ok(json
            .map(|s| serde_json::from_str(&s))
            .transpose()?
            .unwrap_or_default())
    }

    pub fn save_issuer(&self, issuer: &Issuer) -> Result<()> {
        ensure!(!issuer.name.trim().is_empty(), "Issuer name is required");
        let tx = self.0.unchecked_transaction()?;
        let json = serde_json::to_string(issuer)?;
        let changed = tx.execute(
            "INSERT INTO issuer_profile(id,details) VALUES(1,?1)
            ON CONFLICT(id) DO UPDATE SET details=excluded.details WHERE details!=excluded.details",
            [json],
        )?;
        if changed > 0 {
            tx.execute(
                "UPDATE billing SET version=version+1 WHERE number IS NULL",
                [],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Logo as PNG bytes, already resized for documents.
    pub fn logo(&self) -> Result<Option<Vec<u8>>> {
        Ok(self
            .0
            .query_row("SELECT png FROM issuer_logo WHERE id=1", [], |r| r.get(0))
            .optional()?)
    }

    pub fn save_logo(&self, png: &[u8]) -> Result<()> {
        ensure!(png.starts_with(b"\x89PNG"), "Logo must be a PNG image");
        self.0.execute(
            "INSERT INTO issuer_logo(id,png) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET png=excluded.png",
            [png],
        )?;
        Ok(())
    }

    pub fn clear_logo(&self) -> Result<()> {
        self.0.execute("DELETE FROM issuer_logo", [])?;
        Ok(())
    }

    /// Local date `YYYY-MM-DD`.
    pub fn today(&self) -> Result<String> {
        Ok(self
            .0
            .query_row("SELECT date('now','localtime')", [], |r| r.get(0))?)
    }

    /// Local date `days` from today, `YYYY-MM-DD`.
    pub fn date_in_days(&self, days: i64) -> Result<String> {
        Ok(self.0.query_row(
            "SELECT date('now','localtime',?1)",
            [format!("{days:+} days")],
            |r| r.get(0),
        )?)
    }

    pub fn document_issuer(&self, id: i64) -> Result<Issuer> {
        let number: Option<String> =
            self.0
                .query_row("SELECT number FROM billing WHERE id=?1", [id], |r| r.get(0))?;
        if number.is_none() {
            return self.issuer();
        }
        let json: Option<String> = self
            .0
            .query_row(
                "SELECT details FROM issuer_snapshot WHERE document_id=?1",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        // Older simulations may have been frozen without an issuer.
        Ok(json
            .map(|s| serde_json::from_str(&s))
            .transpose()?
            .unwrap_or_default())
    }
}
