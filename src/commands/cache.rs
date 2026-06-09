use crate::cache;
use crate::error::CliError;
use crate::output::{print_json, OutputOptions};
use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub struct CacheCommand {
    #[command(subcommand)]
    command: CacheSubcommand,
}

#[derive(Debug, Subcommand)]
enum CacheSubcommand {
    /// Show local note cache status.
    Status,
    /// Remove the local note cache.
    Clear,
}

pub fn handle(command: CacheCommand, output: &OutputOptions) -> Result<(), CliError> {
    match command.command {
        CacheSubcommand::Status => status(output),
        CacheSubcommand::Clear => clear(output),
    }
}

fn status(output: &OutputOptions) -> Result<(), CliError> {
    let status = cache::status()?;

    if output.is_json() {
        return print_json(&status, output);
    }

    println!("cache path: {}", status.path.display());
    println!("cache exists: {}", status.exists);
    println!(
        "legacy JSON cache path: {}",
        status.legacy_json_path.display()
    );
    println!("legacy JSON cache exists: {}", status.legacy_json_exists);
    println!("summaries: {}", status.summaries);
    println!("hydrated notes: {}", status.hydrated_notes);
    println!("transcript notes: {}", status.transcript_notes);
    if let Some(synced_at) = status.synced_at {
        println!("synced at: {synced_at}");
    }
    Ok(())
}

fn clear(output: &OutputOptions) -> Result<(), CliError> {
    let path = cache::cache_path()?;
    let legacy_path = cache::legacy_cache_path()?;
    let removed = cache::clear()?;
    let data =
        serde_json::json!({ "path": path, "legacy_json_path": legacy_path, "removed": removed });

    if output.is_json() {
        return print_json(&data, output);
    }

    if !output.quiet {
        if removed {
            println!("removed cache {}", path.display());
            println!(
                "removed legacy JSON cache if present at {}",
                legacy_path.display()
            );
        } else {
            println!("cache did not exist at {}", path.display());
        }
    }
    Ok(())
}
