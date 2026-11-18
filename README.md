<div align="center">
  <img src="assets/facturecraft-logo.png" alt="FactureCraft squirrel icon" width="120">
  <h1>FactureCraft</h1>
  <p>Local invoicing for people and their tools.</p>
</div>

FactureCraft is a Rust desktop app for preparing quotes and invoices. The desktop interface, command line, and Model Context Protocol (MCP) server are designed to share the same business rules and local SQLite database. Human review in the desktop app precedes finalization.

## Project status

Sprint 1 provides a local draft workflow using the same database from the desktop app, command line, and MCP. A draft currently records a title, currency, and document language. Customers, line items, PDF generation, migration, and invoice issuance are later milestones.

| Interface | Sprint 1 |
| --- | --- |
| Native desktop app | View saved drafts and choose the interface language |
| `facturecraft` CLI | Create and inspect drafts |
| `facturecraft mcp` | List and read drafts through a local stdio server |

## Run from source

Install a current stable Rust toolchain, clone the repository, and run these commands from its root:

```sh
cargo run -- draft create "Example quote" --currency CHF --language en
cargo run -- gui --language fr
cargo run -- --help
```

The draft created by the first command appears in the desktop app. The interface language and document language can differ.

For MCP clients, build with `cargo build`. Set the server command to the absolute path of the resulting `facturecraft` executable in `target/debug` (use `facturecraft.exe` on Windows) and pass `mcp` as its sole argument. The server exposes `list_drafts` and `get_draft`.

## Roadmap

1. Establish the Cargo workspace, shared draft storage, native UI, CLI, and MCP transport.
2. Add customers, quote lines, tax, discounts, and PDF previews.
3. Import existing documents and original PDFs from Facture.net exports.
4. Add conversion from quote to invoice, deposits, payment tracking, and credit notes.
5. Add finalization, numbering, restoration checks, and distribution.

The planned first release targets macOS, Windows, and Linux, with French and English interfaces and document templates. Each document has its own currency and can use a document language independent of the interface.

## Credits

FactureCraft draws architectural inspiration from [ArtCraft](https://github.com/storytold/artcraft) and uses MCP components from [craft-libs](https://github.com/storytold/craft-libs). Facture.net is the source format for a planned migration workflow.

## Community

Read the [contribution guide](CONTRIBUTING.md) before opening an issue. Use the [security policy](SECURITY.md) for vulnerability reports.
