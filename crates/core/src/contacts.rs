use crate::Store;
use anyhow::{Result, ensure};
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct Contact {
    /// Local address book row; `None` for contacts read from the imported archive.
    #[serde(default)]
    pub id: Option<i64>,
    pub name: String,
    pub email: String,
    /// One line per address line.
    pub address: String,
    #[serde(default)]
    pub country: String,
    /// Company number (SIREN or equivalent).
    #[serde(default)]
    pub registration: String,
}

pub(crate) fn initialize(db: &rusqlite::Connection) -> Result<()> {
    for column in ["country", "registration"] {
        let exists: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('contacts') WHERE name=?1)",
            [column],
            |row| row.get(0),
        )?;
        if !exists {
            db.execute_batch(&format!(
                "ALTER TABLE contacts ADD COLUMN {column} TEXT NOT NULL DEFAULT ''"
            ))?;
        }
    }
    Ok(())
}

impl Store {
    pub fn contacts(&self) -> Result<Vec<Contact>> {
        let mut records = Vec::new();
        let mut query = self
            .0
            .prepare("SELECT rowid,name,email,address,country,registration FROM contacts ORDER BY name COLLATE NOCASE")?;
        for row in query.query_map([], |row| {
            Ok(Contact {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                email: row.get(2)?,
                address: row.get(3)?,
                country: row.get(4)?,
                registration: row.get(5)?,
            })
        })? {
            records.push(row?);
        }
        let imported: bool = self.0.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='facture_net_archive')",
            [], |row| row.get(0),
        )?;
        if imported {
            let mut query = self.0.prepare("SELECT content FROM facture_net_archive WHERE category IN ('clients','companies') ORDER BY source,category,source_key")?;
            let mut rows = query.query([])?;
            while let Some(row) = rows.next()? {
                let record: serde_json::Value = serde_json::from_slice(&row.get::<_, Vec<u8>>(0)?)?;
                let field = |key: &str| record[key].as_str().unwrap_or("").trim();
                let name = if field("name").is_empty() {
                    format!("{} {}", field("firstName"), field("lastName"))
                        .trim()
                        .to_owned()
                } else {
                    field("name").to_owned()
                };
                if name.is_empty() {
                    continue;
                }
                let city = format!("{} {}", field("zipCode"), field("city"));
                let mut address = vec![field("street")];
                if let Some(extra) = record["additional_address_lines"].as_array() {
                    address.extend(extra.iter().filter_map(serde_json::Value::as_str));
                }
                address.push(city.trim());
                records.push(Contact {
                    id: None,
                    name,
                    email: field("email").into(),
                    address: address
                        .into_iter()
                        .map(str::trim)
                        .filter(|part| !part.is_empty())
                        .collect::<Vec<_>>()
                        .join("\n"),
                    country: field("country").into(),
                    registration: field("company_number").into(),
                });
            }
        }
        records.sort_by_key(|record| record.name.to_lowercase());
        // A local entry wins over the archived one it duplicates.
        records.sort_by_key(|record| (record.name.to_lowercase(), record.id.is_none()));
        records.dedup_by(|a, b| a.name == b.name && a.email == b.email && a.address == b.address);
        Ok(records)
    }

    pub fn save_contact(&self, contact: &Contact) -> Result<()> {
        let name = contact.name.trim();
        ensure!(
            !name.is_empty() && name.chars().count() <= 200,
            "Contact name must contain 1–200 characters"
        );
        let values = params![
            name,
            contact.email.trim(),
            contact.address.trim(),
            contact.country.trim(),
            contact.registration.trim(),
            contact.id
        ];
        if contact.id.is_some() {
            ensure!(
                self.0.execute(
                    "UPDATE contacts SET name=?1,email=?2,address=?3,country=?4,registration=?5 WHERE rowid=?6",
                    values,
                )? == 1,
                "Contact not found"
            );
        } else {
            self.0.execute(
                "INSERT INTO contacts(name,email,address,country,registration) VALUES (?1,?2,?3,?4,?5)
                ON CONFLICT(name,email,address) DO UPDATE SET country=excluded.country, registration=excluded.registration",
                &values[..5],
            )?;
        }
        Ok(())
    }

    /// Removes an address book entry; documents keep their own copy of the recipient.
    pub fn delete_contact(&self, id: i64) -> Result<()> {
        ensure!(
            self.0
                .execute("DELETE FROM contacts WHERE rowid=?1", [id])?
                == 1,
            "Contact not found"
        );
        Ok(())
    }
}
