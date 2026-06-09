use crate::api::{resolve_api_key, GranolaClient, ListNotesParams};
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
    output: &OutputOptions,
) -> Result<(), CliError> {
    list_shortcut(
        api_key_override,
        output,
        Some(relative_time_after(Duration::days(7))),
        None,
    )
    .await
}

pub async fn today(
    api_key_override: Option<String>,
    output: &OutputOptions,
) -> Result<(), CliError> {
    list_shortcut(api_key_override, output, Some(start_of_today()), None).await
}

pub async fn yesterday(
    api_key_override: Option<String>,
    output: &OutputOptions,
) -> Result<(), CliError> {
    list_shortcut(
        api_key_override,
        output,
        Some(start_of_yesterday()),
        Some(start_of_today()),
    )
    .await
}

pub async fn last(
    command: LastCommand,
    api_key_override: Option<String>,
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

    if output.is_json() {
        return print_json(&note, output);
    }

    print_note_detail(&note);
    Ok(())
}

pub async fn show(
    note_id: String,
    api_key_override: Option<String>,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let api_key = resolve_api_key(api_key_override)?;
    let client = GranolaClient::new(api_key)?;
    let note = client.get_note(&note_id, false).await?;
    if output.is_json() {
        return print_json(&note, output);
    }
    print_note_detail(&note);
    Ok(())
}

pub async fn open(
    note_id: String,
    api_key_override: Option<String>,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let api_key = resolve_api_key(api_key_override)?;
    let client = GranolaClient::new(api_key)?;
    let note = client.get_note(&note_id, false).await?;
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

pub fn search(query: String, output: &OutputOptions) -> Result<(), CliError> {
    let cache = cache::load()?.ok_or_else(|| {
        CliError::invalid_input(
            "no local cache found; run `granola sync --since 30d --all` before searching",
        )
    })?;
    let notes: Vec<Note> = cache::search_notes(&cache, &query)
        .into_iter()
        .cloned()
        .collect();
    if output.is_json() {
        return print_json(
            &json!({ "query": query, "notes": notes, "count": notes.len(), "cache_synced_at": cache.synced_at }),
            output,
        );
    }
    print_note_table(&notes.iter().map(NoteSummary::from).collect::<Vec<_>>());
    Ok(())
}

async fn list_shortcut(
    api_key_override: Option<String>,
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
                    note.id.as_str(),
                    note.title.as_deref().unwrap_or(""),
                    note.owner.email.as_str(),
                    note.updated_at.as_str(),
                ]
            })
            .collect(),
    );
}

fn print_rows(headers: &[&str], rows: Vec<Vec<&str>>) {
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
        print_table_row(row.iter().copied(), &widths);
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

impl From<&Note> for NoteSummary {
    fn from(note: &Note) -> Self {
        Self {
            id: note.id.clone(),
            object: note.object.clone(),
            title: note.title.clone(),
            owner: note.owner.clone(),
            created_at: note.created_at.clone(),
            updated_at: note.updated_at.clone(),
        }
    }
}
