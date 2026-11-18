use super::Store;
use anyhow::{Context, Result, ensure};
use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use serde_json::Value;
use std::{collections::BTreeSet, fs, path::Path};

#[derive(Serialize)]
pub struct Report {
    pub records: usize,
    pub pdfs: usize,
    pub linked_pdfs: usize,
    pub inserted: usize,
    pub unchanged: usize,
    pub conflicts: usize,
    pub records_by_category: std::collections::BTreeMap<&'static str, usize>,
    pub unlinked_pdfs: usize,
    pub warnings: Vec<&'static str>,
}

struct Entry {
    category: &'static str,
    key: String,
    content: Vec<u8>,
    record_link: Option<String>,
}

pub struct Batch {
    entries: Vec<Entry>,
    report: Report,
}

impl Batch {
    pub fn load(root: &Path) -> Result<Self> {
        let mut entries = Vec::new();
        let mut report = Report {
            records: 0,
            pdfs: 0,
            linked_pdfs: 0,
            inserted: 0,
            unchanged: 0,
            conflicts: 0,
            records_by_category: Default::default(),
            unlinked_pdfs: 0,
            warnings: vec![
                "Source payment history can be incomplete; a missing payment does not imply an unpaid balance.",
                "Archive records preserve source data. Itemized lines and business relationships may require manual reconciliation.",
            ],
        };
        for (folder, key) in [
            ("clients", "clients"),
            ("societes", "companies"),
            ("devis", "quotations"),
            ("factures_acomptes", "invoices"),
        ] {
            let dir = root.join(folder);
            let files = list_files(&dir, "json")?;
            ensure!(
                files.len() == 1,
                "Expected exactly one JSON export in {folder}"
            );
            let data: Value = serde_json::from_reader(fs::File::open(&files[0])?)?;
            let records = data[key]
                .as_array()
                .with_context(|| format!("Missing {key} array"))?;
            report.records_by_category.insert(key, records.len());
            let mut ids = BTreeSet::new();
            for record in records {
                let id = record["id"]
                    .as_u64()
                    .context("Invalid source ID")?
                    .to_string();
                ensure!(ids.insert(id.clone()), "Duplicate source ID in {folder}");
                entries.push(Entry {
                    category: key,
                    key: id,
                    content: serde_json::to_vec(record)?,
                    record_link: None,
                });
                report.records += 1;
            }
            if matches!(key, "quotations" | "invoices") {
                for path in list_files(&dir.join("pdf_originaux"), "pdf")? {
                    let name = path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .context("Invalid PDF filename")?;
                    let stem = path
                        .file_stem()
                        .and_then(|name| name.to_str())
                        .context("Invalid PDF filename")?;
                    let mut matches = records
                        .iter()
                        .filter(|record| record["uid"].as_str() == Some(stem));
                    let link = matches
                        .next()
                        .filter(|_| matches.next().is_none())
                        .and_then(|r| r["id"].as_u64())
                        .map(|id| format!("{key}:{id}"));
                    let content = fs::read(&path)?;
                    ensure!(content.starts_with(b"%PDF-"), "Invalid PDF in {folder}");
                    report.pdfs += 1;
                    report.linked_pdfs += usize::from(link.is_some());
                    report.unlinked_pdfs += usize::from(link.is_none());
                    entries.push(Entry {
                        category: "pdf",
                        key: format!("{folder}/{name}"),
                        content,
                        record_link: link,
                    });
                }
            }
        }
        Ok(Self { entries, report })
    }

    pub fn preview(self) -> Report {
        self.report
    }

    pub fn apply(mut self, store: &mut Store, source: &str) -> Result<Report> {
        ensure!(!source.trim().is_empty(), "Source label must not be empty");
        let tx = store
            .0
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS facture_net_archive (
            source TEXT NOT NULL, category TEXT NOT NULL, source_key TEXT NOT NULL,
            content BLOB NOT NULL, record_link TEXT,
            PRIMARY KEY (source, category, source_key)
        );",
        )?;
        for entry in self.entries {
            let previous: Option<(Vec<u8>, Option<String>)> = tx.query_row(
                "SELECT content,record_link FROM facture_net_archive WHERE source=?1 AND category=?2 AND source_key=?3",
                params![source, entry.category, entry.key],
                |row| Ok((row.get(0)?, row.get(1)?)),
            ).optional()?;
            match previous {
                Some((content, link))
                    if content == entry.content
                        && (entry.record_link.is_none() || link == entry.record_link) =>
                {
                    self.report.unchanged += 1
                }
                Some(_) => self.report.conflicts += 1,
                None => {
                    tx.execute(
                        "INSERT INTO facture_net_archive VALUES (?1,?2,?3,?4,?5)",
                        params![
                            source,
                            entry.category,
                            entry.key,
                            entry.content,
                            entry.record_link
                        ],
                    )?;
                    self.report.inserted += 1;
                }
            }
        }
        tx.commit()?;
        Ok(self.report)
    }
}

fn list_files(dir: &Path, extension: &str) -> Result<Vec<std::path::PathBuf>> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("Cannot read {}", dir.display()))? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case(extension))
        {
            paths.push(entry.path());
        }
    }
    paths.sort();
    Ok(paths)
}

#[derive(Serialize)]
pub struct ArchiveItem {
    pub source: String,
    pub category: String,
    pub key: String,
    pub label: String,
    pub record_link: Option<String>,
}

impl Store {
    pub fn archive(&self) -> Result<Vec<ArchiveItem>> {
        let exists: bool = self.0.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='facture_net_archive')",
            [], |row| row.get(0),
        )?;
        if !exists {
            return Ok(Vec::new());
        }
        let mut query = self.0.prepare(
            "SELECT source, category, source_key, CASE WHEN category='pdf' THEN NULL ELSE content END, record_link
             FROM facture_net_archive ORDER BY source,category,source_key"
        )?;
        let mut rows = query.query([])?;
        let mut items = Vec::new();
        while let Some(row) = rows.next()? {
            let key: String = row.get(2)?;
            let content: Option<Vec<u8>> = row.get(3)?;
            let record = content
                .as_deref()
                .map(serde_json::from_slice::<Value>)
                .transpose()?;
            let label = record
                .as_ref()
                .and_then(|record| {
                    let name = ["firstName", "lastName"]
                        .iter()
                        .filter_map(|field| record[*field].as_str())
                        .collect::<Vec<_>>()
                        .join(" ");
                    record["uid"]
                        .as_str()
                        .or_else(|| record["name"].as_str())
                        .map(str::to_owned)
                        .or_else(|| (!name.is_empty()).then_some(name))
                })
                .unwrap_or_else(|| key.clone());
            items.push(ArchiveItem {
                source: row.get(0)?,
                category: row.get(1)?,
                key,
                label,
                record_link: row.get(4)?,
            });
        }
        Ok(items)
    }

    pub fn archive_record(&self, source: &str, category: &str, key: &str) -> Result<Value> {
        ensure!(
            matches!(
                category,
                "clients" | "companies" | "quotations" | "invoices"
            ),
            "Expected clients, companies, quotations or invoices"
        );
        let content: Vec<u8> = self.0.query_row(
            "SELECT content FROM facture_net_archive WHERE source=?1 AND category=?2 AND source_key=?3",
            params![source, category, key],
            |row| row.get(0),
        ).context("Archived record not found")?;
        Ok(serde_json::from_slice(&content)?)
    }

    pub fn archive_pdf(&self, source: &str, key: &str) -> Result<Vec<u8>> {
        self.0.query_row(
            "SELECT content FROM facture_net_archive WHERE source=?1 AND category='pdf' AND source_key=?2",
            params![source,key], |row| row.get(0),
        ).context("Archived PDF not found")
    }

    pub fn link_archive_pdf(&self, source: &str, pdf: &str, category: &str, id: u64) -> Result<()> {
        let folder = match category {
            "quotations" => "devis",
            "invoices" => "factures_acomptes",
            _ => anyhow::bail!("Expected quotations or invoices"),
        };
        ensure!(
            pdf.starts_with(&format!("{folder}/")),
            "PDF and document categories differ"
        );
        let changed = self.0.execute(
            "UPDATE facture_net_archive SET record_link=?1
             WHERE source=?2 AND category='pdf' AND source_key=?3
             AND (record_link IS NULL OR record_link=?1)
             AND EXISTS (SELECT 1 FROM facture_net_archive WHERE source=?2 AND category=?4 AND source_key=?5)",
            params![format!("{category}:{id}"),source,pdf,category,id.to_string()],
        )?;
        ensure!(
            changed == 1,
            "PDF or document missing, or the PDF already has a different link"
        );
        Ok(())
    }
}
