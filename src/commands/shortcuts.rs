use crate::api::{resolve_api_key, GranolaClient, ListNotesParams};
use crate::cache::{self, CacheMode};
use crate::error::CliError;
use crate::output::{print_json, print_rows, OutputOptions};
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
    cache_mode: CacheMode,
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
    let note =
        get_note_with_cache(&client, &summary.id, command.include_transcript, cache_mode).await?;

    if output.is_json() {
        return print_json(&note, output);
    }

    print_note_detail(&note);
    Ok(())
}

async fn get_note_with_cache(
    client: &GranolaClient,
    note_id: &str,
    include_transcript: bool,
    cache_mode: CacheMode,
) -> Result<Note, CliError> {
    if cache_mode.read {
        if let Some(mut note) = cache::get_note(note_id)? {
            if !include_transcript || note.transcript.is_some() {
                if !include_transcript {
                    note.transcript = None;
                }
                return Ok(note);
            }
        }
    }

    let note = client.get_note(note_id, include_transcript).await?;
    cache_notes_if_enabled(std::slice::from_ref(&note), cache_mode.write)?;
    Ok(note)
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

    print_note_table(&notes, output)
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

fn print_note_table(notes: &[NoteSummary], output: &OutputOptions) -> Result<(), CliError> {
    if notes.is_empty() {
        if !output.fields.is_empty() {
            return Ok(());
        }
        println!("No notes found");
        return Ok(());
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
        output,
    )
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
