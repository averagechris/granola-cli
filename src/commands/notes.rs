use crate::api::{resolve_api_key, validate_page_size, GranolaClient, ListNotesParams};
use crate::error::CliError;
use crate::output::{print_json, OutputOptions};
use crate::types::{Note, NoteSummary};
use clap::{Args, Subcommand};
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
    #[arg(long)]
    created_after: Option<String>,
    /// Return notes updated after this date or date-time.
    #[arg(long)]
    updated_after: Option<String>,
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
    let mut params = ListNotesParams {
        created_before: command.created_before,
        created_after: command.created_after,
        updated_after: command.updated_after,
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

    if output.is_json() {
        return print_json(&notes, output);
    }

    print_note_table(&notes);
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
struct NoteRow<'a> {
    id: &'a str,
    title: &'a str,
    owner: &'a str,
    created_at: &'a str,
    updated_at: &'a str,
}

fn print_note_table(notes: &[NoteSummary]) {
    let rows: Vec<NoteRow<'_>> = notes
        .iter()
        .map(|note| NoteRow {
            id: &note.id,
            title: note.title.as_deref().unwrap_or(""),
            owner: &note.owner.email,
            created_at: &note.created_at,
            updated_at: &note.updated_at,
        })
        .collect();

    if rows.is_empty() {
        println!("No notes found");
    } else {
        println!("{}", Table::new(rows));
    }
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
