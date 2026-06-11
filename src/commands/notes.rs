use crate::api::{
    resolve_api_key, try_resolve_api_key, validate_page_size, GranolaApi, GranolaClient,
    ListNotesParams,
};
use crate::cache::{self, CacheMode, CacheStore};
use crate::error::CliError;
use crate::note_ref::normalize_note_id;
use crate::output::{print_json, print_rows, OutputOptions};
use crate::redaction::{redact_note, redact_note_summary, redact_notes, RedactionKind};
use crate::types::{Note, NoteSummary};
use chrono::{Duration, SecondsFormat, Utc};
use clap::{Args, Subcommand, ValueEnum};
use serde_json::{json, Value};
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;

mod fields;

pub(crate) use fields::note_field_specs;
use fields::NoteOutputCommand;

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
    /// Retrieve many full note records for IDs or list filters.
    #[command(name = "get-many")]
    GetMany(GetManyNotesCommand),
    /// Search the local note cache, not the Granola API.
    Search(SearchNotesCommand),
    /// List fields available for note output and pipelines.
    Fields(FieldsNotesCommand),
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
    /// Sort returned notes by this field. Defaults to updated-at.
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
    /// Granola note ID or copied Granola note URL. If omitted and stdin is piped, reads one ID/URL from stdin.
    #[arg(value_name = "NOTE_ID_OR_URL")]
    note_id: Option<String>,
    /// Redact sensitive data in output. Repeat or comma-separate values: emails, phones, secrets, attendees.
    #[arg(long, value_enum, value_delimiter = ',')]
    redact: Vec<RedactionKind>,
}

#[derive(Debug, Args)]
struct GetManyNotesCommand {
    /// Note IDs or copied Granola note URLs. If omitted, pass a list filter or --all.
    #[arg(value_name = "NOTE_ID_OR_URL")]
    note_ids: Vec<String>,
    /// Read one note ID or URL per line from this file.
    #[arg(long)]
    notes_file: Option<PathBuf>,
    /// Read one note ID or URL per line from stdin.
    #[arg(long)]
    stdin: bool,
    /// Include transcripts in returned notes.
    #[arg(long)]
    include_transcript: bool,
    /// Emit newline-delimited JSON, one note per line.
    #[arg(long)]
    jsonl: bool,
    /// Redact sensitive data in output. Repeat or comma-separate values: emails, phones, secrets, attendees.
    #[arg(long, value_enum, value_delimiter = ',')]
    redact: Vec<RedactionKind>,
    /// Return notes created before this date or date-time when selecting by filters.
    #[arg(long)]
    created_before: Option<String>,
    /// Return notes created after this date or date-time when selecting by filters.
    #[arg(long, conflicts_with = "since")]
    created_after: Option<String>,
    /// Return notes created within a relative duration, e.g. 7d, 24h, 30m.
    #[arg(long, conflicts_with = "created_after")]
    since: Option<String>,
    /// Return notes updated after this date or date-time when selecting by filters.
    #[arg(long, conflicts_with = "updated_since")]
    updated_after: Option<String>,
    /// Return notes updated within a relative duration, e.g. 7d, 24h, 30m.
    #[arg(long, conflicts_with = "updated_after")]
    updated_since: Option<String>,
    /// Return notes in this folder and child folders when selecting by filters.
    #[arg(long)]
    folder_id: Option<String>,
    /// Cursor to continue from when selecting by filters.
    #[arg(long)]
    cursor: Option<String>,
    /// Page size, capped by the Granola API at 30.
    #[arg(long, default_value_t = 10)]
    page_size: u8,
    /// Fetch all pages when selecting by filters.
    #[arg(long)]
    all: bool,
    /// Maximum number of notes to return.
    #[arg(long)]
    limit: Option<usize>,
}

#[derive(Debug, Args)]
#[command(
    about = "Search the local note cache, not the Granola API.",
    long_about = "Search the local SQLite cache, not the Granola API. Run `granola sync --since 30d --all --include-transcript` first, especially for transcript search. Supports SQLite FTS5 syntax such as field filters and phrases. Examples: `granola notes search apple`, `granola notes search attendees:will async config`, `granola notes search attendees:will \"async config\"`, `granola notes search transcript:renewal`.",
    after_help = "Tip: run `granola sync --since 30d --all --include-transcript` first. Transcript search only covers notes cached with transcripts.\n\nPipeline examples:\n  granola notes search renewal --fields id --limit 1 | granola notes get --fields summary\n  granola notes search renewal --fields id --output text --limit 1 | granola notes get --fields transcript\n  granola notes search renewal --fields id | granola notes get-many --stdin --jsonl"
)]
struct SearchNotesCommand {
    /// Read additional search query text from stdin. If QUERY is omitted and stdin is piped, stdin is read automatically.
    #[arg(long)]
    stdin: bool,
    /// Search query. Multiple arguments are joined, so `granola notes search attendees:will "async config"` works without quoting the entire query.
    #[arg(value_name = "QUERY", num_args = 0..)]
    query: Vec<String>,
    /// Maximum number of cached notes to return.
    #[arg(long)]
    limit: Option<usize>,
    /// Return cached notes created within a relative duration, e.g. 7d, 24h, 30m.
    #[arg(long, conflicts_with = "created_after")]
    since: Option<String>,
    /// Return cached notes created after this date or date-time.
    #[arg(long, conflicts_with = "since")]
    created_after: Option<String>,
    /// Return cached notes updated within a relative duration, e.g. 7d, 24h, 30m.
    #[arg(long, conflicts_with = "updated_after")]
    updated_since: Option<String>,
    /// Return cached notes updated after this date or date-time.
    #[arg(long, conflicts_with = "updated_since")]
    updated_after: Option<String>,
    /// Redact sensitive data in output. Repeat or comma-separate values: emails, phones, secrets, attendees.
    #[arg(long, value_enum, value_delimiter = ',')]
    redact: Vec<RedactionKind>,
}

#[derive(Debug, Args)]
#[command(
    about = "List fields available for note output and pipelines.",
    long_about = "List fields accepted by note commands such as `granola notes get --fields summary` and `granola notes search --fields id`. Use `--output json` for machine-readable field metadata."
)]
struct FieldsNotesCommand {
    /// Limit fields to one notes command.
    #[arg(value_enum)]
    command: Option<NoteFieldsCommand>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum NoteFieldsCommand {
    List,
    Search,
    Get,
}

#[derive(Debug, Args)]
struct OpenNoteCommand {
    /// Granola note ID or copied Granola note URL.
    #[arg(value_name = "NOTE_ID_OR_URL")]
    note_id: String,
    /// Print the note URL instead of opening it.
    #[arg(long)]
    print: bool,
}

pub async fn handle(
    command: NotesCommand,
    api_key_override: Option<String>,
    cache_mode: CacheMode,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let cache_store = cache::OsCacheStore;
    match command.command {
        NotesSubcommand::Search(command) => {
            let warning_client = warning_client(api_key_override);
            search_notes(command, &cache_store, warning_client.as_ref(), output).await
        }
        NotesSubcommand::Fields(command) => note_fields(command, output),
        command => {
            if let NotesSubcommand::GetMany(command) = &command {
                require_note_selector(command, "get-many")?;
            }
            let api_key = resolve_api_key(api_key_override)?;
            let client = GranolaClient::new(api_key)?;
            match command {
                NotesSubcommand::List(command) => {
                    list_notes(&client, &cache_store, command, cache_mode.write, output).await
                }
                NotesSubcommand::Get(command) => {
                    get_note(&client, &cache_store, command, cache_mode, output).await
                }
                NotesSubcommand::GetMany(command) => {
                    hydrate_notes(&client, &cache_store, command, cache_mode, output).await
                }
                NotesSubcommand::Open(command) => {
                    open_note(&client, &cache_store, command, cache_mode, output).await
                }
                NotesSubcommand::Search(_) => unreachable!(),
                NotesSubcommand::Fields(_) => unreachable!(),
            }
        }
    }
}

fn note_fields(command: FieldsNotesCommand, output: &OutputOptions) -> Result<(), CliError> {
    let fields: Vec<_> = match command.command {
        Some(command) => fields::specs_for_command(command.into(), true).collect(),
        None => note_field_specs().iter().collect(),
    };

    if output.is_json() {
        return print_json(&json!({ "fields": fields, "count": fields.len() }), output);
    }

    print_rows(
        &[
            "name",
            "commands",
            "requires_transcript",
            "json_only",
            "description",
        ],
        fields
            .into_iter()
            .map(|field| {
                vec![
                    field.name.to_string(),
                    field.commands.join(","),
                    field.requires_transcript.to_string(),
                    field.json_only.to_string(),
                    field.description.to_string(),
                ]
            })
            .collect(),
        output,
    )?;
    Ok(())
}

async fn list_notes(
    client: &impl GranolaApi,
    cache_store: &impl CacheStore,
    command: ListNotesCommand,
    write_through_cache: bool,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let result = collect_list_notes(client, &command).await?;
    cache_summaries_if_enabled(cache_store, &result.notes, write_through_cache)?;

    if output.is_json() {
        let count = result.notes.len();
        return print_json(
            &json!({
                "notes": result.notes,
                "count": count,
                "has_more": result.has_more,
                "cursor": result.cursor,
                "page_size": result.page_size,
            }),
            output,
        );
    }

    print_note_table(&result.notes, command.no_truncate, output)?;
    Ok(())
}

#[derive(Debug, Clone)]
struct ListNotesResult {
    notes: Vec<NoteSummary>,
    has_more: bool,
    cursor: Option<String>,
    page_size: u8,
}

async fn collect_list_notes(
    client: &impl GranolaApi,
    command: &ListNotesCommand,
) -> Result<ListNotesResult, CliError> {
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

    let (has_more, next_cursor) = loop {
        let response = client.list_notes(&params).await?;
        let page_meta = (response.has_more, response.cursor.clone());
        for note in response.notes {
            if command.limit.is_some_and(|limit| notes.len() >= limit) {
                break;
            }
            notes.push(note);
        }

        if command.limit.is_some_and(|limit| notes.len() >= limit) {
            break page_meta;
        }
        if !command.all || !response.has_more {
            break page_meta;
        }
        let Some(cursor) = response.cursor else {
            break page_meta;
        };
        params.cursor = Some(cursor);
    };

    sort_notes(&mut notes, command.sort, command.order);
    Ok(ListNotesResult {
        notes,
        has_more,
        cursor: next_cursor,
        page_size,
    })
}

async fn search_notes(
    command: SearchNotesCommand,
    cache_store: &impl CacheStore,
    warning_client: Option<&impl GranolaApi>,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let query = super::resolve_search_query(&command.query, command.stdin)?;
    let status = cache_store.status()?;
    let mut hits = cache_store.search(&query, search_cache_limit(&command))?.ok_or_else(|| {
        CliError::invalid_input(
            "no local cache found; `granola notes search` searches only local cached notes. Run `granola sync --since 30d --all --include-transcript` before searching transcripts",
        )
    })?;
    filter_search_hits(&mut hits, &command)?;
    redact_search_hits(&mut hits, &command.redact);
    let count = hits.len();
    let warning = search_cache_warning(&status, cache_store, warning_client).await?;

    if output.is_json() {
        return print_json(
            &json!({
                "query": query,
                "results": hits,
                "count": count,
                "warning": warning,
                "cache_synced_at": status.synced_at,
                "cache": {
                    "path": status.path,
                    "synced_at": status.synced_at,
                    "summaries": status.summaries,
                    "hydrated_notes": status.hydrated_notes,
                    "transcript_notes": status.transcript_notes,
                    "has_unhydrated_summaries": status.summaries > status.hydrated_notes,
                },
            }),
            output,
        );
    }

    print_search_result_table(&hits, &query, &status, output)?;
    if let Some(warning) =
        warning.filter(|_| count == 0 || status.summaries > status.hydrated_notes)
    {
        eprintln!("hint: {warning}");
    }
    Ok(())
}

fn redact_search_hits(hits: &mut [cache::CacheSearchHit], kinds: &[RedactionKind]) {
    if kinds.is_empty() {
        return;
    }
    for hit in hits {
        redact_note_summary(&mut hit.summary, kinds);
        if let Some(note) = &mut hit.note {
            redact_note(note, kinds);
        }
    }
}

fn search_cache_limit(command: &SearchNotesCommand) -> Option<usize> {
    if search_has_time_filter(command) {
        None
    } else {
        command.limit
    }
}

fn search_has_time_filter(command: &SearchNotesCommand) -> bool {
    command.since.is_some()
        || command.created_after.is_some()
        || command.updated_since.is_some()
        || command.updated_after.is_some()
}

fn filter_search_hits(
    hits: &mut Vec<cache::CacheSearchHit>,
    command: &SearchNotesCommand,
) -> Result<(), CliError> {
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

    if let Some(created_after) = created_after {
        hits.retain(|hit| hit.summary.created_at.as_str() >= created_after.as_str());
    }
    if let Some(updated_after) = updated_after {
        hits.retain(|hit| hit.summary.updated_at.as_str() >= updated_after.as_str());
    }
    if let Some(limit) = command.limit {
        hits.truncate(limit);
    }

    Ok(())
}

async fn hydrate_notes(
    client: &impl GranolaApi,
    cache_store: &impl CacheStore,
    command: GetManyNotesCommand,
    cache_mode: CacheMode,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let include_transcript = command.include_transcript
        || json_fields_request_transcript(output)
        || human_fields_require_transcript(output, NoteOutputCommand::Get)?;
    let ids = hydrate_note_ids(client, &command).await?;
    let mut notes = Vec::with_capacity(ids.len());

    for id in ids {
        notes.push(
            get_note_with_cache(client, cache_store, &id, include_transcript, cache_mode).await?,
        );
    }
    redact_notes(&mut notes, &command.redact);

    if command.jsonl {
        for note in &notes {
            println!("{}", serde_json::to_string(note).map_err(CliError::from)?);
        }
        return Ok(());
    }

    if output.is_json() {
        return print_json(&json!({ "notes": notes, "count": notes.len() }), output);
    }

    print_hydrated_note_table(&notes, output)?;
    Ok(())
}

async fn hydrate_note_ids(
    client: &impl GranolaApi,
    command: &GetManyNotesCommand,
) -> Result<Vec<String>, CliError> {
    let mut ids = normalize_note_refs(&command.note_ids)?;

    if let Some(path) = &command.notes_file {
        let content = fs::read_to_string(path).map_err(|error| {
            CliError::general(format!(
                "failed to read IDs from {}: {error}",
                path.display()
            ))
        })?;
        ids.extend(
            parse_ids(&content)
                .map(|id| normalize_note_id(&id))
                .collect::<Result<Vec<_>, _>>()?,
        );
    }

    if command.stdin {
        let mut content = String::new();
        io::stdin().read_to_string(&mut content).map_err(|error| {
            CliError::general(format!("failed to read IDs from stdin: {error}"))
        })?;
        ids.extend(
            parse_ids(&content)
                .map(|id| normalize_note_id(&id))
                .collect::<Result<Vec<_>, _>>()?,
        );
    }

    ids.sort();
    ids.dedup();

    if !ids.is_empty() {
        if command_has_selection_filters(command) {
            return Err(CliError::invalid_input(
                "pass note IDs or list-selection filters, not both",
            ));
        }
        if let Some(limit) = command.limit {
            ids.truncate(limit);
        }
        return Ok(ids);
    }

    require_note_selector(command, "get-many")?;

    collect_hydrate_selection_ids(client, command).await
}

fn require_note_selector(command: &GetManyNotesCommand, name: &str) -> Result<(), CliError> {
    if command.note_ids.is_empty()
        && command.notes_file.is_none()
        && !command.stdin
        && !command_has_selection_filters(command)
    {
        return Err(no_note_selector_error(name));
    }
    Ok(())
}

fn no_note_selector_error(command: &str) -> CliError {
    CliError::invalid_input(format!(
        "no note selector provided for `granola notes {command}`; pass note IDs/URLs, --notes-file, --stdin, a filter such as --since 7d or --updated-since 24h, or --all"
    ))
}

async fn collect_hydrate_selection_ids(
    client: &impl GranolaApi,
    command: &GetManyNotesCommand,
) -> Result<Vec<String>, CliError> {
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

    Ok(ids)
}

fn parse_ids(content: &str) -> impl Iterator<Item = String> + '_ {
    content.lines().filter_map(|line| {
        let id = line.trim();
        (!id.is_empty() && !id.starts_with('#')).then(|| id.to_string())
    })
}

fn normalize_note_refs(inputs: &[String]) -> Result<Vec<String>, CliError> {
    inputs
        .iter()
        .map(|input| normalize_note_id(input))
        .collect()
}

fn command_has_selection_filters(command: &GetManyNotesCommand) -> bool {
    command.created_before.is_some()
        || command.created_after.is_some()
        || command.since.is_some()
        || command.updated_after.is_some()
        || command.updated_since.is_some()
        || command.folder_id.is_some()
        || command.cursor.is_some()
        || command.all
}

async fn get_note(
    client: &impl GranolaApi,
    cache_store: &impl CacheStore,
    command: GetNoteCommand,
    cache_mode: CacheMode,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let include_transcript = if output.is_json() {
        output.fields.is_empty() || json_fields_request_transcript(output)
    } else {
        human_fields_require_transcript(output, NoteOutputCommand::Get)?
    };
    let note_id = resolve_get_note_id(command.note_id.as_deref())?;
    let mut note = get_note_with_cache(
        client,
        cache_store,
        &note_id,
        include_transcript,
        cache_mode,
    )
    .await?;
    redact_note(&mut note, &command.redact);

    if output.is_json() {
        return print_json(&note_json_value(&note), output);
    }

    if !output.fields.is_empty() {
        fields::render_single_note_selected(&note, output)?;
        return Ok(());
    }

    print_note_detail(&note);
    Ok(())
}

fn human_fields_require_transcript(
    output: &OutputOptions,
    command: NoteOutputCommand,
) -> Result<bool, CliError> {
    if output.is_json() || output.fields.is_empty() {
        return Ok(false);
    }
    fields::requested_fields_require_transcript(&output.fields, command, output)
}

fn json_fields_request_transcript(output: &OutputOptions) -> bool {
    output.is_json()
        && output.fields.iter().any(|field| {
            matches!(
                field.trim().rsplit('.').next().unwrap_or_default(),
                "transcript" | "transcript_text" | "full"
            )
        })
}

fn note_json_value(note: &Note) -> Value {
    let mut value = serde_json::to_value(note).unwrap_or_else(|_| json!({}));
    if let Value::Object(map) = &mut value {
        map.insert("summary".to_string(), json!(render_note_summary(note)));
        map.insert(
            "transcript_text".to_string(),
            json!(render_transcript(note)),
        );
        map.insert("full".to_string(), json!(render_note_full(note)));
    }
    value
}

fn resolve_get_note_id(note_id: Option<&str>) -> Result<String, CliError> {
    if let Some(note_id) = note_id {
        return normalize_note_id(note_id);
    }

    if io::stdin().is_terminal() {
        return Err(CliError::invalid_input(
            "provide NOTE_ID_OR_URL or pipe one note ID/URL on stdin",
        ));
    }

    let mut content = String::new();
    io::stdin().read_to_string(&mut content).map_err(|error| {
        CliError::general(format!("failed to read note ID from stdin: {error}"))
    })?;
    let ids: Vec<String> = parse_ids(&content).collect();

    match ids.as_slice() {
        [id] => normalize_note_id(id),
        [] => Err(CliError::invalid_input(
            "provide NOTE_ID_OR_URL or pipe one note ID/URL on stdin",
        )),
        _ => Err(CliError::invalid_input(
            "`granola notes get` accepts one note ID; use `granola notes get-many --stdin` for multiple IDs",
        )),
    }
}

async fn get_note_with_cache(
    client: &impl GranolaApi,
    cache_store: &impl CacheStore,
    note_id: &str,
    include_transcript: bool,
    cache_mode: CacheMode,
) -> Result<Note, CliError> {
    if cache_mode.read {
        if let Some(mut note) = cache_store.get_note(note_id)? {
            if !include_transcript || note.transcript.is_some() {
                if !include_transcript {
                    note.transcript = None;
                }
                return Ok(note);
            }
        }
    }

    let note = client.get_note(note_id, include_transcript).await?;
    cache_notes_if_enabled(cache_store, std::slice::from_ref(&note), cache_mode.write)?;
    Ok(note)
}

async fn open_note(
    client: &impl GranolaApi,
    cache_store: &impl CacheStore,
    command: OpenNoteCommand,
    cache_mode: CacheMode,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let note_id = normalize_note_id(&command.note_id)?;
    let note = get_note_with_cache(client, cache_store, &note_id, false, cache_mode).await?;

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

fn cache_summaries_if_enabled(
    cache_store: &impl CacheStore,
    summaries: &[NoteSummary],
    write_through_cache: bool,
) -> Result<(), CliError> {
    if !write_through_cache || summaries.is_empty() {
        return Ok(());
    }

    cache_store.upsert_summaries(summaries)
}

fn cache_notes_if_enabled(
    cache_store: &impl CacheStore,
    notes: &[Note],
    write_through_cache: bool,
) -> Result<(), CliError> {
    if !write_through_cache || notes.is_empty() {
        return Ok(());
    }

    cache_store.upsert_notes(notes)
}

fn warning_client(api_key_override: Option<String>) -> Option<GranolaClient> {
    let api_key = try_resolve_api_key(api_key_override).ok().flatten()?;
    GranolaClient::new(api_key).ok()
}

async fn search_cache_warning(
    status: &cache::CacheStatus,
    cache_store: &impl CacheStore,
    client: Option<&impl GranolaApi>,
) -> Result<Option<String>, CliError> {
    if status.summaries > status.hydrated_notes {
        return Ok(Some(format!(
            "local cache has {} listed note(s) but only {} hydrated note(s); run `granola sync --since 30d --all --include-transcript` to search full summaries and transcripts",
            status.summaries,
            status.hydrated_notes
        )));
    }

    let Some(client) = client else {
        return Ok(None);
    };
    let Ok(response) = client
        .list_notes(&ListNotesParams {
            page_size: Some(30),
            ..Default::default()
        })
        .await
    else {
        return Ok(None);
    };

    let has_remote_uncached_updates = response
        .notes
        .iter()
        .any(|summary| !matches!(cache_store.contains_fresh_summary(summary), Ok(true)));

    Ok(has_remote_uncached_updates.then(|| {
        "newer or unlisted notes may not be in the local cache; run `granola sync --since 30d --all --include-transcript` to refresh before relying on transcript search results".to_string()
    }))
}

fn print_note_table(
    notes: &[NoteSummary],
    no_truncate: bool,
    output: &OutputOptions,
) -> Result<(), CliError> {
    fields::render_summary_records(notes, no_truncate, output)
}

fn print_hydrated_note_table(notes: &[Note], output: &OutputOptions) -> Result<(), CliError> {
    fields::render_note_records(notes, output)
}

fn print_search_result_table(
    hits: &[cache::CacheSearchHit],
    query: &str,
    status: &cache::CacheStatus,
    output: &OutputOptions,
) -> Result<(), CliError> {
    if hits.is_empty() {
        if !output.fields.is_empty() {
            return fields::render_search_records(hits, output);
        }
        print_no_search_hits(query, status);
        return Ok(());
    }

    fields::render_search_records(hits, output)
}

fn print_no_search_hits(query: &str, status: &cache::CacheStatus) {
    if status.summaries == 0 {
        println!("No notes are cached yet. `granola notes search` searches only local cached notes. Run `granola sync --since 30d --all --include-transcript` to populate the local search index.");
        return;
    }

    println!("No cached notes matched '{query}'.");
    println!(
        "Searched {} cached summary note(s), {} hydrated note(s), and {} transcript-indexed note(s).",
        status.summaries, status.hydrated_notes, status.transcript_notes
    );
}

fn cached_label(hit: &cache::CacheSearchHit) -> String {
    if hit.cached_transcript {
        "transcript".to_string()
    } else if hit.cached_detail {
        "full".to_string()
    } else {
        "summary".to_string()
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
    let sort = sort.unwrap_or(NoteSortField::UpdatedAt);
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

    println!("{}", render_note_summary(note));

    if let Some(transcript) = &note.transcript {
        println!();
        println!("Transcript items: {}", transcript.len());
    }
}

fn render_note_summary(note: &Note) -> String {
    note.summary_markdown
        .clone()
        .unwrap_or_else(|| note.summary_text.clone())
}

fn render_note_full(note: &Note) -> String {
    let mut out = String::new();
    out.push_str(note.title.as_deref().unwrap_or("Untitled note"));
    out.push('\n');
    out.push_str(&format!("id: {}\n", note.id));
    out.push_str(&format!("owner: {}\n", note.owner.email));
    out.push_str(&format!("created: {}\n", note.created_at));
    out.push_str(&format!("updated: {}\n", note.updated_at));
    out.push_str(&format!("url: {}\n\n", note.web_url));
    out.push_str(&render_note_summary(note));
    out.push_str("\n\n");
    out.push_str(&render_transcript(note));
    out
}

fn render_transcript(note: &Note) -> String {
    let Some(transcript) = &note.transcript else {
        return "No transcript available".to_string();
    };

    transcript
        .iter()
        .map(|item| {
            let speaker = item
                .speaker
                .diarization_label
                .as_deref()
                .unwrap_or(item.speaker.source.as_str());
            format!("{speaker}: {}", item.text)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::ListFoldersParams;
    use crate::types::{ListFoldersResponse, ListNotesResponse, User};
    use std::collections::VecDeque;
    use std::sync::Mutex;

    #[test]
    fn parses_relative_duration_syntax() {
        assert!(relative_time_after("7d").is_ok());
        assert!(relative_time_after("24h").is_ok());
        assert!(relative_time_after("30m").is_ok());
        assert!(relative_time_after("0d").is_err());
        assert!(relative_time_after("7w").is_err());
    }

    #[test]
    fn resolves_get_note_id_from_argument() {
        assert_eq!(
            resolve_get_note_id(Some("https://notes.granola.ai/d/not_123")).unwrap(),
            "not_123"
        );
    }

    #[test]
    fn transcript_fields_imply_transcript_fetch() {
        let output = OutputOptions::new(crate::OutputFormat::Text, false, Vec::new(), false);

        assert!(fields::requested_fields_require_transcript(
            &["transcript".to_string()],
            NoteOutputCommand::Get,
            &output
        )
        .unwrap());
        assert!(fields::requested_fields_require_transcript(
            &["transcript_text".to_string()],
            NoteOutputCommand::Get,
            &output
        )
        .unwrap());
        assert!(fields::requested_fields_require_transcript(
            &["full".to_string()],
            NoteOutputCommand::Get,
            &output
        )
        .unwrap());
        assert!(!fields::requested_fields_require_transcript(
            &["summary".to_string()],
            NoteOutputCommand::Get,
            &output
        )
        .unwrap());
    }

    #[test]
    fn renders_semantic_note_fields() {
        let note = fixture_note_with_transcript();

        assert_eq!(
            fields::render_note_field(&note, "id").unwrap().as_str(),
            note.id.as_str()
        );
        assert_eq!(
            fields::render_note_field(&note, "summary")
                .unwrap()
                .as_str(),
            "# Redacted Summary\n\n- Redacted bullet"
        );
        assert!(fields::render_note_field(&note, "transcript")
            .unwrap()
            .contains("Speaker A: Redacted transcript text."));
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
    fn sorts_notes_by_updated_at_descending_by_default() {
        let mut notes = vec![
            note_summary("not_old", "Old", "2026-01-01"),
            note_summary("not_new", "New", "2026-01-03"),
            note_summary("not_middle", "Middle", "2026-01-02"),
        ];

        sort_notes(&mut notes, None, SortOrder::Desc);

        assert_eq!(
            notes
                .iter()
                .map(|note| note.id.as_str())
                .collect::<Vec<_>>(),
            vec!["not_new", "not_middle", "not_old"]
        );
    }

    #[test]
    fn truncates_long_table_titles_by_default() {
        let note = note_summary("not_A", &"a".repeat(100), "2026-01-01");
        assert_eq!(display_title(&note, false).chars().count(), 80);
        assert_eq!(display_title(&note, true).chars().count(), 100);
    }

    #[tokio::test]
    async fn collect_list_notes_uses_injected_api_for_paging_limit_and_sort() {
        let api = FakeApi::with_note_pages(vec![
            ListNotesResponse {
                notes: vec![
                    note_summary("not_C", "Charlie", "2026-01-03"),
                    note_summary("not_A", "Alpha", "2026-01-01"),
                ],
                has_more: true,
                cursor: Some("cursor-1".to_string()),
            },
            ListNotesResponse {
                notes: vec![
                    note_summary("not_B", "Bravo", "2026-01-02"),
                    note_summary("not_D", "Delta", "2026-01-04"),
                ],
                has_more: true,
                cursor: Some("cursor-2".to_string()),
            },
        ]);
        let command = ListNotesCommand {
            created_before: None,
            created_after: None,
            since: None,
            updated_after: None,
            updated_since: None,
            folder_id: Some("fol_test".to_string()),
            cursor: None,
            page_size: 2,
            all: true,
            limit: Some(3),
            sort: Some(NoteSortField::Title),
            order: SortOrder::Asc,
            no_truncate: false,
        };

        let result = collect_list_notes(&api, &command).await.unwrap();

        assert_eq!(
            result
                .notes
                .iter()
                .map(|note| note.id.as_str())
                .collect::<Vec<_>>(),
            vec!["not_A", "not_B", "not_C"]
        );
        assert!(result.has_more);
        assert_eq!(result.cursor.as_deref(), Some("cursor-2"));
        assert_eq!(result.page_size, 2);

        let params = api.list_note_params.lock().unwrap();
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].folder_id.as_deref(), Some("fol_test"));
        assert_eq!(params[0].cursor.as_deref(), None);
        assert_eq!(params[1].cursor.as_deref(), Some("cursor-1"));
    }

    #[test]
    fn cache_write_through_uses_injected_cache_store() {
        let cache = RecordingCache::default();
        let summaries = vec![note_summary("not_A", "Alpha", "2026-01-01")];

        cache_summaries_if_enabled(&cache, &summaries, true).unwrap();

        assert_eq!(cache.summary_ids.lock().unwrap().as_slice(), &["not_A"]);
    }

    #[test]
    fn cache_write_through_skips_injected_cache_when_disabled() {
        let cache = RecordingCache::default();
        let summaries = vec![note_summary("not_A", "Alpha", "2026-01-01")];

        cache_summaries_if_enabled(&cache, &summaries, false).unwrap();

        assert!(cache.summary_ids.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn get_note_with_cache_uses_complete_cached_note() {
        let mut note = fixture_note_with_transcript();
        note.id = "not_cached".to_string();
        let cache = RecordingCache::with_note(note.clone());
        let api = FakeApi::default();

        let result =
            get_note_with_cache(&api, &cache, "not_cached", true, CacheMode::new(true, true))
                .await
                .unwrap();

        assert_eq!(result.id, "not_cached");
        assert!(result.transcript.is_some());
        assert!(api.get_note_requests.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn get_note_with_cache_fetches_when_transcript_missing() {
        let mut cached = fixture_note_without_transcript();
        cached.id = "not_cached".to_string();
        let mut remote = fixture_note_with_transcript();
        remote.id = "not_cached".to_string();
        let cache = RecordingCache::with_note(cached);
        let api = FakeApi::with_notes(vec![remote]);

        let result =
            get_note_with_cache(&api, &cache, "not_cached", true, CacheMode::new(true, true))
                .await
                .unwrap();

        assert!(result.transcript.is_some());
        assert_eq!(api.get_note_requests.lock().unwrap().as_slice(), &[true]);
        assert_eq!(cache.note_ids.lock().unwrap().as_slice(), &["not_cached"]);
    }

    #[tokio::test]
    async fn get_note_with_cache_skips_reading_when_disabled() {
        let mut cached = fixture_note_with_transcript();
        cached.id = "not_cached".to_string();
        let mut remote = fixture_note_without_transcript();
        remote.id = "not_remote".to_string();
        let cache = RecordingCache::with_note(cached);
        let api = FakeApi::with_notes(vec![remote]);

        let result = get_note_with_cache(
            &api,
            &cache,
            "not_cached",
            false,
            CacheMode::new(false, true),
        )
        .await
        .unwrap();

        assert_eq!(result.id, "not_remote");
        assert_eq!(api.get_note_requests.lock().unwrap().as_slice(), &[false]);
    }

    #[test]
    fn get_many_requires_an_explicit_selector() {
        let command = get_many_command();

        let error = require_note_selector(&command, "get-many").unwrap_err();

        assert!(error
            .to_string()
            .contains("no note selector provided for `granola notes get-many`"));
    }

    #[test]
    fn get_many_accepts_list_filters_as_selector() {
        let command = GetManyNotesCommand {
            since: Some("7d".to_string()),
            ..get_many_command()
        };

        require_note_selector(&command, "get-many").unwrap();
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

    fn fixture_note_without_transcript() -> Note {
        serde_json::from_str(include_str!("../../tests/fixtures/get_note.json")).unwrap()
    }

    fn fixture_note_with_transcript() -> Note {
        serde_json::from_str(include_str!(
            "../../tests/fixtures/get_note_with_transcript.json"
        ))
        .unwrap()
    }

    fn get_many_command() -> GetManyNotesCommand {
        GetManyNotesCommand {
            note_ids: Vec::new(),
            notes_file: None,
            stdin: false,
            include_transcript: false,
            jsonl: false,
            redact: Vec::new(),
            created_before: None,
            created_after: None,
            since: None,
            updated_after: None,
            updated_since: None,
            folder_id: None,
            cursor: None,
            page_size: 10,
            all: false,
            limit: None,
        }
    }

    #[derive(Default)]
    struct FakeApi {
        note_pages: Mutex<VecDeque<ListNotesResponse>>,
        list_note_params: Mutex<Vec<ListNotesParams>>,
        notes: Mutex<VecDeque<Note>>,
        get_note_requests: Mutex<Vec<bool>>,
    }

    impl FakeApi {
        fn with_note_pages(pages: Vec<ListNotesResponse>) -> Self {
            Self {
                note_pages: Mutex::new(pages.into()),
                list_note_params: Mutex::new(Vec::new()),
                notes: Mutex::new(VecDeque::new()),
                get_note_requests: Mutex::new(Vec::new()),
            }
        }

        fn with_notes(notes: Vec<Note>) -> Self {
            Self {
                notes: Mutex::new(notes.into()),
                ..Default::default()
            }
        }
    }

    impl GranolaApi for FakeApi {
        async fn list_notes(
            &self,
            params: &ListNotesParams,
        ) -> Result<ListNotesResponse, CliError> {
            self.list_note_params.lock().unwrap().push(params.clone());
            self.note_pages
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| CliError::general("unexpected list_notes call"))
        }

        async fn get_note(
            &self,
            _note_id: &str,
            include_transcript: bool,
        ) -> Result<Note, CliError> {
            self.get_note_requests
                .lock()
                .unwrap()
                .push(include_transcript);
            self.notes
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| CliError::general("unexpected get_note call"))
        }

        async fn list_folders(
            &self,
            _params: &ListFoldersParams,
        ) -> Result<ListFoldersResponse, CliError> {
            Err(CliError::general("unexpected list_folders call"))
        }
    }

    #[derive(Default)]
    struct RecordingCache {
        summary_ids: Mutex<Vec<String>>,
        note_ids: Mutex<Vec<String>>,
        note: Mutex<Option<Note>>,
    }

    impl RecordingCache {
        fn with_note(note: Note) -> Self {
            Self {
                note: Mutex::new(Some(note)),
                ..Default::default()
            }
        }
    }

    impl CacheStore for RecordingCache {
        fn upsert_summaries(&self, summaries: &[NoteSummary]) -> Result<(), CliError> {
            self.summary_ids
                .lock()
                .unwrap()
                .extend(summaries.iter().map(|summary| summary.id.clone()));
            Ok(())
        }

        fn upsert_notes(&self, notes: &[Note]) -> Result<(), CliError> {
            self.note_ids
                .lock()
                .unwrap()
                .extend(notes.iter().map(|note| note.id.clone()));
            Ok(())
        }

        fn get_note(&self, note_id: &str) -> Result<Option<Note>, CliError> {
            Ok(self
                .note
                .lock()
                .unwrap()
                .clone()
                .filter(|note| note.id == note_id))
        }

        fn status(&self) -> Result<cache::CacheStatus, CliError> {
            Err(CliError::general("unexpected status call"))
        }

        fn search(
            &self,
            _query: &str,
            _limit: Option<usize>,
        ) -> Result<Option<Vec<cache::CacheSearchHit>>, CliError> {
            Err(CliError::general("unexpected search call"))
        }

        fn contains_fresh_summary(&self, _remote: &NoteSummary) -> Result<bool, CliError> {
            Err(CliError::general("unexpected contains_fresh_summary call"))
        }
    }
}
