use crate::api::{resolve_api_key, validate_page_size, GranolaClient, ListNotesParams};
use crate::cache;
use crate::config::{self, SavedView};
use crate::error::CliError;
use crate::output::{print_json, print_rows, OutputOptions};
use crate::types::NoteSummary;
use chrono::{Duration, SecondsFormat, Utc};
use clap::{Args, Subcommand, ValueEnum};
use serde_json::json;

#[derive(Debug, Args)]
pub struct ViewsCommand {
    #[command(subcommand)]
    command: ViewsSubcommand,
}

#[derive(Debug, Subcommand)]
enum ViewsSubcommand {
    /// Save a named local note view.
    Create(CreateViewCommand),
    /// List saved views.
    List,
    /// Show one saved view.
    Show { name: String },
    /// Delete one saved view.
    Delete { name: String },
    /// Run a saved view.
    Run(RunViewCommand),
}

#[derive(Debug, Args)]
struct CreateViewCommand {
    /// View name.
    name: String,
    /// Local cache search query for this view.
    #[arg(long)]
    query: Option<String>,
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
    /// Sort returned notes by this field.
    #[arg(long, value_enum)]
    sort: Option<NoteSortField>,
    /// Sort order.
    #[arg(long, value_enum)]
    order: Option<SortOrder>,
    /// Maximum number of results.
    #[arg(long)]
    limit: Option<usize>,
    /// Fetch all pages for list views.
    #[arg(long)]
    all: bool,
}

#[derive(Debug, Args)]
struct RunViewCommand {
    /// View name.
    name: String,
    /// Page size for API-backed list views.
    #[arg(long, default_value_t = 10)]
    page_size: u8,
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
    command: ViewsCommand,
    api_key_override: Option<String>,
    output: &OutputOptions,
) -> Result<(), CliError> {
    match command.command {
        ViewsSubcommand::Create(command) => create_view(command, output),
        ViewsSubcommand::List => list_views(output),
        ViewsSubcommand::Show { name } => show_view(&name, output),
        ViewsSubcommand::Delete { name } => delete_view(&name, output),
        ViewsSubcommand::Run(command) => run_view(command, api_key_override, output).await,
    }
}

fn create_view(command: CreateViewCommand, output: &OutputOptions) -> Result<(), CliError> {
    validate_name(&command.name)?;
    if !has_selector(&command) {
        return Err(CliError::invalid_input(
            "view must include --query or at least one list filter such as --since, --updated-since, or --folder-id",
        ));
    }

    let mut cfg = config::load()?;
    let view = SavedView {
        query: command.query,
        created_before: command.created_before,
        created_after: command.created_after,
        since: command.since,
        updated_after: command.updated_after,
        updated_since: command.updated_since,
        folder_id: command.folder_id,
        sort: command.sort.map(sort_name),
        order: command.order.map(order_name),
        limit: command.limit,
        all: command.all,
    };
    cfg.views.insert(command.name.clone(), view.clone());
    config::save(&cfg)?;

    if output.is_json() {
        return print_json(&json!({ "name": command.name, "view": view }), output);
    }
    if !output.quiet {
        println!("saved view '{}'", command.name);
    }
    Ok(())
}

fn list_views(output: &OutputOptions) -> Result<(), CliError> {
    let cfg = config::load()?;
    if output.is_json() {
        return print_json(
            &json!({ "views": cfg.views, "count": cfg.views.len() }),
            output,
        );
    }
    if cfg.views.is_empty() {
        println!("No saved views. Create one with `granola views create NAME --since 7d`.");
        return Ok(());
    }
    print_rows(
        &["name", "kind", "selector", "limit"],
        cfg.views
            .iter()
            .map(|(name, view)| {
                vec![
                    name.clone(),
                    if view.query.is_some() {
                        "search"
                    } else {
                        "list"
                    }
                    .to_string(),
                    view_selector(view),
                    view.limit.map(|n| n.to_string()).unwrap_or_default(),
                ]
            })
            .collect(),
        output,
    )
}

fn show_view(name: &str, output: &OutputOptions) -> Result<(), CliError> {
    let cfg = config::load()?;
    let view = cfg
        .views
        .get(name)
        .ok_or_else(|| CliError::not_found(format!("view '{name}' not found")))?;
    if output.is_json() {
        return print_json(&json!({ "name": name, "view": view }), output);
    }
    println!("view: {name}");
    println!(
        "kind: {}",
        if view.query.is_some() {
            "search"
        } else {
            "list"
        }
    );
    println!("selector: {}", view_selector(view));
    if let Some(limit) = view.limit {
        println!("limit: {limit}");
    }
    Ok(())
}

fn delete_view(name: &str, output: &OutputOptions) -> Result<(), CliError> {
    let mut cfg = config::load()?;
    let removed = cfg.views.remove(name).is_some();
    if !removed {
        return Err(CliError::not_found(format!("view '{name}' not found")));
    }
    config::save(&cfg)?;
    if output.is_json() {
        return print_json(&json!({ "name": name, "removed": true }), output);
    }
    if !output.quiet {
        println!("deleted view '{name}'");
    }
    Ok(())
}

async fn run_view(
    command: RunViewCommand,
    api_key_override: Option<String>,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let cfg = config::load()?;
    let view = cfg
        .views
        .get(&command.name)
        .ok_or_else(|| CliError::not_found(format!("view '{}' not found", command.name)))?;

    if let Some(query) = &view.query {
        return run_search_view(&command.name, query, view.limit, output);
    }

    let api_key = resolve_api_key(api_key_override)?;
    let client = GranolaClient::new(api_key)?;
    let notes = collect_view_notes(&client, view, command.page_size).await?;
    if output.is_json() {
        return print_json(
            &json!({ "name": command.name, "notes": notes, "count": notes.len() }),
            output,
        );
    }
    print_note_table(&notes, output)?;
    Ok(())
}

fn run_search_view(
    name: &str,
    query: &str,
    limit: Option<usize>,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let hits = cache::search(query, limit)?.ok_or_else(|| {
        CliError::invalid_input(
            "no local cache found; search views use only local cached notes. Run `granola sync --since 30d --all --include-transcript` before running transcript search views",
        )
    })?;
    if output.is_json() {
        return print_json(
            &json!({ "name": name, "query": query, "results": hits, "count": hits.len() }),
            output,
        );
    }
    if hits.is_empty() {
        println!("No cached notes matched saved view '{name}'.");
        return Ok(());
    }
    print_rows(
        &["id", "title", "owner", "updated_at", "cached"],
        hits.iter()
            .map(|hit| {
                vec![
                    hit.summary.id.clone(),
                    hit.summary.title.as_deref().unwrap_or("").to_string(),
                    hit.summary.owner.email.clone(),
                    hit.summary.updated_at.clone(),
                    if hit.cached_transcript {
                        "transcript"
                    } else if hit.cached_detail {
                        "full"
                    } else {
                        "summary"
                    }
                    .to_string(),
                ]
            })
            .collect(),
        output,
    )
}

async fn collect_view_notes(
    client: &GranolaClient,
    view: &SavedView,
    page_size: u8,
) -> Result<Vec<NoteSummary>, CliError> {
    let page_size = validate_page_size(page_size)?;
    let created_after = view
        .since
        .as_deref()
        .map(relative_time_after)
        .transpose()?
        .or_else(|| view.created_after.clone());
    let updated_after = view
        .updated_since
        .as_deref()
        .map(relative_time_after)
        .transpose()?
        .or_else(|| view.updated_after.clone());
    let mut params = ListNotesParams {
        created_before: view.created_before.clone(),
        created_after,
        updated_after,
        folder_id: view.folder_id.clone(),
        cursor: None,
        page_size: Some(page_size),
    };
    let mut notes = Vec::new();
    loop {
        let response = client.list_notes(&params).await?;
        for note in response.notes {
            if view.limit.is_some_and(|limit| notes.len() >= limit) {
                break;
            }
            notes.push(note);
        }
        if view.limit.is_some_and(|limit| notes.len() >= limit) || !view.all || !response.has_more {
            break;
        }
        let Some(cursor) = response.cursor else { break };
        params.cursor = Some(cursor);
    }
    sort_notes(&mut notes, view.sort.as_deref(), view.order.as_deref());
    Ok(notes)
}

fn has_selector(command: &CreateViewCommand) -> bool {
    command.query.is_some()
        || command.created_before.is_some()
        || command.created_after.is_some()
        || command.since.is_some()
        || command.updated_after.is_some()
        || command.updated_since.is_some()
        || command.folder_id.is_some()
}

fn validate_name(name: &str) -> Result<(), CliError> {
    if name.trim().is_empty()
        || !name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        return Err(CliError::invalid_input(
            "view name must contain only letters, numbers, '.', '_', or '-'",
        ));
    }
    Ok(())
}

fn view_selector(view: &SavedView) -> String {
    let mut parts = Vec::new();
    if let Some(query) = &view.query {
        parts.push(format!("query={query}"));
    }
    if let Some(since) = &view.since {
        parts.push(format!("since={since}"));
    }
    if let Some(created_after) = &view.created_after {
        parts.push(format!("created_after={created_after}"));
    }
    if let Some(updated_since) = &view.updated_since {
        parts.push(format!("updated_since={updated_since}"));
    }
    if let Some(folder_id) = &view.folder_id {
        parts.push(format!("folder_id={folder_id}"));
    }
    parts.join(", ")
}

fn sort_notes(notes: &mut [NoteSummary], sort: Option<&str>, order: Option<&str>) {
    let Some(sort) = sort else { return };
    notes.sort_by(|left, right| {
        let ord = match sort {
            "created-at" => left.created_at.cmp(&right.created_at),
            "updated-at" => left.updated_at.cmp(&right.updated_at),
            "title" => left
                .title
                .as_deref()
                .unwrap_or("")
                .to_lowercase()
                .cmp(&right.title.as_deref().unwrap_or("").to_lowercase()),
            _ => std::cmp::Ordering::Equal,
        };
        if order == Some("asc") {
            ord
        } else {
            ord.reverse()
        }
    });
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

fn sort_name(sort: NoteSortField) -> String {
    match sort {
        NoteSortField::CreatedAt => "created-at",
        NoteSortField::UpdatedAt => "updated-at",
        NoteSortField::Title => "title",
    }
    .to_string()
}

fn order_name(order: SortOrder) -> String {
    match order {
        SortOrder::Asc => "asc",
        SortOrder::Desc => "desc",
    }
    .to_string()
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
    fn validates_saved_view_names() {
        assert!(validate_name("customer-calls_1.prod").is_ok());
        assert!(validate_name("").is_err());
        assert!(validate_name("bad/name").is_err());
    }

    #[test]
    fn selectors_require_query_or_list_filter() {
        assert!(!has_selector(&create_command(None, None)));
        assert!(has_selector(&create_command(Some("mint"), None)));
        assert!(has_selector(&create_command(None, Some("7d"))));
    }

    #[test]
    fn formats_saved_view_selector() {
        let view = SavedView {
            query: Some("transcript:renewal".to_string()),
            since: Some("30d".to_string()),
            folder_id: Some("fol_123".to_string()),
            ..Default::default()
        };

        let selector = view_selector(&view);

        assert!(selector.contains("query=transcript:renewal"));
        assert!(selector.contains("since=30d"));
        assert!(selector.contains("folder_id=fol_123"));
    }

    #[test]
    fn sorts_notes_by_saved_view_settings() {
        let mut notes = vec![summary("not_b", "Beta"), summary("not_a", "Alpha")];

        sort_notes(&mut notes, Some("title"), Some("asc"));

        assert_eq!(notes[0].id, "not_a");
    }

    fn create_command(query: Option<&str>, since: Option<&str>) -> CreateViewCommand {
        CreateViewCommand {
            name: "view".to_string(),
            query: query.map(str::to_string),
            created_before: None,
            created_after: None,
            since: since.map(str::to_string),
            updated_after: None,
            updated_since: None,
            folder_id: None,
            sort: None,
            order: None,
            limit: None,
            all: false,
        }
    }

    fn summary(id: &str, title: &str) -> NoteSummary {
        NoteSummary {
            id: id.to_string(),
            object: "note".to_string(),
            title: Some(title.to_string()),
            owner: User {
                name: None,
                email: "owner@example.com".to_string(),
            },
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }
}
