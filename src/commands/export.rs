use crate::api::{resolve_api_key, validate_page_size, GranolaClient, ListNotesParams};
use crate::error::CliError;
use crate::output::{print_json, OutputOptions};
use crate::types::{Note, NoteSummary, TranscriptItem};
use chrono::{Duration, SecondsFormat, Utc};
use clap::{Args, Subcommand, ValueEnum};
use serde_json::json;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Args)]
pub struct ExportCommand {
    #[command(subcommand)]
    command: ExportSubcommand,
}

#[derive(Debug, Subcommand)]
enum ExportSubcommand {
    /// Export a single note.
    Note(ExportNoteCommand),
    /// Export multiple notes selected by the list filters.
    Notes(ExportNotesCommand),
}

#[derive(Debug, Args)]
struct ExportNoteCommand {
    /// Granola note ID, e.g. not_1d3tmYTlCICgjy.
    note_id: String,
    /// Export format.
    #[arg(long, value_enum, default_value_t = NoteExportFormat::Markdown)]
    format: NoteExportFormat,
    /// Include the transcript when exporting markdown or JSON.
    #[arg(long)]
    include_transcript: bool,
    /// Write to this file instead of stdout.
    #[arg(short, long)]
    output_file: Option<PathBuf>,
    /// Overwrite an existing output file.
    #[arg(long)]
    force: bool,
    /// Skip writing if the output file already exists.
    #[arg(long, conflicts_with = "force")]
    skip_existing: bool,
}

#[derive(Debug, Args)]
struct ExportNotesCommand {
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
    /// Maximum number of notes to export.
    #[arg(long)]
    limit: Option<usize>,
    /// Sort selected notes by this field before exporting.
    #[arg(long, value_enum)]
    sort: Option<NoteSortField>,
    /// Sort order.
    #[arg(long, value_enum, default_value_t = SortOrder::Desc)]
    order: SortOrder,
    /// Export format.
    #[arg(long, value_enum, default_value_t = NotesExportFormat::Jsonl)]
    format: NotesExportFormat,
    /// Write to this file instead of stdout.
    #[arg(short, long, conflicts_with = "output_dir")]
    output_file: Option<PathBuf>,
    /// Write one file per note into this directory. Supports markdown and json formats.
    #[arg(long, conflicts_with = "output_file")]
    output_dir: Option<PathBuf>,
    /// Include transcripts when writing one file per note with markdown or JSON output.
    #[arg(long)]
    include_transcript: bool,
    /// Overwrite an existing output file.
    #[arg(long)]
    force: bool,
    /// Skip files that already exist.
    #[arg(long, conflicts_with = "force")]
    skip_existing: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum NoteExportFormat {
    Markdown,
    Json,
    Txt,
    Transcript,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum NotesExportFormat {
    Jsonl,
    Markdown,
    Json,
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

pub async fn handle(
    command: ExportCommand,
    api_key_override: Option<String>,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let api_key = resolve_api_key(api_key_override)?;
    let client = GranolaClient::new(api_key)?;

    match command.command {
        ExportSubcommand::Note(command) => export_note(&client, command, output).await,
        ExportSubcommand::Notes(command) => export_notes(&client, command, output).await,
    }
}

async fn export_note(
    client: &GranolaClient,
    command: ExportNoteCommand,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let include_transcript = command.include_transcript
        || matches!(
            command.format,
            NoteExportFormat::Transcript | NoteExportFormat::Json
        );
    let note = client
        .get_note(&command.note_id, include_transcript)
        .await?;
    let content = match command.format {
        NoteExportFormat::Markdown => render_note_markdown(&note, command.include_transcript),
        NoteExportFormat::Json => serde_json::to_string_pretty(&note).map_err(CliError::from)?,
        NoteExportFormat::Txt => render_note_text(&note),
        NoteExportFormat::Transcript => {
            render_transcript(note.transcript.as_deref().unwrap_or(&[]))
        }
    };

    write_or_print(
        content,
        command.output_file.as_deref(),
        WriteMode::from_flags(command.force, command.skip_existing),
        output,
    )
}

async fn export_notes(
    client: &GranolaClient,
    command: ExportNotesCommand,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let notes = collect_note_summaries(client, &command).await?;
    if let Some(output_dir) = command.output_dir.as_deref() {
        return export_notes_to_dir(client, &notes, output_dir, &command, output).await;
    }

    let content = match command.format {
        NotesExportFormat::Jsonl => render_notes_jsonl(&notes)?,
        NotesExportFormat::Json => serde_json::to_string_pretty(&notes).map_err(CliError::from)?,
        NotesExportFormat::Markdown => render_notes_markdown(&notes),
    };

    write_or_print(
        content,
        command.output_file.as_deref(),
        WriteMode::from_flags(command.force, command.skip_existing),
        output,
    )
}

async fn export_notes_to_dir(
    client: &GranolaClient,
    notes: &[NoteSummary],
    output_dir: &Path,
    command: &ExportNotesCommand,
    output: &OutputOptions,
) -> Result<(), CliError> {
    if matches!(command.format, NotesExportFormat::Jsonl) {
        return Err(CliError::invalid_input(
            "--output-dir is not supported with --format jsonl; use --format markdown or --format json",
        ));
    }

    let mut written = 0usize;
    let mut skipped = 0usize;
    let mode = WriteMode::from_flags(command.force, command.skip_existing);

    for summary in notes {
        let note = client
            .get_note(&summary.id, command.include_transcript)
            .await?;
        let extension = match command.format {
            NotesExportFormat::Markdown => "md",
            NotesExportFormat::Json => "json",
            NotesExportFormat::Jsonl => unreachable!("jsonl rejected above"),
        };
        let path = output_dir.join(safe_note_filename(&note, extension));
        let content = match command.format {
            NotesExportFormat::Markdown => render_note_markdown(&note, command.include_transcript),
            NotesExportFormat::Json => {
                serde_json::to_string_pretty(&note).map_err(CliError::from)?
            }
            NotesExportFormat::Jsonl => unreachable!("jsonl rejected above"),
        };

        if atomic_write(&path, &content, mode)? {
            written += 1;
        } else {
            skipped += 1;
        }
    }

    let data = json!({
        "output_dir": output_dir,
        "written": written,
        "skipped": skipped,
    });
    if output.is_json() {
        return print_json(&data, output);
    }
    if !output.quiet {
        println!(
            "wrote {written} file(s) to {}; skipped {skipped}",
            output_dir.display()
        );
    }
    Ok(())
}

async fn collect_note_summaries(
    client: &GranolaClient,
    command: &ExportNotesCommand,
) -> Result<Vec<NoteSummary>, CliError> {
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
    let mut notes = Vec::new();

    loop {
        let response = client.list_notes(&params).await?;
        for note in response.notes {
            if command.limit.is_some_and(|limit| notes.len() >= limit) {
                break;
            }
            notes.push(note);
        }

        if command.limit.is_some_and(|limit| notes.len() >= limit) {
            break;
        }
        if !command.all || !response.has_more {
            break;
        }
        let Some(cursor) = response.cursor else {
            break;
        };
        params.cursor = Some(cursor);
    }

    sort_notes(&mut notes, command.sort, command.order);

    Ok(notes)
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

fn write_or_print(
    content: String,
    output_file: Option<&Path>,
    mode: WriteMode,
    output: &OutputOptions,
) -> Result<(), CliError> {
    if let Some(path) = output_file {
        let written = atomic_write(path, &content, mode)?;
        let data = json!({ "path": path, "written": written, "skipped": !written });
        if output.is_json() {
            return print_json(&data, output);
        }
        if !output.quiet {
            if written {
                println!("wrote {}", path.display());
            } else {
                println!("skipped existing {}", path.display());
            }
        }
        return Ok(());
    }

    print!("{content}");
    if !content.ends_with('\n') {
        println!();
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum WriteMode {
    CreateNew,
    Force,
    SkipExisting,
}

impl WriteMode {
    fn from_flags(force: bool, skip_existing: bool) -> Self {
        if force {
            Self::Force
        } else if skip_existing {
            Self::SkipExisting
        } else {
            Self::CreateNew
        }
    }
}

fn atomic_write(path: &Path, content: &str, mode: WriteMode) -> Result<bool, CliError> {
    if path.exists() {
        match mode {
            WriteMode::Force => {}
            WriteMode::SkipExisting => return Ok(false),
            WriteMode::CreateNew => {
                return Err(CliError::invalid_input(format!(
                    "refusing to overwrite {}; pass --force to replace it or --skip-existing to keep it",
                    path.display()
                )));
            }
        }
    }

    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| {
            CliError::general(format!(
                "failed to create output directory {}: {error}",
                parent.display()
            ))
        })?;
    }

    let temp_path = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("granola")
    ));
    {
        let mut file = fs::File::create(&temp_path).map_err(|error| {
            CliError::general(format!(
                "failed to create temporary file {}: {error}",
                temp_path.display()
            ))
        })?;
        file.write_all(content.as_bytes()).map_err(|error| {
            CliError::general(format!(
                "failed to write temporary file {}: {error}",
                temp_path.display()
            ))
        })?;
        file.sync_all().map_err(|error| {
            CliError::general(format!(
                "failed to flush temporary file {}: {error}",
                temp_path.display()
            ))
        })?;
    }
    fs::rename(&temp_path, path).map_err(|error| {
        let _ = fs::remove_file(&temp_path);
        CliError::general(format!(
            "failed to move temporary file into place at {}: {error}",
            path.display()
        ))
    })?;
    Ok(true)
}

fn render_note_markdown(note: &Note, include_transcript: bool) -> String {
    let mut out = String::new();
    out.push_str("# ");
    out.push_str(note.title.as_deref().unwrap_or("Untitled note"));
    out.push_str("\n\n");
    out.push_str(&format!("- ID: `{}`\n", note.id));
    out.push_str(&format!("- Owner: {}\n", note.owner.email));
    out.push_str(&format!("- Created: {}\n", note.created_at));
    out.push_str(&format!("- Updated: {}\n", note.updated_at));
    out.push_str(&format!("- URL: {}\n", note.web_url));
    out.push_str("\n## Summary\n\n");
    out.push_str(
        note.summary_markdown
            .as_deref()
            .unwrap_or(&note.summary_text),
    );
    out.push('\n');

    if include_transcript {
        out.push_str("\n## Transcript\n\n");
        out.push_str(&render_transcript(
            note.transcript.as_deref().unwrap_or(&[]),
        ));
    }

    out
}

fn render_note_text(note: &Note) -> String {
    format!(
        "{}\n\n{}\n",
        note.title.as_deref().unwrap_or("Untitled note"),
        note.summary_text
    )
}

fn render_transcript(transcript: &[TranscriptItem]) -> String {
    transcript
        .iter()
        .map(|item| {
            let speaker = item
                .speaker
                .diarization_label
                .as_deref()
                .unwrap_or(item.speaker.source.as_str());
            format!("[{speaker}] {}", item.text)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_notes_jsonl(notes: &[NoteSummary]) -> Result<String, CliError> {
    let mut out = String::new();
    for note in notes {
        out.push_str(&serde_json::to_string(note).map_err(CliError::from)?);
        out.push('\n');
    }
    Ok(out)
}

fn render_notes_markdown(notes: &[NoteSummary]) -> String {
    let mut out = String::from("# Granola Notes\n\n");
    for note in notes {
        out.push_str(&format!(
            "- `{}` — {} ({})\n",
            note.id,
            note.title.as_deref().unwrap_or("Untitled note"),
            note.created_at
        ));
    }
    out
}

fn safe_note_filename(note: &Note, extension: &str) -> String {
    let date = note.created_at.split('T').next().unwrap_or("unknown-date");
    let title = note.title.as_deref().unwrap_or("untitled-note");
    let slug = slugify(title);
    format!("{date}-{slug}-{}.{}", note.id, extension)
}

fn slugify(input: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;

    for ch in input.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            previous_dash = false;
        } else if !previous_dash && !slug.is_empty() {
            slug.push('-');
            previous_dash = true;
        }
    }

    while slug.ends_with('-') {
        slug.pop();
    }

    if slug.is_empty() {
        "untitled-note".to_string()
    } else {
        slug.chars().take(80).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_transcript_labels_when_available() {
        let note: Note = serde_json::from_str(include_str!(
            "../../tests/fixtures/get_note_with_transcript.json"
        ))
        .unwrap();
        let transcript = render_transcript(note.transcript.as_deref().unwrap());
        assert!(transcript.contains("[Speaker A]"));
        assert!(transcript.contains("[speaker]"));
    }

    #[test]
    fn jsonl_has_one_json_object_per_line() {
        let response: crate::types::ListNotesResponse =
            serde_json::from_str(include_str!("../../tests/fixtures/list_notes.json")).unwrap();
        let jsonl = render_notes_jsonl(&response.notes).unwrap();
        assert_eq!(jsonl.lines().count(), 1);
        assert!(jsonl.starts_with('{'));
    }

    #[test]
    fn builds_safe_note_filenames() {
        let note: Note = serde_json::from_str(include_str!(
            "../../tests/fixtures/get_note_with_transcript.json"
        ))
        .unwrap();
        let filename = safe_note_filename(&note, "md");
        assert_eq!(filename, "2026-06-08-untitled-note-not_BBBBBBBBBBBBBB.md");
    }

    #[test]
    fn slugifies_titles_for_filenames() {
        assert_eq!(slugify("Hello, World! / Q2"), "hello-world-q2");
        assert_eq!(slugify("!!!"), "untitled-note");
    }
}
