use crate::api::{resolve_api_key, try_resolve_api_key, GranolaClient, ListNotesParams};
use crate::cache;
use crate::error::CliError;
use crate::output::{print_json, OutputOptions};
use crate::types::{Note, NoteSummary};
use chrono::{Duration, SecondsFormat, Utc};
use clap::Args;
use serde_json::json;

#[derive(Debug, Args)]
pub struct LastCommand {
    /// Include the transcript in the response.
    #[arg(long)]
    pub include_transcript: bool,
}

pub async fn recent(
    api_key_override: Option<String>,
    write_through_cache: bool,
    output: &OutputOptions,
) -> Result<(), CliError> {
    list_shortcut(
        api_key_override,
        write_through_cache,
        output,
        Some(relative_time_after(Duration::days(7))),
        None,
    )
    .await
}

pub async fn today(
    api_key_override: Option<String>,
    write_through_cache: bool,
    output: &OutputOptions,
) -> Result<(), CliError> {
    list_shortcut(
        api_key_override,
        write_through_cache,
        output,
        Some(start_of_today()),
        None,
    )
    .await
}

pub async fn yesterday(
    api_key_override: Option<String>,
    write_through_cache: bool,
    output: &OutputOptions,
) -> Result<(), CliError> {
    list_shortcut(
        api_key_override,
        write_through_cache,
        output,
        Some(start_of_yesterday()),
        Some(start_of_today()),
    )
    .await
}

pub async fn last(
    command: LastCommand,
    api_key_override: Option<String>,
    write_through_cache: bool,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let api_key = resolve_api_key(api_key_override)?;
    let client = GranolaClient::new(api_key)?;
    let response = client
        .list_notes(&ListNotesParams {
            page_size: Some(30),
            ..Default::default()
        })
        .await?;
    let Some(summary) = response
        .notes
        .into_iter()
        .max_by(|left, right| left.updated_at.cmp(&right.updated_at))
    else {
        return Err(CliError::not_found("no notes found"));
    };
    let note = client
        .get_note(&summary.id, command.include_transcript)
        .await?;
    cache_notes_if_enabled(std::slice::from_ref(&note), write_through_cache)?;

    if output.is_json() {
        return print_json(&note, output);
    }

    print_note_detail(&note);
    Ok(())
}

pub async fn show(
    note_id: String,
    api_key_override: Option<String>,
    write_through_cache: bool,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let api_key = resolve_api_key(api_key_override)?;
    let client = GranolaClient::new(api_key)?;
    let note = client.get_note(&note_id, false).await?;
    cache_notes_if_enabled(std::slice::from_ref(&note), write_through_cache)?;
    if output.is_json() {
        return print_json(&note, output);
    }
    print_note_detail(&note);
    Ok(())
}

pub async fn open(
    note_id: String,
    api_key_override: Option<String>,
    write_through_cache: bool,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let api_key = resolve_api_key(api_key_override)?;
    let client = GranolaClient::new(api_key)?;
    let note = client.get_note(&note_id, false).await?;
    cache_notes_if_enabled(std::slice::from_ref(&note), write_through_cache)?;
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

pub async fn search(
    query: Vec<String>,
    stdin: bool,
    api_key_override: Option<String>,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let query = super::resolve_search_query(&query, stdin)?;
    let status = cache::status()?;
    let hits = cache::search(&query, None)?.ok_or_else(|| {
        CliError::invalid_input(
            "no local cache found; run `granola sync --since 30d --all` before searching",
        )
    })?;
    let warning = search_cache_warning(&status, api_key_override).await?;
    if output.is_json() {
        return print_json(
            &json!({
                "query": query,
                "results": hits,
                "count": hits.len(),
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
        warning.filter(|_| hits.is_empty() || status.summaries > status.hydrated_notes)
    {
        eprintln!("hint: {warning}");
    }
    Ok(())
}

async fn list_shortcut(
    api_key_override: Option<String>,
    write_through_cache: bool,
    output: &OutputOptions,
    created_after: Option<String>,
    created_before: Option<String>,
) -> Result<(), CliError> {
    let api_key = resolve_api_key(api_key_override)?;
    let client = GranolaClient::new(api_key)?;
    let response = client
        .list_notes(&ListNotesParams {
            created_after,
            created_before,
            page_size: Some(30),
            ..Default::default()
        })
        .await?;
    let mut notes = response.notes;
    notes.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    cache_summaries_if_enabled(&notes, write_through_cache)?;

    if output.is_json() {
        return print_json(
            &json!({
                "notes": notes,
                "count": notes.len(),
                "has_more": response.has_more,
                "cursor": response.cursor,
                "page_size": 30,
            }),
            output,
        );
    }

    print_note_table(&notes);
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

fn print_note_detail(note: &Note) {
    println!("{}", note.title.as_deref().unwrap_or("Untitled note"));
    println!("id: {}", note.id);
    println!("owner: {}", note.owner.email);
    println!("created: {}", note.created_at);
    println!("updated: {}", note.updated_at);
    println!("url: {}", note.web_url);
    println!();
    println!(
        "{}",
        note.summary_markdown
            .as_deref()
            .unwrap_or(&note.summary_text)
    );
}

fn print_note_table(notes: &[NoteSummary]) {
    if notes.is_empty() {
        println!("No notes found");
        return;
    }
    print_rows(
        &["id", "title", "owner", "updated_at"],
        notes
            .iter()
            .map(|note| {
                vec![
                    note.id.clone(),
                    note.title.as_deref().unwrap_or("").to_string(),
                    note.owner.email.clone(),
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

fn relative_time_after(duration: Duration) -> String {
    (Utc::now() - duration).to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn start_of_today() -> String {
    Utc::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .expect("midnight is valid")
        .and_utc()
        .to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn start_of_yesterday() -> String {
    (Utc::now().date_naive() - Duration::days(1))
        .and_hms_opt(0, 0, 0)
        .expect("midnight is valid")
        .and_utc()
        .to_rfc3339_opts(SecondsFormat::Secs, true)
}
