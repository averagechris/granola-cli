pub mod api;
pub mod auth;
pub mod cache;
pub mod config;
pub mod export;
pub mod folders;
pub mod notes;
pub mod shortcuts;
pub mod sync;

use crate::api::{resolve_api_key, GranolaClient, ListNotesParams};
use crate::error::CliError;
use crate::output::{print_json, OutputOptions};
use clap::Args;
use serde_json::json;

#[derive(Debug, Args)]
pub struct DoctorCommand {
    /// Make a safe API request to validate reachability and credentials.
    #[arg(long)]
    validate: bool,
}

pub fn agent(output: &OutputOptions) -> Result<(), CliError> {
    let data = json!({
        "name": "granola",
        "version": env!("CARGO_PKG_VERSION"),
        "purpose": "Read Granola meeting notes, summaries, transcripts, and folders from the documented public API.",
        "commands": {
            "auth": {
                "description": "Store, validate, inspect, and remove API-key credentials.",
                "examples": ["granola auth login --key-stdin --validate", "granola auth status --validate --output json --compact", "granola auth logout --force"]
            },
            "notes": {
                "description": "List note metadata, fetch one note, hydrate batches, or open the Granola web URL.",
                "examples": [
                    "granola notes list --since 7d --sort updated-at --order desc --output json --compact",
                    "granola notes get NOTE_ID --include transcript --output json --compact",
                    "granola notes hydrate --since 7d --include-transcript --jsonl"
                ],
                "json_shapes": {
                    "list": { "notes": [], "count": 0, "has_more": false, "cursor": null, "page_size": 10 },
                    "hydrate": { "notes": [], "count": 0 }
                }
            },
            "folders": {
                "description": "List accessible folders for folder-scoped note queries.",
                "examples": ["granola folders list --output json --compact"],
                "json_shape": { "folders": [], "count": 0, "has_more": false, "cursor": null, "page_size": 10 }
            },
            "export": {
                "description": "Render notes to markdown, text, JSON, JSONL, transcript text, or one file per note.",
                "examples": [
                    "granola export note NOTE_ID --format markdown --include-transcript -o note.md",
                    "granola export notes --since 30d --format jsonl -o notes.jsonl",
                    "granola export notes --since 30d --format markdown --output-dir ./granola-notes --skip-existing"
                ]
            },
            "api": {
                "description": "Guarded GET access to documented /v1/... endpoints for debugging.",
                "examples": ["granola api get /v1/notes --query page_size=5 --output json --compact"]
            },
            "doctor": {
                "description": "Inspect local setup without printing secrets.",
                "examples": ["granola doctor --output json --compact"]
            }
        },
        "global_flags": {
            "--output json": "Use stable machine-readable output for data commands and JSON error objects on failure.",
            "--compact": "Remove JSON whitespace for lower token and byte usage.",
            "--fields a,b.c": "Project JSON to requested field paths where supported.",
            "--quiet": "Suppress non-essential human output.",
            "--api-key KEY": "Process-local auth override. Prefer keyring or --key-stdin for automation."
        },
        "exit_codes": { "success": 0, "general": 1, "not_found": 2, "auth": 3, "rate_limited": 4, "invalid_input": 5 },
        "error_shape": { "error": true, "message": "...", "code": 1, "details": {}, "retry_after": 1 },
        "recipes": [
            { "name": "check auth", "command": "granola auth status --validate --output json --compact" },
            { "name": "recent metadata", "command": "granola notes list --since 7d --sort updated-at --order desc --output json --compact" },
            { "name": "full recent notes", "command": "granola notes hydrate --since 7d --include-transcript --jsonl" },
            { "name": "small payload", "command": "granola notes list --output json --compact --fields notes.id,notes.title,notes.owner.email,count,has_more,cursor" }
        ],
        "constraints": [
            "credentials are keyring-only unless --api-key is provided for one process",
            "Granola public API uses API keys; OAuth is not implemented",
            "list endpoints are cursor-paginated and page_size is capped at 30",
            "the public API is read-only; this CLI does not mutate Granola data",
            "notes appear only after Granola has generated an AI summary and transcript",
            "respect rate limits: 25-request burst and 5 requests/second sustained"
        ]
    });

    if output.is_json() {
        return print_json(&data, output);
    }

    println!("Granola CLI agent guide");
    println!("- Use `granola auth status --validate --output json` to check auth.");
    println!(
        "- Use `granola notes list --output json --compact` for paginated note metadata envelopes."
    );
    println!(
        "- Use `granola notes get NOTE_ID --include transcript --output json` for full note data."
    );
    println!("- Use `granola notes hydrate --since 7d --include-transcript --jsonl` for efficient batch hydration.");
    println!("- Use `granola export note NOTE_ID --format markdown` for portable note output.");
    println!("- Use `--fields` to reduce JSON payloads, e.g. `--fields id,title,owner.email`.");
    println!(
        "- Re-run `granola agent --output json --compact` for the full machine-readable contract."
    );
    Ok(())
}

pub async fn doctor(
    command: DoctorCommand,
    api_key_override: Option<String>,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let keyring_available = crate::keyring::is_available();
    let keyring_configured = crate::keyring::get_key()?.is_some();
    let api_key_override_present = api_key_override.is_some();
    let resolved_api_key = resolve_api_key(api_key_override);
    let auth_resolves = resolved_api_key.is_ok();
    let config_path = crate::config::config_path()?;
    let cache_path = crate::cache::cache_path()?;
    let mut api_reachable = None;
    let mut auth_valid = None;
    let mut validation_error = None;

    if command.validate {
        match resolved_api_key {
            Ok(api_key) => match GranolaClient::new(api_key) {
                Ok(client) => match client
                    .list_notes(&ListNotesParams {
                        page_size: Some(1),
                        ..Default::default()
                    })
                    .await
                {
                    Ok(_) => {
                        api_reachable = Some(true);
                        auth_valid = Some(true);
                    }
                    Err(err) => {
                        api_reachable = Some(!matches!(err.kind, crate::error::ErrorKind::General));
                        auth_valid = Some(!matches!(err.kind, crate::error::ErrorKind::Auth));
                        validation_error = Some(err.message);
                    }
                },
                Err(err) => validation_error = Some(err.message),
            },
            Err(err) => {
                auth_valid = Some(false);
                validation_error = Some(err.message);
            }
        }
    }

    let mut next_steps = Vec::new();
    if !auth_resolves {
        next_steps
            .push("Run `granola auth login --validate` or pass `--api-key` for one invocation");
    }
    if auth_valid == Some(false) {
        next_steps.push(
            "Confirm the API key is active and that your Granola workspace supports API keys",
        );
    }
    if api_reachable == Some(false) {
        next_steps.push("Check network connectivity to https://public-api.granola.ai");
    }

    let data = json!({
        "version": env!("CARGO_PKG_VERSION"),
        "keyring_available": keyring_available,
        "keyring_configured": keyring_configured,
        "api_key_override_present": api_key_override_present,
        "auth_resolves": auth_resolves,
        "validated": command.validate,
        "api_reachable": api_reachable,
        "auth_valid": auth_valid,
        "validation_error": validation_error,
        "config_path": config_path,
        "cache_path": cache_path,
        "next_steps": next_steps,
    });

    if output.is_json() {
        return print_json(&data, output);
    }

    println!("keyring available: {keyring_available}");
    println!("keyring configured: {keyring_configured}");
    println!("api key override present: {api_key_override_present}");
    println!("auth resolves: {auth_resolves}");
    println!("config path: {}", config_path.display());
    println!("cache path: {}", cache_path.display());
    if command.validate {
        println!("api reachable: {}", api_reachable.unwrap_or(false));
        println!("auth valid: {}", auth_valid.unwrap_or(false));
        if let Some(error) = validation_error {
            println!("validation error: {error}");
        }
    }
    for step in next_steps {
        println!("next step: {step}");
    }
    Ok(())
}
