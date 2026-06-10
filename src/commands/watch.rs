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
        let fresh = fresh_notes_for_poll(&mut seen, notes, command.include_existing, poll_number);
        emit_poll(poll_number, &fresh, output)?;

        if command.iterations.is_some_and(|limit| poll_number >= limit) {
            break;
        }
        sleep(TokioDuration::from_secs(command.interval_seconds)).await;
    }
    Ok(())
}

fn fresh_notes_for_poll(
    seen: &mut HashSet<String>,
    notes: Vec<NoteSummary>,
    include_existing: bool,
    poll_number: usize,
) -> Vec<NoteSummary> {
    notes
        .into_iter()
        .filter_map(|note| {
            let key = format!("{}:{}", note.id, note.updated_at);
            (seen.insert(key) && (include_existing || poll_number > 1)).then_some(note)
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::User;

    #[test]
    fn first_poll_suppresses_existing_notes_by_default() {
        let mut seen = HashSet::new();

        let fresh = fresh_notes_for_poll(&mut seen, vec![summary("not_a", "2026-01-01")], false, 1);

        assert!(fresh.is_empty());
        assert_eq!(seen.len(), 1);
    }

    #[test]
    fn later_poll_emits_new_and_updated_notes_once() {
        let mut seen = HashSet::new();
        let _ = fresh_notes_for_poll(&mut seen, vec![summary("not_a", "2026-01-01")], false, 1);

        let fresh = fresh_notes_for_poll(
            &mut seen,
            vec![
                summary("not_a", "2026-01-01"),
                summary("not_a", "2026-01-02"),
                summary("not_b", "2026-01-02"),
            ],
            false,
            2,
        );

        assert_eq!(fresh.len(), 2);
        assert_eq!(fresh[0].id, "not_a");
        assert_eq!(fresh[0].updated_at, "2026-01-02");
        assert_eq!(fresh[1].id, "not_b");
    }

    #[test]
    fn include_existing_emits_first_poll() {
        let mut seen = HashSet::new();

        let fresh = fresh_notes_for_poll(&mut seen, vec![summary("not_a", "2026-01-01")], true, 1);

        assert_eq!(fresh.len(), 1);
    }

    fn summary(id: &str, updated_at: &str) -> NoteSummary {
        NoteSummary {
            id: id.to_string(),
            object: "note".to_string(),
            title: Some(id.to_string()),
            owner: User {
                name: None,
                email: "owner@example.com".to_string(),
            },
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: updated_at.to_string(),
        }
    }
}
