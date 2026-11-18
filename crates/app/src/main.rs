mod brand;
mod dashboard;
mod mcp;
mod operations;
mod pdf;
mod subset;
mod ui;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use facturecraft_core::{Store, default_db};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, ValueEnum)]
enum Language {
    Fr,
    En,
}

impl Language {
    fn code(self) -> &'static str {
        match self {
            Self::Fr => "fr",
            Self::En => "en",
        }
    }
}

#[derive(Parser)]
#[command(version, about = "FactureCraft · Local invoicing")]
struct Args {
    #[arg(long, global = true, env = "FACTURECRAFT_DB")]
    db: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    Gui {
        #[arg(long, value_enum, default_value = "fr")]
        language: Language,
    },
    Draft {
        #[command(subcommand)]
        action: DraftAction,
    },
    Mcp,
    Contacts,
    Backup {
        output: PathBuf,
    },
    Restore {
        snapshot: PathBuf,
    },
    Archive {
        #[command(subcommand)]
        action: ArchiveAction,
    },
    Import {
        folder: PathBuf,
        #[arg(long, requires = "source")]
        apply: bool,
        #[arg(long)]
        source: Option<String>,
    },
}

#[derive(Subcommand)]
enum ArchiveAction {
    List,
    Get {
        source: String,
        category: String,
        key: String,
    },
    ExportPdf {
        source: String,
        key: String,
        output: PathBuf,
    },
    LinkPdf {
        source: String,
        key: String,
        category: String,
        id: u64,
    },
}

#[derive(Subcommand)]
enum DraftAction {
    Create {
        title: String,
        #[arg(long, default_value = "EUR")]
        currency: String,
        #[arg(long, value_enum, default_value = "fr")]
        language: Language,
    },
    List,
    Get {
        id: i64,
    },
    Details {
        id: i64,
    },
    Prepare {
        title: String,
        quote: PathBuf,
        #[arg(long, default_value = "EUR")]
        currency: String,
        #[arg(long, value_enum, default_value = "fr")]
        language: Language,
        #[arg(long)]
        key: String,
    },
    Update {
        id: i64,
        expected_version: i64,
        title: String,
        quote: PathBuf,
        #[arg(long)]
        currency: String,
        #[arg(long, value_enum)]
        language: Language,
    },
    Derive {
        id: i64,
        expected_version: i64,
        kind: String,
        title: String,
        #[arg(long)]
        key: String,
    },
    Preview {
        id: i64,
        output: PathBuf,
    },
}

fn main() -> Result<()> {
    let args = Args::parse();
    if let Some(Command::Import {
        folder,
        apply,
        source,
    }) = &args.command
    {
        let batch = facturecraft_core::import::Batch::load(folder)?;
        let report = if *apply {
            let path = args.db.clone().map_or_else(default_db, Ok)?;
            batch.apply(
                &mut Store::open(&path)?,
                source.as_deref().context("Missing source label")?,
            )?
        } else {
            batch.preview()
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    let path = match args.db {
        Some(path) => path,
        None => default_db()?,
    };
    if let Some(Command::Restore { snapshot }) = &args.command {
        for suffix in ["-journal", "-wal", "-shm"] {
            let mut sidecar = path.as_os_str().to_owned();
            sidecar.push(suffix);
            anyhow::ensure!(
                !PathBuf::from(sidecar).try_exists()?,
                "Restore destination has SQLite companion files"
            );
        }
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }
        return operations::publish_file(&path, |file| {
            facturecraft_core::backup::restore_snapshot(snapshot, file.path())
        });
    }
    let store = Store::open(&path).context("Opening FactureCraft storage")?;
    match args.command.unwrap_or(Command::Gui {
        language: Language::Fr,
    }) {
        Command::Gui { language } => ui::run(store, language),
        Command::Import { .. } => unreachable!("Handled before opening the database"),
        Command::Mcp => mcp::serve(&store),
        Command::Backup { output } => {
            operations::publish_file(&output, |file| store.snapshot(file.path()))
        }
        Command::Restore { .. } => unreachable!("Handled before opening the database"),
        Command::Contacts => {
            println!("{}", serde_json::to_string_pretty(&store.contacts()?)?);
            Ok(())
        }
        Command::Archive { action } => match action {
            ArchiveAction::List => {
                println!("{}", serde_json::to_string_pretty(&store.archive()?)?);
                Ok(())
            }
            ArchiveAction::Get {
                source,
                category,
                key,
            } => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&store.archive_record(&source, &category, &key)?)?
                );
                Ok(())
            }
            ArchiveAction::ExportPdf {
                source,
                key,
                output,
            } => operations::write_file(&output, &store.archive_pdf(&source, &key)?),
            ArchiveAction::LinkPdf {
                source,
                key,
                category,
                id,
            } => store.link_archive_pdf(&source, &key, &category, id),
        },
        Command::Draft { action } => {
            let data = match action {
                DraftAction::Create {
                    title,
                    currency,
                    language,
                } => serde_json::to_value(store.create(&title, &currency, language.code())?)?,
                DraftAction::List => serde_json::to_value(store.list()?)?,
                DraftAction::Get { id } => {
                    serde_json::to_value(store.get(id)?.context("Draft not found")?)?
                }
                DraftAction::Details { id } => operations::document(&store, id)?,
                DraftAction::Prepare {
                    title,
                    quote,
                    currency,
                    language,
                    key,
                } => {
                    let draft = store.create_quote_idempotent(
                        &key,
                        &title,
                        &currency,
                        language.code(),
                        &operations::read_quote(&quote)?,
                    )?;
                    operations::document(&store, draft.id)?
                }
                DraftAction::Update {
                    id,
                    expected_version,
                    title,
                    quote,
                    currency,
                    language,
                } => {
                    store.save_quote_versioned(
                        id,
                        expected_version,
                        &title,
                        &currency,
                        language.code(),
                        &operations::read_quote(&quote)?,
                    )?;
                    operations::document(&store, id)?
                }
                DraftAction::Derive {
                    id,
                    expected_version,
                    kind,
                    title,
                    key,
                } => {
                    let draft = store.derive_document_idempotent(
                        id,
                        expected_version,
                        &kind,
                        &title,
                        &key,
                    )?;
                    operations::document(&store, draft.id)?
                }
                DraftAction::Preview { id, output } => {
                    return operations::write_file(&output, &operations::render_draft(&store, id)?);
                }
            };
            println!("{}", serde_json::to_string_pretty(&data)?);
            Ok(())
        }
    }
}
