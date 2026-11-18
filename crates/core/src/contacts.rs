use crate::Store;
use anyhow::{Result, ensure};
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct Contact {
    pub name: String,
    pub email: String,
    pub address: String,
}

impl Store {
    pub fn contacts(&self) -> Result<Vec<Contact>> {
        let mut records = Vec::new();
        let mut query = self
            .0
            .prepare("SELECT name,email,address FROM contacts ORDER BY name COLLATE NOCASE")?;
        for row in query.query_map([], |row| {
            Ok(Contact {
                name: row.get(0)?,
                email: row.get(1)?,
                address: row.get(2)?,
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
                address.extend([city.trim(), field("country")]);
                records.push(Contact {
                    name,
                    email: field("email").into(),
                    address: address
                        .into_iter()
                        .filter(|part| !part.trim().is_empty())
                        .collect::<Vec<_>>()
                        .join(", "),
                });
            }
        }
        records.sort_by_key(|record| record.name.to_lowercase());
        records.dedup_by(|a, b| a.name == b.name && a.email == b.email && a.address == b.address);
        Ok(records)
    }

    pub fn save_contact(&self, contact: &Contact) -> Result<()> {
        let name = contact.name.trim();
        ensure!(
            !name.is_empty() && name.chars().count() <= 200,
            "Contact name must contain 1–200 characters"
        );
        self.0.execute(
            "INSERT INTO contacts(name,email,address) VALUES (?1,?2,?3) ON CONFLICT(name,email,address) DO NOTHING",
            params![name, contact.email.trim(), contact.address.trim()],
        )?;
        Ok(())
    }
}
