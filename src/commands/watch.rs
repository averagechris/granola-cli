use crate::api::{resolve_api_key, validate_page_size, GranolaClient, ListNotesParams};
use crate::error::CliError;
use crate::output::OutputOptions;
use crate::types::NoteSummary;
use chrono::{Duration, SecondsFormat, Utc};
use clap::Args;
use serde_json::json;
use std::collections::HashSet;
use tokio::time::{sleep, Duration as TokioDuration};

#[derive(Debug, Args)]
pub struct WatchCommand {
    /// Return notes created within a relative duration on each poll, e.g. 2h, 1d, 30m.
    #[arg(long, default_value = "2h")]
    since: String,
    /// Return notes updated within a relative duration on each poll.
    #[arg(long)]
    updated_since: Option<String>,
    /// Return notes in this folder and child folders.
    #[arg(long)]
    folder_id: Option<String>,
    /// Page size, capped by the Granola API at 30.
    #[arg(long, default_value_t = 30)]
    page_size: u8,
    /// Seconds between polls.
    #[arg(long, default_value_t = 60)]
    interval_seconds: u64,
    /// Stop after this many polls. Omit to keep polling.
    #[arg(long)]
    iterations: Option<usize>,
    /// Emit notes from the first poll instead of only future discoveries.
    #[arg(long)]
    include_existing: bool,
}

pub async fn handle(
    command: WatchCommand,
    api_key_override: Option<String>,
    output: &OutputOptions,
) -> Result<(), CliError> {
    if command.interval_seconds == 0 {
        return Err(CliError::invalid_input(
            "--interval-seconds must be greater than zero",
        ));
    }
    let api_key = resolve_api_key(api_key_override)?;
    let client = GranolaClient::new(api_key)?;
    let page_size = validate_page_size(command.page_size)?;
    let mut seen = HashSet::new();
    let mut poll_number = 0usize;

    loop {
        poll_number += 1;
        let notes = poll_notes(&client, &command, page_size).await?;
        let mut fresh = Vec::new();
        for note in notes {
            let key = format!("{}:{}", note.id, note.updated_at);
            if seen.insert(key) && (command.include_existing || poll_number > 1) {
                fresh.push(note);
            }
        }
        emit_poll(poll_number, &fresh, output)?;

        if command.iterations.is_some_and(|limit| poll_number >= limit) {
            break;
        }
        sleep(TokioDuration::from_secs(command.interval_seconds)).await;
    }
    Ok(())
}

async fn poll_notes(
    client: &GranolaClient,
    command: &WatchCommand,
    page_size: u8,
) -> Result<Vec<NoteSummary>, CliError> {
    let created_after = Some(relative_time_after(&command.since)?);
    let updated_after = command
        .updated_since
        .as_deref()
        .map(relative_time_after)
        .transpose()?;
    let response = client
        .list_notes(&ListNotesParams {
            created_after,
            updated_after,
            folder_id: command.folder_id.clone(),
            page_size: Some(page_size),
            ..Default::default()
        })
        .await?;
    Ok(response.notes)
}

fn emit_poll(
    poll_number: usize,
    notes: &[NoteSummary],
    output: &OutputOptions,
) -> Result<(), CliError> {
    if output.is_json() {
        for note in notes {
            let line = if output.json_compact() {
                serde_json::to_string(&json!({
                    "event": "note_seen",
                    "poll": poll_number,
                    "note": note,
                }))
            } else {
                serde_json::to_string_pretty(&json!({
                    "event": "note_seen",
                    "poll": poll_number,
                    "note": note,
                }))
            }
            .map_err(CliError::from)?;
            println!("{line}");
        }
        return Ok(());
    }
    for note in notes {
        println!(
            "[poll {poll_number}] {} — {} ({})",
            note.updated_at,
            note.title.as_deref().unwrap_or("Untitled note"),
            note.id
        );
    }
    Ok(())
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
