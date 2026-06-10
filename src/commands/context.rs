use crate::api::{resolve_api_key, validate_page_size, GranolaClient, ListNotesParams};
use crate::error::CliError;
use crate::note_ref::normalize_note_id;
use crate::output::{print_json, OutputOptions};
use crate::types::{Note, TranscriptItem};
use chrono::{Duration, SecondsFormat, Utc};
use clap::{Args, ValueEnum};
use serde::Serialize;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

#[derive(Debug, Args)]
pub struct ContextCommand {
    /// Note IDs or copied Granola URLs. If omitted, list filters select notes.
    note_ids: Vec<String>,
    /// Read one note ID or URL per line from this file.
    #[arg(long)]
    ids_file: Option<PathBuf>,
    /// Read one note ID or URL per line from stdin.
    #[arg(long)]
    stdin: bool,
    /// Include transcripts in the context bundle.
    #[arg(long)]
    include_transcript: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = ContextFormat::Markdown)]
    format: ContextFormat,
    /// Maximum UTF-8 bytes to print for markdown output.
    #[arg(long)]
    max_bytes: Option<usize>,
    /// Write bundle to this file instead of stdout.
    #[arg(short, long)]
    output_file: Option<PathBuf>,
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
    /// Maximum number of notes to bundle.
    #[arg(long)]
    limit: Option<usize>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ContextFormat {
    Markdown,
    Json,
}

#[derive(Debug, Serialize)]
struct ContextEnvelope<'a> {
    count: usize,
    included_transcripts: bool,
    notes: &'a [Note],
}

pub async fn handle(
    command: ContextCommand,
    api_key_override: Option<String>,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let api_key = resolve_api_key(api_key_override)?;
    let client = GranolaClient::new(api_key)?;
    let ids = resolve_note_ids(&client, &command).await?;
    let mut notes = Vec::with_capacity(ids.len());
    for id in ids {
        notes.push(client.get_note(&id, command.include_transcript).await?);
    }

    if output.is_json() || matches!(command.format, ContextFormat::Json) {
        let envelope = ContextEnvelope {
            count: notes.len(),
            included_transcripts: command.include_transcript,
            notes: &notes,
        };
        if let Some(path) = &command.output_file {
            let content = serde_json::to_string_pretty(&envelope).map_err(CliError::from)?;
            write_file(path, &content)?;
            return Ok(());
        }
        return print_json(&envelope, output);
    }

    let mut content = render_context_markdown(&notes, command.include_transcript);
    if let Some(max_bytes) = command.max_bytes {
        content = truncate_utf8(&content, max_bytes);
    }
    if let Some(path) = &command.output_file {
        write_file(path, &content)?;
        return Ok(());
    }
    print!("{content}");
    Ok(())
}

async fn resolve_note_ids(
    client: &GranolaClient,
    command: &ContextCommand,
) -> Result<Vec<String>, CliError> {
    let mut ids = normalize_note_refs(&command.note_ids)?;
    if let Some(path) = &command.ids_file {
        let content = fs::read_to_string(path).map_err(|error| {
            CliError::general(format!(
                "failed to read IDs from {}: {error}",
                path.display()
            ))
        })?;
        ids.extend(normalize_lines(&content)?);
    }
    if command.stdin {
        let mut content = String::new();
        io::stdin().read_to_string(&mut content).map_err(|error| {
            CliError::general(format!("failed to read IDs from stdin: {error}"))
        })?;
        ids.extend(normalize_lines(&content)?);
    }
    ids.sort();
    ids.dedup();

    if !ids.is_empty() {
        if command_has_selection_filters(command) {
            return Err(CliError::invalid_input(
                "pass note IDs/URLs or list-selection filters, not both",
            ));
        }
        if let Some(limit) = command.limit {
            ids.truncate(limit);
        }
        return Ok(ids);
    }

    collect_selection_ids(client, command).await
}

async fn collect_selection_ids(
    client: &GranolaClient,
    command: &ContextCommand,
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

fn render_context_markdown(notes: &[Note], include_transcript: bool) -> String {
    let mut output = String::new();
    output.push_str("# Granola context bundle\n\n");
    output.push_str(&format!("Notes: {}\n\n", notes.len()));
    for note in notes {
        output.push_str("---\n\n");
        output.push_str(&format!(
            "## {}\n\n",
            note.title.as_deref().unwrap_or("Untitled note")
        ));
        output.push_str(&format!("id: {}\n", note.id));
        output.push_str(&format!("owner: {}\n", note.owner.email));
        output.push_str(&format!("created_at: {}\n", note.created_at));
        output.push_str(&format!("updated_at: {}\n", note.updated_at));
        output.push_str(&format!("url: {}\n\n", note.web_url));
        output.push_str("### Summary\n\n");
        output.push_str(
            note.summary_markdown
                .as_deref()
                .unwrap_or(&note.summary_text),
        );
        output.push_str("\n\n");
        if include_transcript {
            output.push_str("### Transcript\n\n");
            output.push_str(&render_transcript(
                note.transcript.as_deref().unwrap_or(&[]),
            ));
            output.push('\n');
        }
    }
    output
}

fn render_transcript(items: &[TranscriptItem]) -> String {
    items
        .iter()
        .map(|item| {
            let speaker = item
                .speaker
                .diarization_label
                .as_deref()
                .unwrap_or(&item.speaker.source);
            format!(
                "- [{start}–{end}] {speaker}: {text}",
                start = item.start_time,
                end = item.end_time,
                text = item.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn truncate_utf8(content: &str, max_bytes: usize) -> String {
    if content.len() <= max_bytes {
        return content.to_string();
    }
    let mut end = max_bytes;
    while !content.is_char_boundary(end) {
        end -= 1;
    }
    let mut truncated = content[..end].to_string();
    truncated.push_str("\n\n<!-- granola context truncated by --max-bytes -->\n");
    truncated
}

fn write_file(path: &Path, content: &str) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            CliError::general(format!(
                "failed to create output directory {}: {error}",
                parent.display()
            ))
        })?;
    }
    fs::write(path, content)
        .map_err(|error| CliError::general(format!("failed to write {}: {error}", path.display())))
}

fn normalize_note_refs(inputs: &[String]) -> Result<Vec<String>, CliError> {
    inputs
        .iter()
        .map(|input| normalize_note_id(input))
        .collect()
}

fn normalize_lines(content: &str) -> Result<Vec<String>, CliError> {
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            (!line.is_empty() && !line.starts_with('#')).then_some(line)
        })
        .map(normalize_note_id)
        .collect()
}

fn command_has_selection_filters(command: &ContextCommand) -> bool {
    command.created_before.is_some()
        || command.created_after.is_some()
        || command.since.is_some()
        || command.updated_after.is_some()
        || command.updated_since.is_some()
        || command.folder_id.is_some()
        || command.cursor.is_some()
        || command.all
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
