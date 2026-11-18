use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use craft_jsonrpc::{Limits, Request, RpcError, error_response};
use craft_mcp::{Policy, Session, State, text_tool_result, tool_call, tool_list};
use facturecraft_core::Store;
use facturecraft_core::quote::Quote;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    io::{self, BufRead, Read, Write},
};

pub fn serve(store: &Store) -> Result<()> {
    let session = RefCell::new(Session::new(
        "facturecraft",
        env!("CARGO_PKG_VERSION"),
        Policy::Strict,
    ));
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    let mut line = Vec::new();
    const MAX_FRAME: u64 = 1024 * 1024;
    loop {
        line.clear();
        if input
            .by_ref()
            .take(MAX_FRAME + 1)
            .read_until(b'\n', &mut line)?
            == 0
        {
            return Ok(());
        }
        if line.len() as u64 > MAX_FRAME {
            anyhow::bail!("MCP input exceeds 1 MiB");
        }
        let response = match serde_json::from_slice::<Value>(&line) {
            Ok(value) if !value.is_array() => craft_jsonrpc::process(
                &value,
                Limits::default(),
                |request| {
                    let mut session = session.borrow_mut();
                    if let Some(result) = session.request(request, false) {
                        return result;
                    }
                    if session.state() != State::Ready {
                        return Err(RpcError::new(-32600, "Server not ready"));
                    }
                    route(store, request)
                },
                |request| session.borrow_mut().notification(request),
            )
            .map_err(|error| anyhow::anyhow!("{error:?}"))?,
            Ok(_) => Some(error_response(
                Value::Null,
                RpcError::new(-32600, "Batch unsupported"),
            )),
            Err(_) => Some(error_response(
                Value::Null,
                RpcError::new(-32700, "Parse error"),
            )),
        };
        if let Some(response) = response {
            serde_json::to_writer(&mut output, &response)?;
            output.write_all(b"\n")?;
            output.flush()?;
        }
    }
}

fn route(store: &Store, request: Request<'_>) -> Result<Value, RpcError> {
    match request.method {
        "tools/list" => Ok(tool_list(tools())),
        "tools/call" => {
            let (name, args) = tool_call(request.params.unwrap_or(&Value::Null))?;
            call(store, name, &args)
        }
        _ => Err(RpcError::new(-32601, "Method not found")),
    }
}

fn object(properties: Value, required: &[&str]) -> Value {
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}

fn tools() -> Value {
    let string = json!({"type":"string"});
    let id = json!({"type":"integer","minimum":1});
    let language = json!({"enum":["fr","en"]});
    let line = object(
        json!({"description":string,"quantity":string,"price":string,"discount":string,"vat":string,"unit":string,"exemption_reason":string}),
        &["description", "quantity", "price", "discount", "vat"],
    );
    let quote = object(
        json!({"recipient":string,"recipient_email":string,"recipient_address":string,"recipient_country":string,"recipient_registration":string,"payment_terms":string,"payment_method":string,"cgv":string,"valid_until":string,"terms":string,"global_discount":string,"lines":{"type":"array","items":line}}),
        &["recipient", "lines"],
    );
    let draft_id = object(json!({"id":id}), &["id"]);
    json!([
        tool(
            "list_drafts",
            "List local draft metadata",
            object(json!({}), &[]),
            true
        ),
        tool(
            "get_draft",
            "Read local draft metadata",
            draft_id.clone(),
            true
        ),
        tool(
            "get_document",
            "Read a document draft, its version, and totals",
            draft_id.clone(),
            true
        ),
        tool(
            "list_products",
            "List reusable products and services; copy their values into quote lines",
            object(json!({}), &[]),
            true
        ),
        tool(
            "search_parties",
            "Find local contacts by name",
            object(json!({"query":string}), &["query"]),
            true
        ),
        tool(
            "create_draft",
            "Prepare a quote draft",
            object(
                json!({"title":string,"currency":string,"language":language,"quote":quote,"idempotency_key":string}),
                &["title", "currency", "language", "quote", "idempotency_key"],
            ),
            false
        ),
        tool(
            "update_draft",
            "Update draft details at the specified version",
            object(
                json!({"id":id,"expected_version":id,"title":string,"currency":string,"language":language,"quote":quote}),
                &[
                    "id",
                    "expected_version",
                    "title",
                    "currency",
                    "language",
                    "quote"
                ],
            ),
            false
        ),
        tool(
            "create_invoice_draft_from_quote",
            "Prepare an invoice for an accepted quote",
            object(
                json!({"id":id,"expected_version":id,"title":string,"idempotency_key":string}),
                &["id", "expected_version", "title", "idempotency_key"],
            ),
            false
        ),
        tool(
            "preview_draft",
            "Read an embedded draft PDF",
            draft_id,
            true
        ),
    ])
}

fn tool(name: &str, description: &str, schema: Value, read_only: bool) -> Value {
    json!({"name":name,"description":description,"inputSchema":schema,
           "annotations":{"readOnlyHint":read_only,"destructiveHint":false,"openWorldHint":false}})
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DraftId {
    id: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Search {
    query: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Prepare {
    title: String,
    currency: String,
    language: String,
    quote: Quote,
    idempotency_key: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Update {
    id: i64,
    expected_version: i64,
    title: String,
    currency: String,
    language: String,
    quote: Quote,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeriveInvoice {
    id: i64,
    expected_version: i64,
    title: String,
    idempotency_key: String,
}

fn arguments<T: DeserializeOwned>(value: &Value) -> Result<T, RpcError> {
    serde_json::from_value(value.clone())
        .map_err(|_| RpcError::new(-32602, "Invalid tool arguments"))
}

fn quote_arguments<T: DeserializeOwned>(value: &Value) -> Result<T, RpcError> {
    let known_fields = |object: &Value, fields: &[&str]| {
        object
            .as_object()
            .is_some_and(|entries| entries.keys().all(|key| fields.contains(&key.as_str())))
    };
    let quote = &value["quote"];
    if !known_fields(
        quote,
        &[
            "recipient",
            "recipient_email",
            "recipient_address",
            "valid_until",
            "terms",
            "global_discount",
            "lines",
        ],
    ) || !quote["lines"].as_array().is_some_and(|lines| {
        lines.iter().all(|line| {
            known_fields(
                line,
                &[
                    "description",
                    "quantity",
                    "price",
                    "discount",
                    "vat",
                    "unit",
                    "exemption_reason",
                ],
            )
        })
    }) {
        return Err(RpcError::new(-32602, "Invalid quote fields"));
    }
    arguments(value)
}

fn positive(value: i64) -> Result<i64, RpcError> {
    if value > 0 {
        Ok(value)
    } else {
        Err(RpcError::new(-32602, "Expected a positive ID or version"))
    }
}

fn call(store: &Store, name: &str, args: &Value) -> Result<Value, RpcError> {
    use crate::operations;
    let result: Result<Value> = match name {
        "list_drafts" => {
            arguments::<Empty>(args)?;
            store
                .list()
                .and_then(|drafts| Ok(serde_json::to_value(drafts)?))
        }
        "get_draft" => {
            let id = positive(arguments::<DraftId>(args)?.id)?;
            store
                .get(id)
                .and_then(|draft| Ok(serde_json::to_value(draft.context("Draft not found")?)?))
        }
        "get_document" => operations::document(store, positive(arguments::<DraftId>(args)?.id)?),
        "list_products" => {
            arguments::<Empty>(args)?;
            store
                .products()
                .and_then(|products| Ok(serde_json::to_value(products)?))
        }
        "search_parties" => {
            let query = arguments::<Search>(args)?.query.to_lowercase();
            store.contacts().map(|contacts| {
                json!(
                    contacts
                        .into_iter()
                        .filter(|contact| contact.name.to_lowercase().contains(query.trim()))
                        .take(50)
                        .collect::<Vec<_>>()
                )
            })
        }
        "create_draft" => {
            let input: Prepare = quote_arguments(args)?;
            store
                .create_quote_idempotent(
                    &input.idempotency_key,
                    &input.title,
                    &input.currency,
                    &input.language,
                    &input.quote,
                )
                .and_then(|draft| operations::document(store, draft.id))
        }
        "update_draft" => {
            let input: Update = quote_arguments(args)?;
            let id = positive(input.id)?;
            let version = positive(input.expected_version)?;
            store
                .save_quote_versioned(
                    id,
                    version,
                    &input.title,
                    &input.currency,
                    &input.language,
                    &input.quote,
                )
                .and_then(|draft| operations::document(store, draft.id))
        }
        "create_invoice_draft_from_quote" => {
            let input = arguments::<DeriveInvoice>(args)?;
            let id = positive(input.id)?;
            let version = positive(input.expected_version)?;
            store
                .derive_document_idempotent(
                    id,
                    version,
                    "invoice",
                    &input.title,
                    &input.idempotency_key,
                )
                .and_then(|draft| operations::document(store, draft.id))
        }
        "preview_draft" => {
            let id = positive(arguments::<DraftId>(args)?.id)?;
            return Ok(match operations::render_draft(store, id) {
                Ok(pdf) => json!({"content":[{"type":"resource","resource":{
                    "uri":format!("facturecraft://draft/{id}/preview.pdf"),
                    "mimeType":"application/pdf",
                    "blob":STANDARD.encode(pdf)
                }}],"isError":false}),
                Err(error) => text_tool_result(&error.to_string(), true),
            });
        }
        _ => return Err(RpcError::new(-32602, "Invalid tool name")),
    };
    Ok(match result {
        Ok(data) => text_tool_result(&data.to_string(), false),
        Err(error) => text_tool_result(&error.to_string(), true),
    })
}
