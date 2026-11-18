use anyhow::Result;
use craft_jsonrpc::{Limits, Request, RpcError, error_response};
use craft_mcp::{Policy, Session, State, text_tool_result, tool_call, tool_list};
use facturecraft_core::Store;
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
        "tools/list" => Ok(tool_list(json!([
            {"name":"list_drafts","description":"List local drafts","inputSchema":{"type":"object","additionalProperties":false},"annotations":{"readOnlyHint":true}},
            {"name":"get_draft","description":"Read a local draft by ID","inputSchema":{"type":"object","properties":{"id":{"type":"integer","minimum":1}},"required":["id"],"additionalProperties":false},"annotations":{"readOnlyHint":true}}
        ]))),
        "tools/call" => {
            let (name, args) = tool_call(request.params.unwrap_or(&Value::Null))?;
            let invalid = || RpcError::new(-32602, "Invalid tool name or arguments");
            let result = match name {
                "list_drafts" if args.as_object().is_some_and(|args| args.is_empty()) => store
                    .list()
                    .and_then(|drafts| Ok(serde_json::to_string(&drafts)?)),
                "get_draft" if args.as_object().is_some_and(|args| args.len() == 1) => {
                    let id = args["id"]
                        .as_i64()
                        .filter(|id| *id > 0)
                        .ok_or_else(invalid)?;
                    store.get(id).and_then(|draft| {
                        let draft = draft.ok_or_else(|| anyhow::anyhow!("Draft not found"))?;
                        Ok(serde_json::to_string(&draft)?)
                    })
                }
                _ => return Err(invalid()),
            };
            Ok(match result {
                Ok(data) => text_tool_result(&data, false),
                Err(error) => text_tool_result(&error.to_string(), true),
            })
        }
        _ => Err(RpcError::new(-32601, "Method not found")),
    }
}
