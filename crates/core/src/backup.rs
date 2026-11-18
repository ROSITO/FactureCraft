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

/// Verifies a FactureCraft snapshot and copies it without changing the source.
pub fn restore_snapshot(source: &Path, destination: &Path) -> Result<()> {
    let connection = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let status: String = connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    ensure!(status == "ok", "Snapshot failed SQLite integrity check");
    let tables: i64 = connection.query_row(
        "SELECT count(*) FROM sqlite_schema WHERE type='table' AND name IN
         ('drafts', 'quote_details', 'contacts')",
        [],
        |row| row.get(0),
    )?;
    ensure!(tables == 3, "Snapshot is missing FactureCraft tables");
    snapshot(&connection, destination)
}

fn snapshot(connection: &Connection, destination: &Path) -> Result<()> {
    let path = destination
        .to_str()
        .context("Snapshot path must be valid UTF-8")?;
    connection.execute("VACUUM INTO ?1", [path])?;
    Ok(())
}
