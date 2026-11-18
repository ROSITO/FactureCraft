use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn cli_draft_is_readable_over_mcp() {
    let folder = tempfile::tempdir().unwrap();
    let db = folder.path().join("test.sqlite3");
    let binary = env!("CARGO_BIN_EXE_facturecraft");
    let created = Command::new(binary)
        .arg("--db")
        .arg(&db)
        .args([
            "draft",
            "create",
            "Test devis",
            "--currency",
            "CHF",
            "--language",
            "en",
        ])
        .output()
        .unwrap();
    assert!(created.status.success(), "{:?}", created.stderr);
    let draft: Value = serde_json::from_slice(&created.stdout).unwrap();
    assert_eq!(draft["currency"], "CHF");
    assert_eq!(draft["language"], "en");

    let mut mcp = Command::new(binary)
        .arg("--db")
        .arg(&db)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = mcp.stdin.take().unwrap();
    for message in [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"smoke","version":"1"}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"get_draft","arguments":{"id":draft["id"]}}}),
    ] {
        writeln!(input, "{message}").unwrap();
    }
    drop(input);
    let output = mcp.wait_with_output().unwrap();
    assert!(output.status.success());
    let replies: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(replies.len(), 3);
    assert_eq!(replies[0]["result"]["serverInfo"]["name"], "facturecraft");
    assert_eq!(replies[1]["result"]["tools"].as_array().unwrap().len(), 2);
    let read: Value =
        serde_json::from_str(replies[2]["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(read, draft);
}
