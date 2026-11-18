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
    let names: Vec<_> = replies[1]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"create_draft"));
    assert!(names.contains(&"preview_draft"));
    assert!(
        !names
            .iter()
            .any(|name| name.contains("finalize") || name.contains("send"))
    );
    let read: Value =
        serde_json::from_str(replies[2]["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(read, draft);
}

#[test]
fn agent_prepares_a_draft_with_replay_and_stale_version_checks() {
    let folder = tempfile::tempdir().unwrap();
    let db = folder.path().join("test.sqlite3");
    let binary = env!("CARGO_BIN_EXE_facturecraft");
    let quote = json!({"recipient":"Example","lines":[{"description":"Work",
        "quantity":"1.5","price":"19.99","discount":"10","vat":"20"}]});
    let prepare = json!({"title":"Example quote","currency":"EUR","language":"en",
        "quote":quote,"idempotency_key":"example-quote-1"});
    let update = json!({"id":1,"expected_version":1,"title":"Updated quote",
        "currency":"EUR","language":"en","quote":quote});
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
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"create_draft","arguments":prepare}}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"create_draft","arguments":prepare}}),
        json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"update_draft","arguments":update}}),
        json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"update_draft","arguments":update}}),
        json!({"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"preview_draft","arguments":{"id":1}}}),
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
    assert_eq!(replies.len(), 6);
    let result = |index: usize| &replies[index]["result"];
    assert_eq!(result(1), result(2));
    assert_eq!(result(1)["isError"], false);
    assert_eq!(result(3)["isError"], false);
    assert_eq!(result(4)["isError"], true);
    assert_eq!(
        result(5)["content"][0]["resource"]["mimeType"],
        "application/pdf"
    );
    assert!(
        result(5)["content"][0]["resource"]["blob"]
            .as_str()
            .unwrap()
            .starts_with("JVBER")
    );
    let read = Command::new(binary)
        .arg("--db")
        .arg(db)
        .args(["draft", "details", "1"])
        .output()
        .unwrap();
    assert!(read.status.success(), "{:?}", read.stderr);
    let details: Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(details["draft"]["title"], "Updated quote");
    assert_eq!(details["billing"]["version"], 2);
    assert_eq!(details["totals"]["gross"], "32.39");
}

#[test]
fn backup_restores_archived_pdfs_and_refuses_to_replace_a_database() {
    let folder = tempfile::tempdir().unwrap();
    let db = folder.path().join("initial.sqlite3");
    let snapshot = folder.path().join("backup.sqlite3");
    let restored = folder.path().join("restored.sqlite3");
    let export = folder.path().join("export");
    let pdf = b"%PDF-1.4\nsynthetic original";
    for (directory, content) in [
        (
            "clients",
            json!({"clients": [{"id": 1, "name": "Example"}]}),
        ),
        ("societes", json!({"companies": []})),
        ("devis", json!({"quotations": [{"id":1, "uid":"TEST-1"}]})),
        ("factures_acomptes", json!({"invoices": []})),
    ] {
        let path = export.join(directory);
        std::fs::create_dir_all(path.join("pdf_originaux")).unwrap();
        std::fs::write(path.join("source.json"), content.to_string()).unwrap();
    }
    std::fs::write(export.join("devis/pdf_originaux/TEST-1.pdf"), pdf).unwrap();
    let run = |db: &std::path::Path, args: &[&std::ffi::OsStr]| {
        Command::new(env!("CARGO_BIN_EXE_facturecraft"))
            .arg("--db")
            .arg(db)
            .args(args)
            .output()
            .unwrap()
    };
    let import_args = [
        "import".as_ref(),
        export.as_os_str(),
        "--apply".as_ref(),
        "--source".as_ref(),
        "fixture".as_ref(),
    ];
    let first = run(&db, &import_args);
    assert!(first.status.success(), "{:?}", first.stderr);
    let first: Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(first["linked_pdfs"], 1);
    assert_eq!(first["inserted"], 3);
    let replay = run(&db, &import_args);
    assert!(replay.status.success(), "{:?}", replay.stderr);
    let replay: Value = serde_json::from_slice(&replay.stdout).unwrap();
    assert_eq!(replay["unchanged"], 3);
    let backup = run(&db, &["backup".as_ref(), snapshot.as_os_str()]);
    assert!(backup.status.success(), "{:?}", backup.stderr);
    let restore = || run(&restored, &["restore".as_ref(), snapshot.as_os_str()]);
    let first_restore = restore();
    assert!(first_restore.status.success(), "{:?}", first_restore.stderr);
    assert!(!restore().status.success());
    let output = folder.path().join("original.pdf");
    let extracted = run(
        &restored,
        &[
            "archive".as_ref(),
            "export-pdf".as_ref(),
            "fixture".as_ref(),
            "devis/TEST-1.pdf".as_ref(),
            output.as_os_str(),
        ],
    );
    assert!(extracted.status.success(), "{:?}", extracted.stderr);
    assert_eq!(std::fs::read(output).unwrap(), pdf);
}
