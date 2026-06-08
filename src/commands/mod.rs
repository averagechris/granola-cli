pub mod auth;
pub mod folders;
pub mod notes;

use crate::api::resolve_api_key;
use crate::error::CliError;
use crate::output::{print_json, OutputOptions};
use serde_json::json;

pub fn agent(output: &OutputOptions) -> Result<(), CliError> {
    let data = json!({
        "commands": {
            "auth": ["granola auth login", "granola auth status --validate", "granola auth logout"],
            "notes": ["granola notes list --output json", "granola notes get NOTE_ID --include transcript --output json"],
            "folders": ["granola folders list --output json"]
        },
        "agent_flags": ["--output json", "--compact", "--fields id,title,owner.email", "--quiet", "--api-key KEY"],
        "constraints": [
            "credentials are keyring-only unless --api-key is provided for one process",
            "Granola public API uses API keys; OAuth is not implemented",
            "list endpoints are cursor-paginated and page_size is capped at 30"
        ]
    });

    if output.is_json() {
        return print_json(&data, output);
    }

    println!("Granola CLI agent guide");
    println!("- Use `granola auth status --validate --output json` to check auth.");
    println!("- Use `granola notes list --output json --compact` for machine-readable note lists.");
    println!(
        "- Use `granola notes get NOTE_ID --include transcript --output json` for full note data."
    );
    println!("- Use `--fields` to reduce JSON payloads, e.g. `--fields id,title,owner.email`.");
    Ok(())
}

pub fn doctor(api_key_override: Option<String>, output: &OutputOptions) -> Result<(), CliError> {
    let keyring_available = crate::keyring::is_available();
    let keyring_configured = crate::keyring::get_key()?.is_some();
    let api_key_override_present = api_key_override.is_some();
    let auth_resolves = resolve_api_key(api_key_override).is_ok();

    let data = json!({
        "keyring_available": keyring_available,
        "keyring_configured": keyring_configured,
        "api_key_override_present": api_key_override_present,
        "auth_resolves": auth_resolves,
    });

    if output.is_json() {
        return print_json(&data, output);
    }

    println!("keyring available: {keyring_available}");
    println!("keyring configured: {keyring_configured}");
    println!("api key override present: {api_key_override_present}");
    println!("auth resolves: {auth_resolves}");
    Ok(())
}
