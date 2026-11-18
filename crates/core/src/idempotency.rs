use anyhow::{Result, ensure};
use rusqlite::{Connection, OptionalExtension, params};

pub(crate) fn lookup(connection: &Connection, key: &str, request: &str) -> Result<Option<i64>> {
    ensure!(
        !key.trim().is_empty() && key.len() <= 200,
        "Idempotency key must contain 1–200 bytes"
    );
    let existing: Option<(String, i64)> = connection
        .query_row(
            "SELECT request, draft_id FROM draft_requests WHERE request_key=?1",
            [key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if let Some((stored_request, id)) = existing {
        ensure!(
            stored_request == request,
            "Idempotency key already used for a different request"
        );
        return Ok(Some(id));
    }
    Ok(None)
}

pub(crate) fn record(connection: &Connection, key: &str, request: &str, id: i64) -> Result<()> {
    connection.execute(
        "INSERT INTO draft_requests(request_key, request, draft_id) VALUES (?1, ?2, ?3)",
        params![key, request, id],
    )?;
    Ok(())
}
