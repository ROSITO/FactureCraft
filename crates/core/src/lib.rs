use anyhow::{Context, Result, ensure};
use directories::ProjectDirs;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, Serialize)]
pub struct Draft {
    pub id: i64,
    pub title: String,
    pub currency: String,
    pub language: String,
}

pub fn default_db() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("", "", "FactureCraft")
        .context("Cannot locate the application data directory")?;
    Ok(dirs.data_local_dir().join("facturecraft.sqlite3"))
}

pub struct Store(Connection);

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        schema::migrate(&conn)?;
        Ok(Self(conn))
    }

    pub fn create(&self, title: &str, currency: &str, language: &str) -> Result<Draft> {
        let title = title.trim();
        let currency = currency.trim().to_ascii_uppercase();
        ensure!(
            !title.is_empty() && title.chars().count() <= 200,
            "Title must contain 1–200 characters"
        );
        ensure!(
            currency.parse::<iso_currency::Currency>().is_ok(),
            "Unknown ISO 4217 currency"
        );
        ensure!(
            matches!(language, "fr" | "en"),
            "Supported document languages: fr, en"
        );
        self.0.execute(
            "INSERT INTO drafts(title, currency, language) VALUES (?1, ?2, ?3)",
            params![title, currency, language],
        )?;
        Ok(Draft {
            id: self.0.last_insert_rowid(),
            title: title.into(),
            currency,
            language: language.into(),
        })
    }

    pub fn get(&self, id: i64) -> Result<Option<Draft>> {
        Ok(self
            .0
            .query_row(
                "SELECT id,title,currency,language FROM drafts WHERE id=?1",
                [id],
                read_draft,
            )
            .optional()?)
    }

    /// Interface preference stored with the data, e.g. `ui.theme`.
    pub fn preference(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .0
            .query_row("SELECT value FROM settings WHERE key=?1", [key], |row| {
                row.get(0)
            })
            .optional()?)
    }

    pub fn set_preference(&self, key: &str, value: &str) -> Result<()> {
        ensure!(
            key.starts_with("ui."),
            "Only interface preferences can be set"
        );
        self.0.execute(
            "INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<Draft>> {
        let mut query = self
            .0
            .prepare("SELECT id,title,currency,language FROM drafts ORDER BY id DESC")?;
        Ok(query
            .query_map([], read_draft)?
            .collect::<rusqlite::Result<_>>()?)
    }
}

fn read_draft(row: &rusqlite::Row<'_>) -> rusqlite::Result<Draft> {
    Ok(Draft {
        id: row.get(0)?,
        title: row.get(1)?,
        currency: row.get(2)?,
        language: row.get(3)?,
    })
}

pub mod schema;

pub mod export;

pub mod import;

pub mod quote;

pub mod contacts;

pub mod billing;
mod idempotency;

pub mod backup;

pub mod issuer;

pub mod products;

pub mod documents;

pub mod history;

pub mod dashboard;

pub mod facturx;

pub mod reconciliation;
