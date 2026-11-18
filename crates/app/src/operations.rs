use anyhow::Result;
use facturecraft_core::{Store, billing::DocumentSnapshot, quote::Quote};
use serde_json::{Value, json};
use std::{io::Write, path::Path};

pub fn document(store: &Store, id: i64) -> Result<Value> {
    let DocumentSnapshot {
        draft,
        billing,
        quote,
        ..
    } = store.document_snapshot(id, false)?;
    let (net, vat, gross) = quote.totals(&draft.currency)?;
    Ok(json!({
        "draft": draft,
        "billing": billing,
        "quote": quote,
        "totals": {"net": net, "vat": vat, "gross": gross},
    }))
}

pub fn render_draft(store: &Store, id: i64) -> Result<Vec<u8>> {
    let DocumentSnapshot {
        draft,
        billing,
        quote,
        finalized_pdf,
    } = store.document_snapshot(id, true)?;
    if let Some(pdf) = finalized_pdf {
        return Ok(pdf);
    }
    let issuer = store.document_issuer(id)?;
    let logo = store.logo()?;
    crate::pdf::render(&crate::pdf::Document {
        kind: &billing.kind,
        number: None,
        date: &store.today()?,
        title: &draft.title,
        currency: &draft.currency,
        language: &draft.language,
        quote: &quote,
        issuer: &issuer,
        logo: logo.as_deref(),
        applied_deposit: (billing.kind == "invoice")
            .then_some((&billing.deposits, &billing.outstanding)),
    })
}

pub fn read_quote(path: &Path) -> Result<Quote> {
    Ok(serde_json::from_reader(std::fs::File::open(path)?)?)
}

pub fn publish_file(
    path: &Path,
    write: impl FnOnce(&mut tempfile::NamedTempFile) -> Result<()>,
) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    write(&mut temporary)?;
    temporary.as_file().sync_all()?;
    temporary.persist_noclobber(path)?;
    Ok(())
}

pub fn write_file(path: &Path, contents: &[u8]) -> Result<()> {
    publish_file(path, |file| Ok(file.write_all(contents)?))
}

/// Writes `bytes` to a fresh temporary file and opens it with the system viewer.
pub fn open_pdf(bytes: &[u8]) -> Result<()> {
    let mut file = tempfile::Builder::new()
        .prefix("facturecraft-")
        .suffix(".pdf")
        .tempfile()?;
    file.write_all(bytes)?;
    let (_, path) = file.keep()?;
    let mut command = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else if cfg!(target_os = "windows") {
        let mut command = std::process::Command::new("cmd");
        command.args(["/C", "start", ""]);
        command
    } else {
        std::process::Command::new("xdg-open")
    };
    command.arg(&path).spawn()?;
    Ok(())
}
