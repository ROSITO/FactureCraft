use crate::Store;
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OpenFlags};
use std::path::Path;

impl Store {
    /// Writes a consistent SQLite snapshot to an empty or absent path.
    pub fn snapshot(&self, path: &Path) -> Result<()> {
        snapshot(&self.0, path)
    }
}

impl Store {
    /// File backing this database, `None` for in-memory databases.
    pub fn path(&self) -> Option<std::path::PathBuf> {
        let file: String = self
            .0
            .query_row(
                "SELECT file FROM pragma_database_list WHERE name='main'",
                [],
                |row| row.get(0),
            )
            .ok()?;
        (!file.is_empty()).then(|| file.into())
    }
}

/// Verifies a FactureCraft snapshot and copies it without changing the source.
pub fn restore_snapshot(source: &Path, destination: &Path) -> Result<()> {
    let connection = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let status: String = connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    ensure!(status == "ok", "Snapshot failed SQLite integrity check");
    let tables: i64 = connection.query_row(
        "SELECT count(*) FROM sqlite_schema WHERE type='table' AND name IN
         ('drafts', 'quote_details', 'contacts', 'billing')",
        [],
        |row| row.get(0),
    )?;
    ensure!(tables == 4, "Snapshot is missing FactureCraft tables");
    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    ensure!(
        version <= crate::schema::VERSION,
        "Snapshot comes from a newer FactureCraft (schema {version}); update the application"
    );
    snapshot(&connection, destination)
}

fn snapshot(connection: &Connection, destination: &Path) -> Result<()> {
    let path = destination
        .to_str()
        .context("Snapshot path must be valid UTF-8")?;
    connection.execute("VACUUM INTO ?1", [path])?;
    Ok(())
}
