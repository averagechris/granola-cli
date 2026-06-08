use crate::api::{resolve_api_key, validate_page_size, GranolaClient, ListNotesParams};
use crate::error::CliError;
use crate::output::{print_json, OutputOptions};
use crate::types::{Note, NoteSummary};
use chrono::{Duration, SecondsFormat, Utc};
use clap::{Args, Subcommand, ValueEnum};
use serde_json::json;
use tabled::{Table, Tabled};

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
    output: &OutputOptions,
) -> Result<(), CliError> {
    let api_key = resolve_api_key(api_key_override)?;
    let client = GranolaClient::new(api_key)?;

    match command.command {
        NotesSubcommand::List(command) => list_notes(&client, command, output).await,
        NotesSubcommand::Get(command) => get_note(&client, command, output).await,
        NotesSubcommand::Open(command) => open_note(&client, command, output).await,
    }
}

async fn list_notes(
    client: &GranolaClient,
    command: ListNotesCommand,
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
        let Some(cursor) = response.cursor else { break };
        params.cursor = Some(cursor);
    }

    sort_notes(&mut notes, command.sort, command.order);

    if output.is_json() {
        return print_json(&notes, output);
    }

    print_note_table(&notes, command.no_truncate);
    Ok(())
}

async fn get_note(
    client: &GranolaClient,
    command: GetNoteCommand,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let include_transcript = command.include.as_deref() == Some("transcript");
    let note = client
        .get_note(&command.note_id, include_transcript)
        .await?;

    if output.is_json() {
        return print_json(&note, output);
    }

    print_note_detail(&note);
    Ok(())
}

async fn open_note(
    client: &GranolaClient,
    command: OpenNoteCommand,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let note = client.get_note(&command.note_id, false).await?;

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

#[derive(Tabled)]
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
        println!("{}", Table::new(rows));
    }
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
