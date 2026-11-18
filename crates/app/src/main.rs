mod mcp;
mod ui;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use facturecraft_core::{Store, default_db};
use std::path::PathBuf;

#[derive(Clone, Copy, ValueEnum)]
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
}

fn main() -> Result<()> {
    let args = Args::parse();
    let path = match args.db {
        Some(path) => path,
        None => default_db()?,
    };
    let store = Store::open(&path).context("Opening FactureCraft storage")?;
    match args.command.unwrap_or(Command::Gui {
        language: Language::Fr,
    }) {
        Command::Gui { language } => ui::run(store, language),
        Command::Mcp => mcp::serve(&store),
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
            };
            println!("{}", serde_json::to_string_pretty(&data)?);
            Ok(())
        }
    }
}
