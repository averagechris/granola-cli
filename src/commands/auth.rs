use crate::api::{resolve_api_key, GranolaClient, ListFoldersParams};
use crate::error::CliError;
use crate::output::{print_json, OutputOptions};
use clap::{Args, Subcommand};
use dialoguer::{Confirm, Password};
use serde_json::json;
use std::io::Read;

#[derive(Debug, Args)]
pub struct AuthCommand {
    #[command(subcommand)]
    command: AuthSubcommand,
}

#[derive(Debug, Subcommand)]
enum AuthSubcommand {
    /// Store a Granola API key in the OS keyring.
    Login {
        /// API key to store. Omit to prompt securely.
        #[arg(long, value_name = "KEY", conflicts_with = "key_stdin")]
        key: Option<String>,
        /// Read the API key from stdin. Preferred for automation to avoid argv leaks.
        #[arg(long, conflicts_with = "key")]
        key_stdin: bool,
        /// Validate the API key before saving it.
        #[arg(long)]
        validate: bool,
    },
    /// Remove the stored Granola API key.
    Logout {
        /// Skip confirmation.
        #[arg(long)]
        force: bool,
    },
    /// Show authentication status.
    Status {
        /// Validate API access with a small request.
        #[arg(long)]
        validate: bool,
    },
}

pub async fn handle(
    command: AuthCommand,
    api_key_override: Option<String>,
    output: &OutputOptions,
) -> Result<(), CliError> {
    match command.command {
        AuthSubcommand::Login {
            key,
            key_stdin,
            validate,
        } => login(key, key_stdin, validate, output).await,
        AuthSubcommand::Logout { force } => logout(force, output),
        AuthSubcommand::Status { validate } => status(validate, api_key_override, output).await,
    }
}

async fn login(
    key: Option<String>,
    key_stdin: bool,
    validate: bool,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let api_key = if key_stdin {
        read_key_from_stdin()?
    } else {
        match key {
            Some(key) => key,
            None => Password::new()
                .with_prompt("Granola API key")
                .interact()
                .map_err(|error| CliError::general(format!("failed to read API key: {error}")))?,
        }
    };

    if api_key.trim().is_empty() {
        return Err(CliError::invalid_input("API key cannot be empty"));
    }

    if validate {
        validate_key(&api_key).await?;
    }

    crate::keyring::set_key(api_key.trim())?;

    let data = json!({ "saved": true, "storage": "keyring" });
    if output.is_json() {
        return print_json(&data, output);
    }

    if !output.quiet {
        println!("Granola API key saved to OS keyring");
    }
    Ok(())
}

fn read_key_from_stdin() -> Result<String, CliError> {
    let mut buffer = String::new();
    std::io::stdin()
        .read_to_string(&mut buffer)
        .map_err(|error| {
            CliError::general(format!("failed to read API key from stdin: {error}"))
        })?;
    Ok(buffer.trim().to_string())
}

fn logout(force: bool, output: &OutputOptions) -> Result<(), CliError> {
    if !force {
        let confirmed = Confirm::new()
            .with_prompt("Remove stored Granola API key?")
            .default(false)
            .interact()
            .map_err(|error| CliError::general(format!("failed to read confirmation: {error}")))?;
        if !confirmed {
            return Ok(());
        }
    }

    crate::keyring::delete_key()?;

    let data = json!({ "cleared": true });
    if output.is_json() {
        return print_json(&data, output);
    }

    if !output.quiet {
        println!("Removed stored Granola API key");
    }
    Ok(())
}

async fn status(
    validate: bool,
    api_key_override: Option<String>,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let keyring_available = crate::keyring::is_available();
    let keyring_configured = crate::keyring::get_key()?.is_some();
    let api_key_override_present = api_key_override.is_some();
    let configured = keyring_configured || api_key_override_present;
    let mut validated = None;

    if validate {
        validated = Some(match resolve_api_key(api_key_override) {
            Ok(api_key) => validate_key(&api_key).await.is_ok(),
            Err(_) => false,
        });
    }

    let data = json!({
        "configured": configured,
        "storage": if keyring_configured { "keyring" } else { "none" },
        "keyring_available": keyring_available,
        "api_key_override_present": api_key_override_present,
        "validated": validated,
    });

    if output.is_json() {
        return print_json(&data, output);
    }

    println!("configured: {configured}");
    println!(
        "storage: {}",
        if keyring_configured {
            "keyring"
        } else {
            "none"
        }
    );
    println!("keyring available: {keyring_available}");
    println!("api key override present: {api_key_override_present}");
    if let Some(validated) = validated {
        println!("validated: {validated}");
    }
    Ok(())
}

async fn validate_key(api_key: &str) -> Result<(), CliError> {
    let client = GranolaClient::new(api_key.to_string())?;
    let params = ListFoldersParams {
        cursor: None,
        page_size: Some(1),
    };
    client.list_folders(&params).await.map(|_| ())
}
