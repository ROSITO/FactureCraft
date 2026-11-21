//! Ordered schema migrations driven by `PRAGMA user_version`.
use anyhow::{Result, bail};
use rusqlite::Connection;

type Migration = fn(&Connection) -> Result<()>;

/// Append only. Entry `n` upgrades a database from `user_version` n to n + 1.
const MIGRATIONS: &[Migration] = &[v1, v2];

/// Highest schema version this build understands.
pub const VERSION: i64 = MIGRATIONS.len() as i64;

pub(crate) fn migrate(connection: &Connection) -> Result<()> {
    let current: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if current > VERSION {
        bail!(
            "This database uses schema version {current}, newer than version {VERSION} known by this FactureCraft. Update the application; the file was not modified."
        );
    }
    // Not changeable inside a transaction, so set it first.
    connection.execute_batch("PRAGMA foreign_keys=ON;")?;
    let tx = connection.unchecked_transaction()?;
    for (index, migration) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        migration(&tx)?;
        tx.execute_batch(&format!("PRAGMA user_version={}", index + 1))?;
    }
    tx.commit()?;
    Ok(())
}

fn add_column(db: &Connection, table: &str, column: &str, declaration: &str) -> Result<()> {
    let exists: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info(?1) WHERE name=?2)",
        [table, column],
        |row| row.get(0),
    )?;
    if !exists {
        db.execute_batch(&format!(
            "ALTER TABLE {table} ADD COLUMN {column} {declaration}"
        ))?;
    }
    Ok(())
}

/// Brings a fresh database, or one written by any earlier release, to the first versioned schema.
fn v1(db: &Connection) -> Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS drafts (
            id INTEGER PRIMARY KEY, title TEXT NOT NULL, currency TEXT NOT NULL, language TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS quote_details (draft_id INTEGER PRIMARY KEY, details TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS contacts (name TEXT NOT NULL, email TEXT NOT NULL, address TEXT NOT NULL, UNIQUE(name,email,address));
        CREATE TABLE IF NOT EXISTS billing (
            id INTEGER PRIMARY KEY REFERENCES drafts(id), kind TEXT NOT NULL DEFAULT 'quote',
            version INTEGER NOT NULL DEFAULT 1, source_id INTEGER REFERENCES drafts(id),
            accepted_date TEXT, number TEXT UNIQUE, pdf BLOB, issued_at TEXT);
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
        CREATE TABLE IF NOT EXISTS issuer_logo(id INTEGER PRIMARY KEY CHECK(id=1), png BLOB NOT NULL);
        CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY, value TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS products (id INTEGER PRIMARY KEY, name TEXT NOT NULL,
            description TEXT NOT NULL, unit TEXT NOT NULL, price TEXT NOT NULL, vat TEXT NOT NULL,
            exemption_reason TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS facture_net_archive (
            source TEXT NOT NULL, category TEXT NOT NULL, source_key TEXT NOT NULL,
            content BLOB NOT NULL, record_link TEXT,
            PRIMARY KEY (source, category, source_key));
        CREATE TABLE IF NOT EXISTS draft_requests (
            request_key TEXT PRIMARY KEY, request TEXT NOT NULL,
            draft_id INTEGER NOT NULL REFERENCES drafts(id));
        INSERT OR IGNORE INTO billing(id) SELECT id FROM drafts;",
    )?;
    // Columns added by later releases.
    add_column(db, "billing", "closed", "TEXT")?;
    add_column(db, "billing", "closed_date", "TEXT")?;
    add_column(db, "contacts", "country", "TEXT NOT NULL DEFAULT ''")?;
    add_column(db, "contacts", "registration", "TEXT NOT NULL DEFAULT ''")?;
    Ok(())
}

/// CRM fields on the local address book: phone, private notes, tags, follow-up date.
fn v2(db: &Connection) -> Result<()> {
    add_column(db, "contacts", "phone", "TEXT NOT NULL DEFAULT ''")?;
    add_column(db, "contacts", "notes", "TEXT NOT NULL DEFAULT ''")?;
    add_column(db, "contacts", "tags", "TEXT NOT NULL DEFAULT ''")?;
    add_column(db, "contacts", "follow_up", "TEXT NOT NULL DEFAULT ''")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Store;
    use std::path::PathBuf;

    fn temp_db(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("fc-{name}-{}.sqlite3", std::process::id()));
        let _ = std::fs::remove_file(&path);
        path
    }

    /// Schema as of the first release: no closed columns, no country/registration, no lazy tables.
    fn old_database(path: &PathBuf) {
        let db = Connection::open(path).unwrap();
        db.execute_batch(
            "CREATE TABLE drafts (id INTEGER PRIMARY KEY, title TEXT NOT NULL, currency TEXT NOT NULL, language TEXT NOT NULL);
            CREATE TABLE quote_details (draft_id INTEGER PRIMARY KEY, details TEXT NOT NULL);
            CREATE TABLE contacts (name TEXT NOT NULL, email TEXT NOT NULL, address TEXT NOT NULL, UNIQUE(name,email,address));
            CREATE TABLE billing (id INTEGER PRIMARY KEY REFERENCES drafts(id), kind TEXT NOT NULL DEFAULT 'quote',
              version INTEGER NOT NULL DEFAULT 1, source_id INTEGER REFERENCES drafts(id),
              accepted_date TEXT, number TEXT UNIQUE, pdf BLOB, issued_at TEXT);
            INSERT INTO drafts VALUES (1,'Old quote','EUR','fr');
            INSERT INTO billing(id) VALUES (1);
            INSERT INTO quote_details VALUES (1,'{\"recipient\":\"X\",\"lines\":[]}');
            INSERT INTO contacts VALUES ('Alice','a@example.test','1 rue Test');",
        )
        .unwrap();
    }

    #[test]
    fn old_database_upgrades_with_data_intact_and_reopen_is_a_no_op() {
        let path = temp_db("old");
        old_database(&path);
        let store = Store::open(&path).unwrap();
        assert_eq!(store.get(1).unwrap().unwrap().title, "Old quote");
        let contacts = store.contacts().unwrap();
        assert_eq!(contacts[0].name, "Alice");
        assert_eq!(contacts[0].country, "");
        assert!(store.billing_state(1).unwrap().closed.is_none());
        drop(store);
        let db = Connection::open(&path).unwrap();
        let version: i64 = db
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, VERSION);
        for table in ["facture_net_archive", "draft_requests", "settings"] {
            let n: i64 = db
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE name=?1",
                    [table],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "{table}");
        }
        drop(db);
        Store::open(&path).unwrap();
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn newer_schema_is_refused_untouched() {
        let path = temp_db("newer");
        Store::open(&path).unwrap();
        Connection::open(&path)
            .unwrap()
            .execute_batch(&format!("PRAGMA user_version={}", VERSION + 1))
            .unwrap();
        let error = Store::open(&path).err().unwrap().to_string();
        assert!(error.contains("newer"), "{error}");
        let _ = std::fs::remove_file(&path);
    }
}
