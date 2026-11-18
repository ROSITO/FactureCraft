<div align="center">
  <img src="assets/facturecraft-logo.png" alt="FactureCraft squirrel icon" width="120">
  <h1>FactureCraft</h1>
  <p>Local invoicing for people and their tools.</p>
</div>

FactureCraft is a Rust desktop app for preparing quotes and invoices. The desktop interface, command line, and Model Context Protocol (MCP) server are designed to share the same business rules and local SQLite database. Human review in the desktop app precedes finalization.

## Project status

FactureCraft is in early development. This repository starts with project documentation. Releases, invoice issuance, and regulatory integrations are planned separately from the initial draft workflow.

| Interface | Planned use |
| --- | --- |
| Native desktop app | View customers and documents, edit drafts, preview PDFs, and review before finalization |
| `facturecraft` CLI | Inspect and prepare drafts, import archives, and manage backups |
| `facturecraft mcp` | Let MCP clients read and prepare drafts through a local stdio server |

The initial milestone is to create a draft from the CLI, open that same draft in the desktop interface, and read it through MCP.

## Roadmap

1. Establish the Cargo workspace, shared draft storage, native UI, CLI, and MCP transport.
2. Add customers, quote lines, tax, discounts, and PDF previews.
3. Import existing documents and original PDFs from Facture.net exports.
4. Add conversion from quote to invoice, deposits, payment tracking, and credit notes.
5. Add finalization, numbering, restoration checks, and distribution.

The planned first release targets macOS, Windows, and Linux, with French and English interfaces and document templates. Each document has its own currency and can use a document language independent of the interface.

## Credits

FactureCraft draws architectural inspiration from [ArtCraft](https://github.com/storytold/artcraft) and plans to reuse MCP components from [craft-libs](https://github.com/storytold/craft-libs). Facture.net is the source format for a planned migration workflow.

## Community

Read the [contribution guide](CONTRIBUTING.md) before opening an issue. Use the [security policy](SECURITY.md) for vulnerability reports.
