use crate::api::{resolve_api_key, validate_page_size, GranolaClient, ListNotesParams};
use crate::error::CliError;
use crate::output::{print_json, OutputOptions};
use crate::types::NoteSummary;
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use clap::Args;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Args)]
pub struct DigestCommand {
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
    /// Page size, capped by the Granola API at 30.
    #[arg(long, default_value_t = 30)]
    page_size: u8,
    /// Fetch all pages.
    #[arg(long)]
    all: bool,
    /// Maximum number of notes to include.
    #[arg(long)]
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
struct Digest {
    count: usize,
    created_after: Option<String>,
    created_before: Option<String>,
    updated_after: Option<String>,
    folder_id: Option<String>,
    by_day: Vec<DayBucket>,
    by_owner: Vec<OwnerBucket>,
    notes: Vec<NoteSummary>,
}

#[derive(Debug, Serialize)]
struct DayBucket {
    day: String,
    count: usize,
    notes: Vec<NoteSummary>,
}

#[derive(Debug, Serialize)]
struct OwnerBucket {
    owner: String,
    count: usize,
}

pub async fn handle(
    command: DigestCommand,
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
        .or(command.created_after)
        .or(Some(relative_time_after("7d")?));
    let updated_after = command
        .updated_since
        .as_deref()
        .map(relative_time_after)
        .transpose()?
        .or(command.updated_after);
    let mut params = ListNotesParams {
        created_before: command.created_before.clone(),
        created_after: created_after.clone(),
        updated_after: updated_after.clone(),
        folder_id: command.folder_id.clone(),
        cursor: None,
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
        if command.limit.is_some_and(|limit| notes.len() >= limit)
            || !command.all
            || !response.has_more
        {
            break;
        }
        let Some(cursor) = response.cursor else { break };
        params.cursor = Some(cursor);
    }
    notes.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    let digest = build_digest(
        notes,
        created_after,
        command.created_before,
        updated_after,
        command.folder_id,
    );

    if output.is_json() {
        return print_json(&digest, output);
    }
    print_digest(&digest, output);
    Ok(())
}

fn build_digest(
    notes: Vec<NoteSummary>,
    created_after: Option<String>,
    created_before: Option<String>,
    updated_after: Option<String>,
    folder_id: Option<String>,
) -> Digest {
    let mut days: BTreeMap<String, Vec<NoteSummary>> = BTreeMap::new();
    let mut owners: BTreeMap<String, usize> = BTreeMap::new();
    for note in &notes {
        days.entry(note_day(&note.created_at))
            .or_default()
            .push(note.clone());
        *owners.entry(note.owner.email.clone()).or_default() += 1;
    }

    let mut by_day: Vec<DayBucket> = days
        .into_iter()
        .map(|(day, mut notes)| {
            notes.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
            DayBucket {
                day,
                count: notes.len(),
                notes,
            }
        })
        .collect();
    by_day.reverse();

    let mut by_owner: Vec<OwnerBucket> = owners
        .into_iter()
        .map(|(owner, count)| OwnerBucket { owner, count })
        .collect();
    by_owner.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then(left.owner.cmp(&right.owner))
    });

    Digest {
        count: notes.len(),
        created_after,
        created_before,
        updated_after,
        folder_id,
        by_day,
        by_owner,
        notes,
    }
}

fn print_digest(digest: &Digest, output: &OutputOptions) {
    if digest.count == 0 {
        println!("No notes found for digest.");
        return;
    }
    println!("Granola digest: {} note(s)", digest.count);
    if let Some(created_after) = &digest.created_after {
        println!("created after: {created_after}");
    }
    if let Some(updated_after) = &digest.updated_after {
        println!("updated after: {updated_after}");
    }
    if let Some(folder_id) = &digest.folder_id {
        println!("folder: {folder_id}");
    }
    println!();
    println!("By day");
    for bucket in &digest.by_day {
        println!("- {} ({} note(s))", bucket.day, bucket.count);
        for note in &bucket.notes {
            println!(
                "  - {} — {} ({})",
                note.title.as_deref().unwrap_or("Untitled note"),
                note.owner.email,
                note.id
            );
        }
    }
    if !output.quiet && !digest.by_owner.is_empty() {
        println!();
        println!("By owner");
        for bucket in &digest.by_owner {
            println!("- {}: {}", bucket.owner, bucket.count);
        }
    }
}

fn note_day(value: &str) -> String {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.date_naive().to_string())
        .unwrap_or_else(|_| value.chars().take(10).collect())
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
