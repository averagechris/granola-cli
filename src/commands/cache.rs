use crate::cache;
use crate::error::CliError;
use crate::output::{print_json, OutputOptions};
use clap::{Args, Subcommand, ValueEnum};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct CacheCommand {
    #[command(subcommand)]
    command: CacheSubcommand,
}

#[derive(Debug, Subcommand)]
enum CacheSubcommand {
    /// Show local note cache status.
    Status {
        /// Print extra path and version details in human output.
        #[arg(long)]
        verbose: bool,
    },
    /// Print the local note cache path.
    Path,
    /// Remove the local note cache.
    Clear,
    /// Run SQLite integrity checks against the cache.
    Verify,
    /// Vacuum the SQLite cache file.
    Vacuum,
    /// Export cache contents.
    Export(CacheExportCommand),
}

#[derive(Debug, Args)]
struct CacheExportCommand {
    /// Export format.
    #[arg(long, value_enum, default_value_t = CacheExportFormat::Jsonl)]
    format: CacheExportFormat,
    /// Write to this file instead of stdout.
    #[arg(short, long)]
    output_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CacheExportFormat {
    Jsonl,
}

pub fn handle(command: CacheCommand, output: &OutputOptions) -> Result<(), CliError> {
    match command.command {
        CacheSubcommand::Status { verbose } => status(verbose, output),
        CacheSubcommand::Path => path(output),
        CacheSubcommand::Clear => clear(output),
        CacheSubcommand::Verify => verify(output),
        CacheSubcommand::Vacuum => vacuum(output),
        CacheSubcommand::Export(command) => export(command, output),
    }
}

fn status(verbose: bool, output: &OutputOptions) -> Result<(), CliError> {
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
    if verbose {
        println!(
            "version: {}",
            status
                .version
                .map_or_else(|| "unknown".to_string(), |v| v.to_string())
        );
    }
    if let Some(synced_at) = status.synced_at {
        println!("synced at: {synced_at}");
    }
    Ok(())
}

fn path(output: &OutputOptions) -> Result<(), CliError> {
    let path = cache::cache_path()?;
    if output.is_json() {
        return print_json(&serde_json::json!({ "path": path }), output);
    }
    println!("{}", path.display());
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

fn verify(output: &OutputOptions) -> Result<(), CliError> {
    let result = cache::verify()?;
    if output.is_json() {
        return print_json(&result, output);
    }
    println!("cache path: {}", result.path.display());
    println!("cache exists: {}", result.exists);
    println!(
        "integrity: {}",
        result.integrity_check.as_deref().unwrap_or("not checked")
    );
    println!("ok: {}", result.ok);
    Ok(())
}

fn vacuum(output: &OutputOptions) -> Result<(), CliError> {
    let status = cache::vacuum()?;
    if output.is_json() {
        return print_json(&status, output);
    }
    if !output.quiet {
        println!("vacuumed cache {}", status.path.display());
    }
    Ok(())
}

fn export(command: CacheExportCommand, _output: &OutputOptions) -> Result<(), CliError> {
    let content = match command.format {
        CacheExportFormat::Jsonl => cache::export_jsonl()?.ok_or_else(|| {
            CliError::invalid_input(
                "no local cache found; run `granola sync --since 30d --all --include-transcript` first",
            )
        })?,
    };
    if let Some(path) = command.output_file {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                CliError::general(format!(
                    "failed to create output directory {}: {error}",
                    parent.display()
                ))
            })?;
        }
        fs::write(&path, content).map_err(|error| {
            CliError::general(format!("failed to write {}: {error}", path.display()))
        })?;
        return Ok(());
    }
    print!("{content}");
    Ok(())
}
