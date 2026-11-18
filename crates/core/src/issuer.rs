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
    pub terms: String,
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
