use crate::cache;
use crate::error::CliError;
use crate::output::{print_json, OutputOptions};
use clap::{Args, Subcommand};
use serde_json::json;

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
    let path = cache::cache_path()?;
    let cache = cache::load()?;
    let data = json!({
        "path": path,
        "exists": cache.is_some(),
        "synced_at": cache.as_ref().map(|cache| cache.synced_at.as_str()),
        "notes": cache.as_ref().map_or(0, |cache| cache.notes.len()),
    });

    if output.is_json() {
        return print_json(&data, output);
    }

    println!("cache path: {}", data["path"].as_str().unwrap_or(""));
    println!("cache exists: {}", data["exists"]);
    println!("notes: {}", data["notes"]);
    if let Some(synced_at) = data["synced_at"].as_str() {
        println!("synced at: {synced_at}");
    }
    Ok(())
}

fn clear(output: &OutputOptions) -> Result<(), CliError> {
    let path = cache::cache_path()?;
    let removed = cache::clear()?;
    let data = json!({ "path": path, "removed": removed });

    if output.is_json() {
        return print_json(&data, output);
    }

    if !output.quiet {
        if removed {
            println!("removed cache {}", path.display());
        } else {
            println!("cache did not exist at {}", path.display());
        }
    }
    Ok(())
}
