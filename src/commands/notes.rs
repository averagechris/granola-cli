use crate::api::{
    resolve_api_key, try_resolve_api_key, validate_page_size, GranolaClient, ListNotesParams,
};
use crate::cache;
use crate::error::CliError;
use crate::output::{print_json, OutputOptions};
use crate::types::{Note, NoteSummary};
use chrono::{Duration, SecondsFormat, Utc};
use clap::{Args, Subcommand, ValueEnum};
use serde_json::json;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct NotesCommand {
    #[command(subcommand)]
    command: NotesSubcommand,
}

#[derive(Debug, Subcommand)]
enum NotesSubcommand {
    /// List accessible notes.
    List(ListNotesCommand),
    /// Retrieve one note by ID.
    Get(GetNoteCommand),
    /// Fetch full note records for IDs or list filters.
    Hydrate(HydrateNotesCommand),
    /// Search locally synced notes.
    Search(SearchNotesCommand),
    /// Open a note in the browser.
    Open(OpenNoteCommand),
}

#[derive(Debug, Args)]
struct ListNotesCommand {
    /// Return notes created before this date or date-time.
    #[arg(long)]
    created_before: Option<String>,
    /// Return notes created after this date or date-time.
    #[arg(long, conflicts_with = "since")]
    created_after: Option<String>,
    /// Return notes created within a relative duration, e.g. 7d, 24h, 30m.
    #[arg(long, conflicts_with = "created_after")]
    since: Option<String>,
    /// Return notes updated after this date or date-time.
    #[arg(long, conflicts_with = "updated_since")]
    updated_after: Option<String>,
    /// Return notes updated within a relative duration, e.g. 7d, 24h, 30m.
    #[arg(long, conflicts_with = "updated_after")]
    updated_since: Option<String>,
    /// Return notes in this folder and child folders.
    #[arg(long)]
    folder_id: Option<String>,
    /// Cursor to continue from.
    #[arg(long)]
    cursor: Option<String>,
    /// Page size, capped by the Granola API at 30.
    #[arg(long, default_value_t = 10)]
    page_size: u8,
    /// Fetch all pages.
    #[arg(long)]
    all: bool,
    /// Maximum number of notes to return.
    #[arg(long)]
    limit: Option<usize>,
    /// Sort returned notes by this field.
    #[arg(long, value_enum)]
    sort: Option<NoteSortField>,
    /// Sort order.
    #[arg(long, value_enum, default_value_t = SortOrder::Desc)]
    order: SortOrder,
    /// Do not truncate long table fields.
    #[arg(long)]
    no_truncate: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum NoteSortField {
    CreatedAt,
    UpdatedAt,
    Title,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SortOrder {
    Asc,
    Desc,
}

#[derive(Debug, Args)]
struct GetNoteCommand {
    /// Granola note ID, e.g. not_1d3tmYTlCICgjy.
    note_id: String,
    /// Include the transcript in the response.
    #[arg(long, value_parser = ["transcript"])]
    include: Option<String>,
}

#[derive(Debug, Args)]
struct HydrateNotesCommand {
    /// Note IDs to fetch. If omitted, list filters select notes to hydrate.
    note_ids: Vec<String>,
    /// Read one note ID per line from this file.
    #[arg(long)]
    ids_file: Option<PathBuf>,
    /// Read one note ID per line from stdin.
    #[arg(long)]
    stdin: bool,
    /// Include transcripts in hydrated notes.
    #[arg(long)]
    include_transcript: bool,
    /// Emit newline-delimited JSON, one note per line.
    #[arg(long)]
    jsonl: bool,
    /// Return notes created before this date or date-time when selecting by filters.
    #[arg(long)]
    created_before: Option<String>,
    /// Return notes created after this date or date-time when selecting by filters.
    #[arg(long, conflicts_with = "since")]
    created_after: Option<String>,
    /// Return notes created within a relative duration, e.g. 7d, 24h, 30m.
    #[arg(long, conflicts_with = "created_after")]
    since: Option<String>,
    /// Return notes updated after this date or date-time when selecting by filters.
    #[arg(long, conflicts_with = "updated_since")]
    updated_after: Option<String>,
    /// Return notes updated within a relative duration, e.g. 7d, 24h, 30m.
    #[arg(long, conflicts_with = "updated_after")]
    updated_since: Option<String>,
    /// Return notes in this folder and child folders when selecting by filters.
    #[arg(long)]
    folder_id: Option<String>,
    /// Cursor to continue from when selecting by filters.
    #[arg(long)]
    cursor: Option<String>,
    /// Page size, capped by the Granola API at 30.
    #[arg(long, default_value_t = 10)]
    page_size: u8,
    /// Fetch all pages when selecting by filters.
    #[arg(long)]
    all: bool,
    /// Maximum number of notes to hydrate.
    #[arg(long)]
    limit: Option<usize>,
}

#[derive(Debug, Args)]
#[command(
    long_about = "Search the local SQLite FTS index. Supports SQLite FTS5 syntax such as field filters and phrases. Examples: `granola notes search apple`, `granola notes search attendees:will async config`, `granola notes search attendees:will \"async config\"`, `granola notes search transcript:renewal`."
)]
struct SearchNotesCommand {
    /// Read additional search query text from stdin. If QUERY is omitted and stdin is piped, stdin is read automatically.
    #[arg(long)]
    stdin: bool,
    /// Search query. Multiple arguments are joined, so `granola notes search attendees:will "async config"` works without quoting the entire query.
    #[arg(value_name = "QUERY", num_args = 0..)]
    query: Vec<String>,
    /// Maximum number of cached notes to return.
    #[arg(long)]
    limit: Option<usize>,
}

#[derive(Debug, Args)]
struct OpenNoteCommand {
    /// Granola note ID, e.g. not_1d3tmYTlCICgjy.
    note_id: String,
    /// Print the note URL instead of opening it.
    #[arg(long)]
    print: bool,
}

pub async fn handle(
    command: NotesCommand,
    api_key_override: Option<String>,
    write_through_cache: bool,
    output: &OutputOptions,
) -> Result<(), CliError> {
    match command.command {
        NotesSubcommand::Search(command) => search_notes(command, api_key_override, output).await,
        command => {
            let api_key = resolve_api_key(api_key_override)?;
            let client = GranolaClient::new(api_key)?;
            match command {
                NotesSubcommand::List(command) => {
                    list_notes(&client, command, write_through_cache, output).await
                }
                NotesSubcommand::Get(command) => {
                    get_note(&client, command, write_through_cache, output).await
                }
                NotesSubcommand::Hydrate(command) => {
                    hydrate_notes(&client, command, write_through_cache, output).await
                }
                NotesSubcommand::Open(command) => {
                    open_note(&client, command, write_through_cache, output).await
                }
                NotesSubcommand::Search(_) => unreachable!(),
            }
        }
    }
}

async fn list_notes(
    client: &GranolaClient,
    command: ListNotesCommand,
    write_through_cache: bool,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let page_size = validate_page_size(command.page_size)?;
    let created_after = command
        .since
        .as_deref()
        .map(relative_time_after)
        .transpose()?
        .or(command.created_after);
    let updated_after = command
        .updated_since
        .as_deref()
        .map(relative_time_after)
        .transpose()?
        .or(command.updated_after);
    let mut params = ListNotesParams {
        created_before: command.created_before,
        created_after,
        updated_after,
        folder_id: command.folder_id,
        cursor: command.cursor,
        page_size: Some(page_size),
    };
    let mut notes = Vec::new();

    let (has_more, next_cursor) = loop {
        let response = client.list_notes(&params).await?;
        let page_meta = (response.has_more, response.cursor.clone());
        for note in response.notes {
            if command.limit.is_some_and(|limit| notes.len() >= limit) {
                break;
            }
            notes.push(note);
        }

        if command.limit.is_some_and(|limit| notes.len() >= limit) {
            break page_meta;
        }
        if !command.all || !response.has_more {
            break page_meta;
        }
        let Some(cursor) = response.cursor else {
            break page_meta;
        };
        params.cursor = Some(cursor);
    };

    sort_notes(&mut notes, command.sort, command.order);
    cache_summaries_if_enabled(&notes, write_through_cache)?;

    if output.is_json() {
        return print_json(
            &json!({
                "notes": notes,
                "count": notes.len(),
                "has_more": has_more,
                "cursor": next_cursor,
                "page_size": page_size,
            }),
            output,
        );
    }

    print_note_table(&notes, command.no_truncate);
    Ok(())
}

async fn search_notes(
    command: SearchNotesCommand,
    api_key_override: Option<String>,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let query = super::resolve_search_query(&command.query, command.stdin)?;
    let status = cache::status()?;
    let hits = cache::search(&query, command.limit)?.ok_or_else(|| {
        CliError::invalid_input(
            "no local cache found; run `granola sync --since 30d --all` before searching",
        )
    })?;
    let count = hits.len();
    let warning = search_cache_warning(&status, api_key_override).await?;

    if output.is_json() {
        return print_json(
            &json!({
                "query": query,
                "results": hits,
                "count": count,
                "warning": warning,
                "cache_synced_at": status.synced_at,
                "cache": {
                    "path": status.path,
                    "synced_at": status.synced_at,
                    "summaries": status.summaries,
                    "hydrated_notes": status.hydrated_notes,
                    "transcript_notes": status.transcript_notes,
                    "has_unhydrated_summaries": status.summaries > status.hydrated_notes,
                },
            }),
            output,
        );
    }

    print_search_result_table(&hits, &query, &status);
    if let Some(warning) =
        warning.filter(|_| count == 0 || status.summaries > status.hydrated_notes)
    {
        eprintln!("hint: {warning}");
    }
    Ok(())
}

async fn hydrate_notes(
    client: &GranolaClient,
    command: HydrateNotesCommand,
    write_through_cache: bool,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let ids = hydrate_note_ids(client, &command).await?;
    let mut notes = Vec::with_capacity(ids.len());

    for id in ids {
        notes.push(client.get_note(&id, command.include_transcript).await?);
    }
    cache_notes_if_enabled(&notes, write_through_cache)?;

    if command.jsonl {
        for note in &notes {
            println!("{}", serde_json::to_string(note).map_err(CliError::from)?);
        }
        return Ok(());
    }

    if output.is_json() {
        return print_json(&json!({ "notes": notes, "count": notes.len() }), output);
    }

    print_hydrated_note_table(&notes);
    Ok(())
}

async fn hydrate_note_ids(
    client: &GranolaClient,
    command: &HydrateNotesCommand,
) -> Result<Vec<String>, CliError> {
    let mut ids = command.note_ids.clone();

    if let Some(path) = &command.ids_file {
        let content = fs::read_to_string(path).map_err(|error| {
            CliError::general(format!(
                "failed to read IDs from {}: {error}",
                path.display()
            ))
        })?;
        ids.extend(parse_ids(&content));
    }

    if command.stdin {
        let mut content = String::new();
        io::stdin().read_to_string(&mut content).map_err(|error| {
            CliError::general(format!("failed to read IDs from stdin: {error}"))
        })?;
        ids.extend(parse_ids(&content));
    }

    ids.sort();
    ids.dedup();

    if !ids.is_empty() {
        if command_has_selection_filters(command) {
            return Err(CliError::invalid_input(
                "pass note IDs or list-selection filters, not both",
            ));
        }
        if let Some(limit) = command.limit {
            ids.truncate(limit);
        }
        return Ok(ids);
    }

    collect_hydrate_selection_ids(client, command).await
}

async fn collect_hydrate_selection_ids(
    client: &GranolaClient,
    command: &HydrateNotesCommand,
) -> Result<Vec<String>, CliError> {
    let page_size = validate_page_size(command.page_size)?;
    let created_after = command
        .since
        .as_deref()
        .map(relative_time_after)
        .transpose()?
        .or_else(|| command.created_after.clone());
    let updated_after = command
        .updated_since
        .as_deref()
        .map(relative_time_after)
        .transpose()?
        .or_else(|| command.updated_after.clone());
    let mut params = ListNotesParams {
        created_before: command.created_before.clone(),
        created_after,
        updated_after,
        folder_id: command.folder_id.clone(),
        cursor: command.cursor.clone(),
        page_size: Some(page_size),
    };
    let mut ids = Vec::new();

    loop {
        let response = client.list_notes(&params).await?;
        for note in response.notes {
            if command.limit.is_some_and(|limit| ids.len() >= limit) {
                break;
            }
            ids.push(note.id);
        }

        if command.limit.is_some_and(|limit| ids.len() >= limit)
            || !command.all
            || !response.has_more
        {
            break;
        }
        let Some(cursor) = response.cursor else { break };
        params.cursor = Some(cursor);
    }

    Ok(ids)
}

fn parse_ids(content: &str) -> impl Iterator<Item = String> + '_ {
    content.lines().filter_map(|line| {
        let id = line.trim();
        (!id.is_empty() && !id.starts_with('#')).then(|| id.to_string())
    })
}

fn command_has_selection_filters(command: &HydrateNotesCommand) -> bool {
    command.created_before.is_some()
        || command.created_after.is_some()
        || command.since.is_some()
        || command.updated_after.is_some()
        || command.updated_since.is_some()
        || command.folder_id.is_some()
        || command.cursor.is_some()
        || command.all
}

async fn get_note(
    client: &GranolaClient,
    command: GetNoteCommand,
    write_through_cache: bool,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let include_transcript = command.include.as_deref() == Some("transcript");
    let note = client
        .get_note(&command.note_id, include_transcript)
        .await?;
    cache_notes_if_enabled(std::slice::from_ref(&note), write_through_cache)?;

    if output.is_json() {
        return print_json(&note, output);
    }

    print_note_detail(&note);
    Ok(())
}

async fn open_note(
    client: &GranolaClient,
    command: OpenNoteCommand,
    write_through_cache: bool,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let note = client.get_note(&command.note_id, false).await?;
    cache_notes_if_enabled(std::slice::from_ref(&note), write_through_cache)?;

    if command.print {
        if output.is_json() {
            return print_json(
                &json!({ "id": note.id, "url": note.web_url, "opened": false }),
                output,
            );
        }
        println!("{}", note.web_url);
        return Ok(());
    }

    open::that(&note.web_url)
        .map_err(|error| CliError::general(format!("failed to open note URL: {error}")))?;

    if output.is_json() {
        return print_json(
            &json!({ "id": note.id, "url": note.web_url, "opened": true }),
            output,
        );
    }
    if !output.quiet {
        println!("opened {}", note.web_url);
    }
    Ok(())
}

fn cache_summaries_if_enabled(
    summaries: &[NoteSummary],
    write_through_cache: bool,
) -> Result<(), CliError> {
    if !write_through_cache || summaries.is_empty() {
        return Ok(());
    }

    cache::upsert_summaries(summaries)
}

fn cache_notes_if_enabled(notes: &[Note], write_through_cache: bool) -> Result<(), CliError> {
    if !write_through_cache || notes.is_empty() {
        return Ok(());
    }

    cache::upsert_notes(notes)
}

async fn search_cache_warning(
    status: &cache::CacheStatus,
    api_key_override: Option<String>,
) -> Result<Option<String>, CliError> {
    if status.summaries > status.hydrated_notes {
        return Ok(Some(format!(
            "local cache has {} listed note(s) but only {} hydrated note(s); run `granola sync --since 30d --all --include-transcripts` to search full summaries and transcripts",
            status.summaries,
            status.hydrated_notes
        )));
    }

    let Some(api_key) = try_resolve_api_key(api_key_override).ok().flatten() else {
        return Ok(None);
    };
    let Ok(client) = GranolaClient::new(api_key) else {
        return Ok(None);
    };
    let Ok(response) = client
        .list_notes(&ListNotesParams {
            page_size: Some(30),
            ..Default::default()
        })
        .await
    else {
        return Ok(None);
    };

    let has_remote_uncached_updates = response
        .notes
        .iter()
        .any(|summary| !matches!(cache::contains_fresh_summary(summary), Ok(true)));

    Ok(has_remote_uncached_updates.then(|| {
        "newer or unlisted notes may not be in the local cache; run `granola sync --since 30d --all` to refresh before relying on search results".to_string()
    }))
}

struct NoteRow {
    id: String,
    title: String,
    owner: String,
    created_at: String,
    updated_at: String,
}

fn print_note_table(notes: &[NoteSummary], no_truncate: bool) {
    let rows: Vec<NoteRow> = notes
        .iter()
        .map(|note| NoteRow {
            id: note.id.clone(),
            title: display_title(note, no_truncate),
            owner: note.owner.email.clone(),
            created_at: note.created_at.clone(),
            updated_at: note.updated_at.clone(),
        })
        .collect();

    if rows.is_empty() {
        println!("No notes found");
    } else {
        print_rows(
            &["id", "title", "owner", "created_at", "updated_at"],
            rows.into_iter()
                .map(|row| vec![row.id, row.title, row.owner, row.created_at, row.updated_at])
                .collect(),
        );
    }
}

fn print_hydrated_note_table(notes: &[Note]) {
    if notes.is_empty() {
        println!("No notes found");
        return;
    }

    print_rows(
        &["id", "title", "owner", "created_at", "updated_at"],
        notes
            .iter()
            .map(|note| {
                vec![
                    note.id.clone(),
                    note.title.as_deref().unwrap_or("").to_string(),
                    note.owner.email.clone(),
                    note.created_at.clone(),
                    note.updated_at.clone(),
                ]
            })
            .collect(),
    );
}

fn print_search_result_table(
    hits: &[cache::CacheSearchHit],
    query: &str,
    status: &cache::CacheStatus,
) {
    if hits.is_empty() {
        print_no_search_hits(query, status);
        return;
    }

    print_rows(
        &["id", "title", "owner", "updated_at", "cached"],
        hits.iter()
            .map(|hit| {
                vec![
                    hit.summary.id.clone(),
                    hit.summary.title.as_deref().unwrap_or("").to_string(),
                    hit.summary.owner.email.clone(),
                    hit.summary.updated_at.clone(),
                    cached_label(hit),
                ]
            })
            .collect(),
    );
}

fn print_no_search_hits(query: &str, status: &cache::CacheStatus) {
    if status.summaries == 0 {
        println!("No notes are cached yet. Run `granola sync --since 30d --all` to populate the local search index.");
        return;
    }

    println!("No cached notes matched '{query}'.");
    println!(
        "Searched {} cached summary note(s), {} hydrated note(s), and {} transcript-indexed note(s).",
        status.summaries, status.hydrated_notes, status.transcript_notes
    );
}

fn cached_label(hit: &cache::CacheSearchHit) -> String {
    if hit.cached_transcript {
        "transcript".to_string()
    } else if hit.cached_detail {
        "full".to_string()
    } else {
        "summary".to_string()
    }
}

fn print_rows(headers: &[&str], rows: Vec<Vec<String>>) {
    let mut widths: Vec<usize> = headers.iter().map(|header| header.len()).collect();
    for row in &rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(cell.len());
        }
    }

    print_table_line(&widths);
    print_table_row(headers.iter().copied(), &widths);
    print_table_line(&widths);
    for row in &rows {
        print_table_row(row.iter().map(String::as_str), &widths);
    }
    print_table_line(&widths);
}

fn print_table_line(widths: &[usize]) {
    print!("+");
    for width in widths {
        print!("-{:-<width$}-+", "", width = width);
    }
    println!();
}

fn print_table_row<'a>(cells: impl IntoIterator<Item = &'a str>, widths: &[usize]) {
    print!("|");
    for (cell, width) in cells.into_iter().zip(widths) {
        print!(" {cell:<width$} |", width = width);
    }
    println!();
}

fn display_title(note: &NoteSummary, no_truncate: bool) -> String {
    let title = note.title.as_deref().unwrap_or("");
    if no_truncate || title.chars().count() <= 80 {
        return title.to_string();
    }

    let mut truncated: String = title.chars().take(79).collect();
    truncated.push('…');
    truncated
}

fn sort_notes(notes: &mut [NoteSummary], sort: Option<NoteSortField>, order: SortOrder) {
    let Some(sort) = sort else { return };
    notes.sort_by(|left, right| {
        let ord = match sort {
            NoteSortField::CreatedAt => left.created_at.cmp(&right.created_at),
            NoteSortField::UpdatedAt => left.updated_at.cmp(&right.updated_at),
            NoteSortField::Title => left
                .title
                .as_deref()
                .unwrap_or("")
                .to_lowercase()
                .cmp(&right.title.as_deref().unwrap_or("").to_lowercase()),
        };
        match order {
            SortOrder::Asc => ord,
            SortOrder::Desc => ord.reverse(),
        }
    });
}

fn relative_time_after(input: &str) -> Result<String, CliError> {
    let input = input.trim();
    if input.len() < 2 {
        return Err(relative_duration_error(input));
    }

    let (amount, unit) = input.split_at(input.len() - 1);
    let amount: i64 = amount.parse().map_err(|_| relative_duration_error(input))?;
    if amount <= 0 {
        return Err(relative_duration_error(input));
    }

    let duration = match unit {
        "d" => Duration::days(amount),
        "h" => Duration::hours(amount),
        "m" => Duration::minutes(amount),
        _ => return Err(relative_duration_error(input)),
    };

    Ok((Utc::now() - duration).to_rfc3339_opts(SecondsFormat::Secs, true))
}

fn relative_duration_error(input: &str) -> CliError {
    CliError::invalid_input(format!(
        "invalid relative duration '{input}'; use a positive value ending in d, h, or m (for example 7d, 24h, 30m)"
    ))
}

fn print_note_detail(note: &Note) {
    println!("{}", note.title.as_deref().unwrap_or("Untitled note"));
    println!("id: {}", note.id);
    println!("owner: {}", note.owner.email);
    println!("created: {}", note.created_at);
    println!("updated: {}", note.updated_at);
    println!("url: {}", note.web_url);
    println!();

    if let Some(markdown) = note.summary_markdown.as_deref() {
        println!("{markdown}");
    } else {
        println!("{}", note.summary_text);
    }

    if let Some(transcript) = &note.transcript {
        println!();
        println!("Transcript items: {}", transcript.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::User;

    #[test]
    fn parses_relative_duration_syntax() {
        assert!(relative_time_after("7d").is_ok());
        assert!(relative_time_after("24h").is_ok());
        assert!(relative_time_after("30m").is_ok());
        assert!(relative_time_after("0d").is_err());
        assert!(relative_time_after("7w").is_err());
    }

    #[test]
    fn sorts_notes_by_title_descending() {
        let mut notes = vec![
            note_summary("not_A", "Alpha", "2026-01-01"),
            note_summary("not_B", "Beta", "2026-01-02"),
        ];
        sort_notes(&mut notes, Some(NoteSortField::Title), SortOrder::Desc);
        assert_eq!(notes[0].id, "not_B");
    }

    #[test]
    fn truncates_long_table_titles_by_default() {
        let note = note_summary("not_A", &"a".repeat(100), "2026-01-01");
        assert_eq!(display_title(&note, false).chars().count(), 80);
        assert_eq!(display_title(&note, true).chars().count(), 100);
    }

    fn note_summary(id: &str, title: &str, created_at: &str) -> NoteSummary {
        NoteSummary {
            id: id.to_string(),
            object: "note".to_string(),
            title: Some(title.to_string()),
            owner: User {
                name: None,
                email: "owner@example.com".to_string(),
            },
            created_at: created_at.to_string(),
            updated_at: created_at.to_string(),
        }
    }
}
