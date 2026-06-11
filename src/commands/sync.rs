use crate::api::{resolve_api_key, validate_page_size, GranolaClient, ListNotesParams};
use crate::cache;
use crate::error::CliError;
use crate::output::{print_json, OutputOptions};
use chrono::{Duration, SecondsFormat, Utc};
use clap::Args;
use serde_json::json;

#[derive(Debug, Args)]
pub struct SyncCommand {
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
    #[arg(long, default_value_t = 30)]
    page_size: u8,
    /// Fetch all pages.
    #[arg(long)]
    all: bool,
    /// Maximum number of notes to sync.
    #[arg(long)]
    limit: Option<usize>,
    /// Include transcript data in the local cache.
    #[arg(long)]
    include_transcript: bool,
    /// Replace the cache instead of merging fetched notes into it.
    #[arg(long)]
    replace: bool,
}

pub async fn handle(
    command: SyncCommand,
    api_key_override: Option<String>,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let api_key = resolve_api_key(api_key_override)?;
    let client = GranolaClient::new(api_key)?;
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

    let mut fetched = Vec::with_capacity(ids.len());
    for id in ids {
        fetched.push(client.get_note(&id, command.include_transcript).await?);
    }

    let fetched_count = fetched.len();
    if command.replace {
        let cache = cache::merge_notes(None, fetched);
        cache::save(&cache)?;
    } else {
        cache::upsert_notes(&fetched)?;
    }
    let status = cache::status()?;

    let data = json!({
        "path": status.path,
        "fetched": fetched_count,
        "cached": status.hydrated_notes,
        "cached_summaries": status.summaries,
        "cached_hydrated_notes": status.hydrated_notes,
        "cached_transcript_notes": status.transcript_notes,
        "synced_at": status.synced_at,
        "included_transcripts": command.include_transcript,
    });
    if output.is_json() {
        return print_json(&data, output);
    }
    if !output.quiet {
        println!(
            "synced {fetched_count} note(s); cache now has {} note(s) at {}",
            status.hydrated_notes,
            status.path.display()
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
