#![allow(dead_code)]

use assert_cmd::prelude::*;
use rusqlite::{params, Connection};
use serde_json::json;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

pub fn granola(home: &std::path::Path) -> Command {
    let mut command = Command::cargo_bin("granola").unwrap();
    command.env("HOME", home).env("NO_COLOR", "1");
    command
}

pub fn temp_home_with_cache() -> TempDir {
    let home = TempDir::new().unwrap();
    fs::create_dir_all(sqlite_cache_path(home.path()).parent().unwrap()).unwrap();
    home
}

pub fn sqlite_cache_path(home: &std::path::Path) -> std::path::PathBuf {
    #[cfg(target_os = "macos")]
    return home
        .join("Library")
        .join("Caches")
        .join("granola-cli")
        .join("notes.sqlite");

    #[cfg(not(target_os = "macos"))]
    return home.join(".cache").join("granola-cli").join("notes.sqlite");
}

pub fn legacy_cache_path(home: &std::path::Path) -> std::path::PathBuf {
    sqlite_cache_path(home).with_file_name("notes.json")
}

pub fn create_empty_cache(home: &std::path::Path) {
    let conn = Connection::open(sqlite_cache_path(home)).unwrap();
    init_schema(&conn);
}

pub fn seed_cache(home: &std::path::Path) {
    let conn = Connection::open(sqlite_cache_path(home)).unwrap();
    init_schema(&conn);
    insert_note(
        &conn,
        "not_async",
        "Async Config Refinement",
        "chris@example.com",
        "Will Example will@example.com",
        "We discussed async config rollout details.",
        "mint alpha",
        true,
    );
    insert_note(
        &conn,
        "not_mint",
        "Mint Planning",
        "chris@example.com",
        "Avery Example avery@example.com",
        "Planning notes about mint flows.",
        "",
        false,
    );
    insert_summary_only(
        &conn,
        "not_summary",
        "Summary Only Mint",
        "owner@example.com",
    );
}

fn init_schema(conn: &Connection) {
    conn.execute_batch(
        "CREATE TABLE cache_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
         CREATE TABLE note_summaries (
           id TEXT PRIMARY KEY,
           summary_json TEXT NOT NULL,
           title TEXT,
           owner_name TEXT,
           owner_email TEXT NOT NULL,
           created_at TEXT NOT NULL,
           updated_at TEXT NOT NULL,
           cached_at TEXT NOT NULL
         );
         CREATE TABLE notes (
           id TEXT PRIMARY KEY REFERENCES note_summaries(id) ON DELETE CASCADE,
           note_json TEXT NOT NULL,
           hydrated_at TEXT NOT NULL,
           includes_transcript INTEGER NOT NULL DEFAULT 0
         );
         CREATE VIRTUAL TABLE note_search USING fts5(
           note_id UNINDEXED,
           id,
           title,
           owner,
           summary_text,
           summary_markdown,
           attendees,
           folders,
           transcript
         );
         INSERT INTO cache_meta (key, value) VALUES ('version', '2'), ('synced_at', '2026-06-09T00:00:00Z');",
    )
    .unwrap();
}

#[allow(clippy::too_many_arguments)]
fn insert_note(
    conn: &Connection,
    id: &str,
    title: &str,
    owner_email: &str,
    attendees: &str,
    summary_text: &str,
    transcript: &str,
    includes_transcript: bool,
) {
    let summary = summary_json(id, title, owner_email);
    let note = json!({
        "id": id,
        "object": "note",
        "title": title,
        "owner": { "name": "Chris", "email": owner_email },
        "created_at": "2026-06-01T00:00:00Z",
        "updated_at": "2026-06-09T00:00:00Z",
        "web_url": "https://notes.granola.ai/d/test",
        "calendar_event": null,
        "attendees": attendees.split_whitespace().filter(|part| part.contains('@')).map(|email| json!({ "name": null, "email": email })).collect::<Vec<_>>(),
        "folder_membership": [],
        "summary_text": summary_text,
        "summary_markdown": null,
        "transcript": includes_transcript.then(|| vec![json!({
            "speaker": { "source": "speaker", "diarization_label": null },
            "text": transcript,
            "start_time": "2026-06-01T00:00:00Z",
            "end_time": "2026-06-01T00:00:01Z"
        })])
    });
    insert_summary(conn, id, title, owner_email, &summary);
    conn.execute(
        "INSERT INTO notes (id, note_json, hydrated_at, includes_transcript) VALUES (?1, ?2, ?3, ?4)",
        params![id, note.to_string(), "2026-06-09T00:00:00Z", includes_transcript as i64],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO note_search (note_id, id, title, owner, summary_text, summary_markdown, attendees, folders, transcript)
         VALUES (?1, ?2, ?3, ?4, ?5, '', ?6, '', ?7)",
        params![id, id, title, owner_email, summary_text, attendees, transcript],
    )
    .unwrap();
}

fn insert_summary_only(conn: &Connection, id: &str, title: &str, owner_email: &str) {
    let summary = summary_json(id, title, owner_email);
    insert_summary(conn, id, title, owner_email, &summary);
    conn.execute(
        "INSERT INTO note_search (note_id, id, title, owner, summary_text, summary_markdown, attendees, folders, transcript)
         VALUES (?1, ?2, ?3, ?4, '', '', '', '', '')",
        params![id, id, title, owner_email],
    )
    .unwrap();
}

fn insert_summary(
    conn: &Connection,
    id: &str,
    title: &str,
    owner_email: &str,
    summary: &serde_json::Value,
) {
    conn.execute(
        "INSERT INTO note_summaries
         (id, summary_json, title, owner_name, owner_email, created_at, updated_at, cached_at)
         VALUES (?1, ?2, ?3, 'Chris', ?4, '2026-06-01T00:00:00Z', '2026-06-09T00:00:00Z', '2026-06-09T00:00:00Z')",
        params![id, summary.to_string(), title, owner_email],
    )
    .unwrap();
}

fn summary_json(id: &str, title: &str, owner_email: &str) -> serde_json::Value {
    json!({
        "id": id,
        "object": "note",
        "title": title,
        "owner": { "name": "Chris", "email": owner_email },
        "created_at": "2026-06-01T00:00:00Z",
        "updated_at": "2026-06-09T00:00:00Z"
    })
}
