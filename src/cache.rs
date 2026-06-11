use crate::error::CliError;
use crate::types::{Note, NoteSummary, TranscriptItem, User};
use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const CACHE_VERSION: u8 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotesCache {
    pub version: u8,
    pub synced_at: String,
    #[serde(default)]
    pub summaries: Vec<NoteSummary>,
    pub notes: Vec<Note>,
}

impl Default for NotesCache {
    fn default() -> Self {
        Self {
            version: CACHE_VERSION,
            synced_at: now(),
            summaries: Vec::new(),
            notes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CacheSearchHit {
    pub summary: NoteSummary,
    pub note: Option<Note>,
    pub cached_detail: bool,
    pub cached_transcript: bool,
    pub rank: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CacheStatus {
    pub path: PathBuf,
    pub exists: bool,
    pub legacy_json_path: PathBuf,
    pub legacy_json_exists: bool,
    pub version: Option<u8>,
    pub synced_at: Option<String>,
    pub summaries: usize,
    pub notes: usize,
    pub hydrated_notes: usize,
    pub transcript_notes: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct CacheVerifyResult {
    pub path: PathBuf,
    pub exists: bool,
    pub integrity_check: Option<String>,
    pub ok: bool,
    pub summaries: usize,
    pub hydrated_notes: usize,
    pub transcript_notes: usize,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OsCacheStore;

#[derive(Debug, Clone, Copy)]
pub struct CacheMode {
    pub read: bool,
    pub write: bool,
}

impl CacheMode {
    pub fn new(read: bool, write: bool) -> Self {
        Self { read, write }
    }
}

pub trait CacheStore {
    fn upsert_summaries(&self, summaries: &[NoteSummary]) -> Result<(), CliError>;
    fn upsert_notes(&self, notes: &[Note]) -> Result<(), CliError>;
    fn get_note(&self, note_id: &str) -> Result<Option<Note>, CliError>;
    fn status(&self) -> Result<CacheStatus, CliError>;
    fn search(
        &self,
        query: &str,
        limit: Option<usize>,
    ) -> Result<Option<Vec<CacheSearchHit>>, CliError>;
    fn contains_fresh_summary(&self, remote: &NoteSummary) -> Result<bool, CliError>;
}

impl CacheStore for OsCacheStore {
    fn upsert_summaries(&self, summaries: &[NoteSummary]) -> Result<(), CliError> {
        upsert_summaries(summaries)
    }

    fn upsert_notes(&self, notes: &[Note]) -> Result<(), CliError> {
        upsert_notes(notes)
    }

    fn get_note(&self, note_id: &str) -> Result<Option<Note>, CliError> {
        get_note(note_id)
    }

    fn status(&self) -> Result<CacheStatus, CliError> {
        status()
    }

    fn search(
        &self,
        query: &str,
        limit: Option<usize>,
    ) -> Result<Option<Vec<CacheSearchHit>>, CliError> {
        search(query, limit)
    }

    fn contains_fresh_summary(&self, remote: &NoteSummary) -> Result<bool, CliError> {
        contains_fresh_summary(remote)
    }
}

pub fn cache_path() -> Result<PathBuf, CliError> {
    let base = dirs::cache_dir()
        .ok_or_else(|| CliError::general("could not determine the OS cache directory"))?;
    Ok(base.join("granola-cli").join("notes.sqlite"))
}

pub fn legacy_cache_path() -> Result<PathBuf, CliError> {
    let base = dirs::cache_dir()
        .ok_or_else(|| CliError::general("could not determine the OS cache directory"))?;
    Ok(base.join("granola-cli").join("notes.json"))
}

pub fn save(cache: &NotesCache) -> Result<(), CliError> {
    let path = cache_path()?;
    let mut conn = open_cache(&path)?;
    reset_tables(&mut conn)?;
    save_to_conn(&mut conn, cache)
}

pub fn upsert_summaries(summaries: &[NoteSummary]) -> Result<(), CliError> {
    if summaries.is_empty() {
        return Ok(());
    }

    let path = cache_path()?;
    let mut conn = open_cache(&path)?;
    upsert_summaries_in_conn(&mut conn, summaries)
}

pub fn upsert_notes(notes: &[Note]) -> Result<(), CliError> {
    if notes.is_empty() {
        return Ok(());
    }

    let path = cache_path()?;
    let mut conn = open_cache(&path)?;
    upsert_notes_in_conn(&mut conn, notes)
}

pub fn get_note(note_id: &str) -> Result<Option<Note>, CliError> {
    let path = cache_path()?;
    if !path.exists() {
        return Ok(None);
    }

    let conn = open_existing(&path)?;
    let raw: Option<String> = conn
        .query_row(
            "SELECT note_json FROM notes WHERE id = ?1",
            params![note_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_error)?;
    raw.map(|value| serde_json::from_str(&value).map_err(CliError::from))
        .transpose()
}

pub fn clear() -> Result<bool, CliError> {
    let paths = [cache_path()?, legacy_cache_path()?];
    let mut removed = false;
    for path in paths {
        if !path.exists() {
            continue;
        }
        fs::remove_file(&path).map_err(|error| {
            CliError::general(format!(
                "failed to remove cache {}: {error}",
                path.display()
            ))
        })?;
        removed = true;
    }
    Ok(removed)
}

pub fn status() -> Result<CacheStatus, CliError> {
    let path = cache_path()?;
    let legacy_json_path = legacy_cache_path()?;
    if !path.exists() {
        return Ok(CacheStatus {
            path,
            exists: false,
            legacy_json_exists: legacy_json_path.exists(),
            legacy_json_path,
            version: None,
            synced_at: None,
            summaries: 0,
            notes: 0,
            hydrated_notes: 0,
            transcript_notes: 0,
        });
    }

    let conn = open_existing(&path)?;
    let hydrated_notes = count_rows(&conn, "notes")?;
    Ok(CacheStatus {
        path,
        exists: true,
        legacy_json_exists: legacy_json_path.exists(),
        legacy_json_path,
        version: meta_value(&conn, "version")?.and_then(|value| value.parse().ok()),
        synced_at: meta_value(&conn, "synced_at")?,
        summaries: count_rows(&conn, "note_summaries")?,
        notes: hydrated_notes,
        hydrated_notes,
        transcript_notes: count_transcript_notes(&conn)?,
    })
}

pub fn vacuum() -> Result<CacheStatus, CliError> {
    let path = cache_path()?;
    let conn = open_cache(&path)?;
    conn.execute_batch("VACUUM;").map_err(db_error)?;
    status()
}

pub fn verify() -> Result<CacheVerifyResult, CliError> {
    let status = status()?;
    if !status.exists {
        return Ok(CacheVerifyResult {
            path: status.path,
            exists: false,
            integrity_check: None,
            ok: true,
            summaries: 0,
            hydrated_notes: 0,
            transcript_notes: 0,
        });
    }
    let conn = open_existing(&status.path)?;
    let integrity_check: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(db_error)?;
    Ok(CacheVerifyResult {
        path: status.path,
        exists: true,
        ok: integrity_check == "ok",
        integrity_check: Some(integrity_check),
        summaries: status.summaries,
        hydrated_notes: status.hydrated_notes,
        transcript_notes: status.transcript_notes,
    })
}

pub fn export_jsonl() -> Result<Option<String>, CliError> {
    let path = cache_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let conn = open_existing(&path)?;
    export_jsonl_from_conn(&conn).map(Some)
}

fn export_jsonl_from_conn(conn: &Connection) -> Result<String, CliError> {
    let mut stmt = conn
        .prepare(
            "SELECT s.id, s.summary_json, n.note_json, n.includes_transcript \
             FROM note_summaries s \
             LEFT JOIN notes n ON n.id = s.id \
             ORDER BY s.updated_at DESC",
        )
        .map_err(db_error)?;
    let rows = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let summary_json: String = row.get(1)?;
            let note_json: Option<String> = row.get(2)?;
            let includes_transcript: Option<i64> = row.get(3)?;
            Ok((id, summary_json, note_json, includes_transcript))
        })
        .map_err(db_error)?;
    let mut output = String::new();
    for row in rows {
        let (id, summary_json, note_json, includes_transcript) = row.map_err(db_error)?;
        let summary: serde_json::Value =
            serde_json::from_str(&summary_json).map_err(CliError::from)?;
        let note: Option<serde_json::Value> = note_json
            .map(|value| serde_json::from_str(&value))
            .transpose()
            .map_err(CliError::from)?;
        output.push_str(
            &serde_json::to_string(&json!({
                "id": id,
                "summary": summary,
                "note": note,
                "cached_detail": note.is_some(),
                "cached_transcript": includes_transcript.unwrap_or(0) != 0,
            }))
            .map_err(CliError::from)?,
        );
        output.push('\n');
    }
    Ok(output)
}

pub fn search(query: &str, limit: Option<usize>) -> Result<Option<Vec<CacheSearchHit>>, CliError> {
    let path = cache_path()?;
    if !path.exists() {
        return Ok(None);
    }

    let conn = open_existing(&path)?;
    search_in_conn(&conn, query, limit)
}

fn search_in_conn(
    conn: &Connection,
    query: &str,
    limit: Option<usize>,
) -> Result<Option<Vec<CacheSearchHit>>, CliError> {
    let sql = format!(
        "SELECT s.summary_json, n.note_json, n.includes_transcript, bm25(note_search) AS rank \
         FROM note_search \
         JOIN note_summaries s ON s.id = note_search.note_id \
         LEFT JOIN notes n ON n.id = s.id \
         WHERE note_search MATCH ?1 \
         ORDER BY rank, s.updated_at DESC, s.created_at DESC \
         LIMIT {}",
        limit.unwrap_or(100)
    );

    query_search(conn, &sql, query).or_else(|_| {
        let quoted = quote_fts_query(query);
        query_search(conn, &sql, &quoted)
    })
}

pub fn normalize_search_query(parts: &[String]) -> String {
    parts
        .iter()
        .map(|part| {
            if part.chars().any(char::is_whitespace) {
                quote_fts_query(part)
            } else {
                part.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn contains_fresh_summary(remote: &NoteSummary) -> Result<bool, CliError> {
    let path = cache_path()?;
    if !path.exists() {
        return Ok(false);
    }
    let conn = open_existing(&path)?;
    let updated_at: Option<String> = conn
        .query_row(
            "SELECT updated_at FROM note_summaries WHERE id = ?1",
            params![remote.id],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_error)?;

    Ok(updated_at.is_some_and(|updated_at| updated_at.as_str() >= remote.updated_at.as_str()))
}

pub fn merge_notes(existing: Option<NotesCache>, fetched: Vec<Note>) -> NotesCache {
    let existing = existing.unwrap_or_default();
    let mut summary_by_id: BTreeMap<String, NoteSummary> = existing
        .summaries
        .into_iter()
        .map(|summary| (summary.id.clone(), summary))
        .collect();
    let mut note_by_id: BTreeMap<String, Note> = existing
        .notes
        .into_iter()
        .map(|note| (note.id.clone(), note))
        .collect();

    for note in fetched {
        summary_by_id.insert(note.id.clone(), NoteSummary::from(&note));
        note_by_id.insert(note.id.clone(), note);
    }

    NotesCache {
        version: CACHE_VERSION,
        synced_at: now(),
        summaries: summary_by_id.into_values().collect(),
        notes: note_by_id.into_values().collect(),
    }
}

#[cfg(test)]
pub fn merge_summaries(existing: Option<NotesCache>, fetched: Vec<NoteSummary>) -> NotesCache {
    let existing = existing.unwrap_or_default();
    let mut by_id: BTreeMap<String, NoteSummary> = existing
        .summaries
        .into_iter()
        .map(|summary| (summary.id.clone(), summary))
        .collect();

    for summary in fetched {
        by_id.insert(summary.id.clone(), summary);
    }

    NotesCache {
        version: CACHE_VERSION,
        synced_at: now(),
        summaries: by_id.into_values().collect(),
        notes: existing.notes,
    }
}

#[cfg(test)]
pub fn search_notes(cache: &NotesCache, query: &str) -> Vec<CacheSearchHit> {
    let query = query.to_lowercase();
    let notes_by_id: BTreeMap<&str, &Note> = cache
        .notes
        .iter()
        .map(|note| (note.id.as_str(), note))
        .collect();

    let mut summaries_by_id: BTreeMap<String, NoteSummary> = cache
        .summaries
        .iter()
        .cloned()
        .map(|summary| (summary.id.clone(), summary))
        .collect();
    for note in &cache.notes {
        summaries_by_id.insert(note.id.clone(), NoteSummary::from(note));
    }

    summaries_by_id
        .into_values()
        .filter_map(|summary| {
            let note = notes_by_id.get(summary.id.as_str()).copied();
            let matches = note.is_some_and(|note| note_matches(note, &query))
                || summary_matches(&summary, &query);
            matches.then(|| CacheSearchHit {
                summary,
                note: note.cloned(),
                cached_detail: note.is_some(),
                cached_transcript: note.and_then(|note| note.transcript.as_ref()).is_some(),
                rank: 0.0,
            })
        })
        .collect()
}

#[cfg(test)]
impl NotesCache {
    pub fn hydrated_note_count(&self) -> usize {
        self.notes.len()
    }

    pub fn summary_count(&self) -> usize {
        self.summaries.len()
    }

    pub fn has_unhydrated_summaries(&self) -> bool {
        self.summaries.len() > self.notes.len()
    }
}

fn open_cache(path: &Path) -> Result<Connection, CliError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            CliError::general(format!(
                "failed to create cache directory {}: {error}",
                parent.display()
            ))
        })?;
    }

    let conn = Connection::open(path).map_err(db_error)?;
    init_schema(&conn)?;
    Ok(conn)
}

fn open_existing(path: &Path) -> Result<Connection, CliError> {
    let conn = Connection::open(path).map_err(db_error)?;
    init_schema(&conn)?;
    Ok(conn)
}

fn init_schema(conn: &Connection) -> Result<(), CliError> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;
         CREATE TABLE IF NOT EXISTS cache_meta (
           key TEXT PRIMARY KEY,
           value TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS note_summaries (
           id TEXT PRIMARY KEY,
           summary_json TEXT NOT NULL,
           title TEXT,
           owner_name TEXT,
           owner_email TEXT NOT NULL,
           created_at TEXT NOT NULL,
           updated_at TEXT NOT NULL,
           cached_at TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS notes (
           id TEXT PRIMARY KEY REFERENCES note_summaries(id) ON DELETE CASCADE,
           note_json TEXT NOT NULL,
           hydrated_at TEXT NOT NULL,
           includes_transcript INTEGER NOT NULL DEFAULT 0
         );
         CREATE VIRTUAL TABLE IF NOT EXISTS note_search USING fts5(
           note_id UNINDEXED,
           id,
           title,
           owner,
           summary_text,
           summary_markdown,
           attendees,
           folders,
           transcript
         );",
    )
    .map_err(db_error)?;
    set_meta(conn, "version", &CACHE_VERSION.to_string())?;
    Ok(())
}

fn reset_tables(conn: &mut Connection) -> Result<(), CliError> {
    let tx = conn.transaction().map_err(db_error)?;
    tx.execute("DELETE FROM note_search", [])
        .map_err(db_error)?;
    tx.execute("DELETE FROM notes", []).map_err(db_error)?;
    tx.execute("DELETE FROM note_summaries", [])
        .map_err(db_error)?;
    tx.commit().map_err(db_error)
}

fn save_to_conn(conn: &mut Connection, cache: &NotesCache) -> Result<(), CliError> {
    upsert_summaries_in_conn(conn, &cache.summaries)?;
    upsert_notes_in_conn(conn, &cache.notes)?;
    set_meta(conn, "synced_at", &cache.synced_at)
}

fn upsert_summaries_in_conn(
    conn: &mut Connection,
    summaries: &[NoteSummary],
) -> Result<(), CliError> {
    let cached_at = now();
    let tx = conn.transaction().map_err(db_error)?;
    for summary in summaries {
        upsert_summary(&tx, summary, &cached_at)?;
        upsert_search_row(&tx, summary, None)?;
    }
    set_meta_in_conn(&tx, "synced_at", &cached_at)?;
    tx.commit().map_err(db_error)
}

fn upsert_notes_in_conn(conn: &mut Connection, notes: &[Note]) -> Result<(), CliError> {
    let cached_at = now();
    let tx = conn.transaction().map_err(db_error)?;
    for note in notes {
        let summary = NoteSummary::from(note);
        upsert_summary(&tx, &summary, &cached_at)?;
        tx.execute(
            "INSERT INTO notes (id, note_json, hydrated_at, includes_transcript)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET
               note_json = excluded.note_json,
               hydrated_at = excluded.hydrated_at,
               includes_transcript = excluded.includes_transcript",
            params![
                note.id,
                serde_json::to_string(note).map_err(CliError::from)?,
                cached_at,
                note.transcript.is_some() as i64,
            ],
        )
        .map_err(db_error)?;
        upsert_search_row(&tx, &summary, Some(note))?;
    }
    set_meta_in_conn(&tx, "synced_at", &cached_at)?;
    tx.commit().map_err(db_error)
}

fn upsert_summary(
    conn: &Connection,
    summary: &NoteSummary,
    cached_at: &str,
) -> Result<(), CliError> {
    conn.execute(
        "INSERT INTO note_summaries
         (id, summary_json, title, owner_name, owner_email, created_at, updated_at, cached_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(id) DO UPDATE SET
           summary_json = excluded.summary_json,
           title = excluded.title,
           owner_name = excluded.owner_name,
           owner_email = excluded.owner_email,
           created_at = excluded.created_at,
           updated_at = excluded.updated_at,
           cached_at = excluded.cached_at",
        params![
            summary.id,
            serde_json::to_string(summary).map_err(CliError::from)?,
            summary.title,
            summary.owner.name,
            summary.owner.email,
            summary.created_at,
            summary.updated_at,
            cached_at,
        ],
    )
    .map_err(db_error)?;
    Ok(())
}

fn upsert_search_row(
    conn: &Connection,
    summary: &NoteSummary,
    note: Option<&Note>,
) -> Result<(), CliError> {
    conn.execute(
        "DELETE FROM note_search WHERE note_id = ?1",
        params![summary.id],
    )
    .map_err(db_error)?;
    conn.execute(
        "INSERT INTO note_search
         (note_id, id, title, owner, summary_text, summary_markdown, attendees, folders, transcript)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            summary.id,
            summary.id,
            summary.title.as_deref().unwrap_or(""),
            user_text(&summary.owner),
            note.map_or("", |note| note.summary_text.as_str()),
            note.and_then(|note| note.summary_markdown.as_deref())
                .unwrap_or(""),
            note.map(attendees_text).unwrap_or_default(),
            note.map(folders_text).unwrap_or_default(),
            note.and_then(transcript_text).unwrap_or_default(),
        ],
    )
    .map_err(db_error)?;
    Ok(())
}

fn query_search(
    conn: &Connection,
    sql: &str,
    query: &str,
) -> Result<Option<Vec<CacheSearchHit>>, CliError> {
    let mut stmt = conn.prepare(sql).map_err(db_error)?;
    let rows = stmt
        .query_map(params![query], |row| {
            let summary_json: String = row.get(0)?;
            let note_json: Option<String> = row.get(1)?;
            let includes_transcript: Option<i64> = row.get(2)?;
            let rank: f64 = row.get(3)?;
            Ok((summary_json, note_json, includes_transcript, rank))
        })
        .map_err(db_error)?;

    let mut hits = Vec::new();
    for row in rows {
        let (summary_json, note_json, includes_transcript, rank) = row.map_err(db_error)?;
        let summary = serde_json::from_str(&summary_json).map_err(CliError::from)?;
        let note: Option<Note> = note_json
            .map(|raw| serde_json::from_str(&raw).map_err(CliError::from))
            .transpose()?;
        hits.push(CacheSearchHit {
            summary,
            cached_detail: note.is_some(),
            cached_transcript: includes_transcript == Some(1),
            note,
            rank,
        });
    }

    Ok(Some(hits))
}

fn meta_value(conn: &Connection, key: &str) -> Result<Option<String>, CliError> {
    conn.query_row(
        "SELECT value FROM cache_meta WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
    .map_err(db_error)
}

fn set_meta(conn: &Connection, key: &str, value: &str) -> Result<(), CliError> {
    set_meta_in_conn(conn, key, value)
}

fn set_meta_in_conn(conn: &Connection, key: &str, value: &str) -> Result<(), CliError> {
    conn.execute(
        "INSERT INTO cache_meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(db_error)?;
    Ok(())
}

fn count_rows(conn: &Connection, table: &str) -> Result<usize, CliError> {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
        row.get::<_, i64>(0)
    })
    .map(|count| count as usize)
    .map_err(db_error)
}

fn count_transcript_notes(conn: &Connection) -> Result<usize, CliError> {
    conn.query_row(
        "SELECT COUNT(*) FROM notes WHERE includes_transcript = 1",
        [],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count as usize)
    .map_err(db_error)
}

#[cfg(test)]
fn summary_matches(summary: &NoteSummary, query: &str) -> bool {
    contains(summary.id.as_str(), query)
        || summary
            .title
            .as_deref()
            .is_some_and(|title| contains(title, query))
        || summary
            .owner
            .name
            .as_deref()
            .is_some_and(|name| contains(name, query))
        || contains(&summary.owner.email, query)
}

#[cfg(test)]
fn note_matches(note: &Note, query: &str) -> bool {
    contains(note.id.as_str(), query)
        || note
            .title
            .as_deref()
            .is_some_and(|title| contains(title, query))
        || contains(&note.summary_text, query)
        || note
            .summary_markdown
            .as_deref()
            .is_some_and(|summary| contains(summary, query))
        || note
            .owner
            .name
            .as_deref()
            .is_some_and(|name| contains(name, query))
        || contains(&note.owner.email, query)
        || note.attendees.iter().any(|user| {
            user.name
                .as_deref()
                .is_some_and(|name| contains(name, query))
                || contains(&user.email, query)
        })
        || note
            .folder_membership
            .iter()
            .any(|folder| contains(&folder.id, query) || contains(&folder.name, query))
        || note
            .transcript
            .as_ref()
            .is_some_and(|transcript| transcript.iter().any(|item| contains(&item.text, query)))
}

#[cfg(test)]
fn contains(value: &str, query: &str) -> bool {
    value.to_lowercase().contains(query)
}

fn user_text(user: &User) -> String {
    [user.name.as_deref(), Some(user.email.as_str())]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ")
}

fn attendees_text(note: &Note) -> String {
    note.attendees
        .iter()
        .map(user_text)
        .collect::<Vec<_>>()
        .join(" ")
}

fn folders_text(note: &Note) -> String {
    note.folder_membership
        .iter()
        .flat_map(|folder| [folder.id.as_str(), folder.name.as_str()])
        .collect::<Vec<_>>()
        .join(" ")
}

fn transcript_text(note: &Note) -> Option<String> {
    note.transcript
        .as_ref()
        .map(|items| transcript_items_text(items))
}

fn transcript_items_text(items: &[TranscriptItem]) -> String {
    items
        .iter()
        .map(|item| item.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

fn quote_fts_query(query: &str) -> String {
    format!("\"{}\"", query.replace('"', "\"\""))
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn db_error(error: rusqlite::Error) -> CliError {
    CliError::general(format!("local cache database error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ListNotesResponse, Note, User};

    #[test]
    fn search_matches_summary_text() {
        let note: Note =
            serde_json::from_str(include_str!("../tests/fixtures/get_note.json")).unwrap();
        let cache = NotesCache {
            notes: vec![note],
            ..Default::default()
        };

        assert_eq!(search_notes(&cache, "summary").len(), 1);
        assert!(search_notes(&cache, "definitely absent").is_empty());
    }

    #[test]
    fn merge_summaries_populates_list_cache_without_hydrated_notes() {
        let response: ListNotesResponse =
            serde_json::from_str(include_str!("../tests/fixtures/list_notes.json")).unwrap();

        let cache = merge_summaries(None, response.notes);

        assert_eq!(cache.summary_count(), 1);
        assert_eq!(cache.hydrated_note_count(), 0);
        assert!(cache.has_unhydrated_summaries());
        assert_eq!(search_notes(&cache, "Redacted").len(), 1);
        assert!(!search_notes(&cache, "Redacted")[0].cached_detail);
    }

    #[test]
    fn merge_notes_updates_summary_and_hydrated_cache() {
        let response: ListNotesResponse =
            serde_json::from_str(include_str!("../tests/fixtures/list_notes.json")).unwrap();
        let listed_cache = merge_summaries(None, response.notes);
        let note: Note =
            serde_json::from_str(include_str!("../tests/fixtures/get_note.json")).unwrap();

        let cache = merge_notes(Some(listed_cache), vec![note]);

        assert_eq!(cache.summary_count(), 1);
        assert_eq!(cache.hydrated_note_count(), 1);
        assert!(!cache.has_unhydrated_summaries());
        let hits = search_notes(&cache, "summary");
        assert_eq!(hits.len(), 1);
        assert!(hits[0].cached_detail);
    }

    #[test]
    fn sqlite_fts_searches_hydrated_note_body_and_transcript() {
        let mut conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        let note: Note = serde_json::from_str(include_str!(
            "../tests/fixtures/get_note_with_transcript.json"
        ))
        .unwrap();

        upsert_notes_in_conn(&mut conn, &[note]).unwrap();
        let hits = query_search(
            &conn,
            "SELECT s.summary_json, n.note_json, n.includes_transcript, bm25(note_search) AS rank \
             FROM note_search \
             JOIN note_summaries s ON s.id = note_search.note_id \
             LEFT JOIN notes n ON n.id = s.id \
             WHERE note_search MATCH ?1 \
             ORDER BY rank LIMIT 10",
            "transcript",
        )
        .unwrap()
        .unwrap();

        assert_eq!(hits.len(), 1);
        assert!(hits[0].cached_detail);
        assert!(hits[0].cached_transcript);
    }

    #[test]
    fn sqlite_fts_searches_summary_only_rows() {
        let mut conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        let response: ListNotesResponse =
            serde_json::from_str(include_str!("../tests/fixtures/list_notes.json")).unwrap();

        upsert_summaries_in_conn(&mut conn, &response.notes).unwrap();
        let hits = query_search(
            &conn,
            "SELECT s.summary_json, n.note_json, n.includes_transcript, bm25(note_search) AS rank \
             FROM note_search \
             JOIN note_summaries s ON s.id = note_search.note_id \
             LEFT JOIN notes n ON n.id = s.id \
             WHERE note_search MATCH ?1 \
             ORDER BY rank LIMIT 10",
            "Redacted",
        )
        .unwrap()
        .unwrap();

        assert_eq!(hits.len(), 1);
        assert!(!hits[0].cached_detail);
        assert!(!hits[0].cached_transcript);
    }

    #[test]
    fn normalizes_multi_argument_search_queries() {
        assert_eq!(
            normalize_search_query(&["attendees:will".to_string(), "async config".to_string(),]),
            "attendees:will \"async config\""
        );
        assert_eq!(
            normalize_search_query(&["async".to_string(), "config".to_string()]),
            "async config"
        );
    }

    #[test]
    fn sqlite_fts_supports_attendee_field_filters_and_phrases() {
        let mut conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        let mut matching_note: Note =
            serde_json::from_str(include_str!("../tests/fixtures/get_note.json")).unwrap();
        matching_note.id = "not_matching".to_string();
        matching_note.title = Some("Async Config Refinement".to_string());
        matching_note.summary_text = "We discussed async config rollout details.".to_string();
        matching_note.attendees = vec![User {
            name: Some("Will Example".to_string()),
            email: "will@example.com".to_string(),
        }];

        let mut non_matching_note = matching_note.clone();
        non_matching_note.id = "not_non_matching".to_string();
        non_matching_note.attendees = vec![User {
            name: Some("Avery Example".to_string()),
            email: "avery@example.com".to_string(),
        }];

        upsert_notes_in_conn(&mut conn, &[matching_note, non_matching_note]).unwrap();
        let hits = search_in_conn(&conn, "attendees:will \"async config\"", None)
            .unwrap()
            .unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].summary.id, "not_matching");
    }

    #[test]
    fn sqlite_fts_honors_result_limits() {
        let mut conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        let mut first_note: Note =
            serde_json::from_str(include_str!("../tests/fixtures/get_note.json")).unwrap();
        first_note.id = "not_first".to_string();
        first_note.summary_text = "mint alpha".to_string();
        let mut second_note = first_note.clone();
        second_note.id = "not_second".to_string();
        second_note.summary_text = "mint beta".to_string();

        upsert_notes_in_conn(&mut conn, &[first_note, second_note]).unwrap();
        let hits = search_in_conn(&conn, "mint", Some(1)).unwrap().unwrap();

        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn sqlite_fts_tiebreaks_search_results_by_most_recent_update() {
        let mut conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        let mut old_note: Note =
            serde_json::from_str(include_str!("../tests/fixtures/get_note.json")).unwrap();
        old_note.id = "not_old".to_string();
        old_note.summary_text = "mint planning".to_string();
        old_note.created_at = "2026-01-01T00:00:00Z".to_string();
        old_note.updated_at = "2026-01-01T00:00:00Z".to_string();
        let mut new_note = old_note.clone();
        new_note.id = "not_new".to_string();
        new_note.created_at = "2026-01-02T00:00:00Z".to_string();
        new_note.updated_at = "2026-01-03T00:00:00Z".to_string();

        upsert_notes_in_conn(&mut conn, &[old_note, new_note]).unwrap();
        let hits = search_in_conn(&conn, "mint", None).unwrap().unwrap();

        assert_eq!(
            hits.iter()
                .map(|hit| hit.summary.id.as_str())
                .collect::<Vec<_>>(),
            vec!["not_new", "not_old"]
        );
    }

    #[test]
    fn sqlite_fts_falls_back_to_quoted_query_for_invalid_syntax() {
        let mut conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        let mut note: Note =
            serde_json::from_str(include_str!("../tests/fixtures/get_note.json")).unwrap();
        note.title = Some("Question mark syntax".to_string());

        upsert_notes_in_conn(&mut conn, &[note]).unwrap();
        let hits = search_in_conn(&conn, "?", None).unwrap().unwrap();

        assert!(hits.is_empty());
    }

    #[test]
    fn exports_cache_rows_as_jsonl() {
        let mut conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        let note: Note = serde_json::from_str(include_str!(
            "../tests/fixtures/get_note_with_transcript.json"
        ))
        .unwrap();
        upsert_notes_in_conn(&mut conn, std::slice::from_ref(&note)).unwrap();

        let jsonl = export_jsonl_from_conn(&conn).unwrap();
        let rows: Vec<serde_json::Value> = jsonl
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["id"], note.id);
        assert_eq!(rows[0]["cached_detail"], true);
        assert_eq!(rows[0]["cached_transcript"], true);
    }
}
