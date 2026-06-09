mod api;
mod cache;
mod commands;
mod config;
mod error;
mod keyring;
mod output;
mod types;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use error::CliError;
use output::{emit_error_json, OutputOptions};
use std::io;

#[derive(Debug, Parser)]
#[command(name = "granola")]
#[command(about = "A Rust CLI for Granola meeting notes")]
struct Cli {
    /// Output format for data commands.
    #[arg(long, value_enum, global = true)]
    output: Option<OutputFormat>,

    /// Emit compact JSON without whitespace.
    #[arg(long, global = true)]
    compact: bool,

    /// Limit JSON output to comma-separated field paths.
    #[arg(long, value_delimiter = ',', global = true)]
    fields: Vec<String>,

    /// Suppress non-essential human output.
    #[arg(short, long, global = true)]
    quiet: bool,

    /// Apply a named non-secret config profile.
    #[arg(long, global = true)]
    profile: Option<String>,

    /// Use an API key for this invocation only. The key is not stored.
    #[arg(long, global = true, value_name = "KEY")]
    api_key: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    #[default]
    Table,
    Json,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Manage Granola API authentication.
    Auth(commands::auth::AuthCommand),
    /// Make guarded raw Granola API requests.
    Api(commands::api::ApiCommand),
    /// List and retrieve Granola notes.
    Notes(commands::notes::NotesCommand),
    /// List accessible Granola folders.
    Folders(commands::folders::FoldersCommand),
    /// List notes from the last 7 days.
    Recent,
    /// List notes created today.
    Today,
    /// List notes created yesterday.
    Yesterday,
    /// Fetch the most recently updated note.
    Last(commands::shortcuts::LastCommand),
    /// Search the local note cache.
    Search { query: String },
    /// Show one note by ID.
    Show { note_id: String },
    /// Open one note in the browser.
    Open { note_id: String },
    /// Export notes and transcripts.
    Export(commands::export::ExportCommand),
    /// Sync notes into the local non-secret cache.
    Sync(commands::sync::SyncCommand),
    /// Inspect or clear the local non-secret cache.
    Cache(commands::cache::CacheCommand),
    /// Manage non-secret CLI defaults and profiles.
    Config(commands::config::ConfigCommand),
    /// Print agent-focused usage guidance.
    Agent,
    /// Generate shell completion scripts.
    Completions {
        /// Shell to generate completions for.
        shell: Shell,
    },
    /// Check local CLI configuration and credential availability.
    Doctor(commands::DoctorCommand),
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let effective_config =
        match config::load().and_then(|config| config.effective_profile(cli.profile.as_deref())) {
            Ok(config) => config,
            Err(err) => {
                eprintln!("error: {err}");
                std::process::exit(err.code().into());
            }
        };
    let configured_output = match effective_config.output_format() {
        Ok(format) => format,
        Err(err) => {
            eprintln!("error: {err}");
            std::process::exit(err.code().into());
        }
    };
    let output_format = cli
        .output
        .or(configured_output)
        .unwrap_or(OutputFormat::Table);
    let compact = if arg_present("--compact") {
        cli.compact
    } else {
        effective_config.compact.unwrap_or(cli.compact)
    };
    let quiet = if arg_present("--quiet") || arg_present("-q") {
        cli.quiet
    } else {
        effective_config.quiet.unwrap_or(cli.quiet)
    };
    let output = OutputOptions::new(output_format, compact, cli.fields, quiet);

    if let Err(err) = run(cli.command, cli.api_key, &output).await {
        if output.is_json() {
            emit_error_json(&err, output.compact);
        } else {
            eprintln!("error: {err}");
        }
        std::process::exit(err.code().into());
    }
}

async fn run(
    command: Command,
    api_key_override: Option<String>,
    output: &OutputOptions,
) -> Result<(), CliError> {
    match command {
        Command::Auth(command) => commands::auth::handle(command, api_key_override, output).await,
        Command::Api(command) => commands::api::handle(command, api_key_override, output).await,
        Command::Notes(command) => commands::notes::handle(command, api_key_override, output).await,
        Command::Folders(command) => {
            commands::folders::handle(command, api_key_override, output).await
        }
        Command::Recent => commands::shortcuts::recent(api_key_override, output).await,
        Command::Today => commands::shortcuts::today(api_key_override, output).await,
        Command::Yesterday => commands::shortcuts::yesterday(api_key_override, output).await,
        Command::Last(command) => {
            commands::shortcuts::last(command, api_key_override, output).await
        }
        Command::Search { query } => commands::shortcuts::search(query, output),
        Command::Show { note_id } => {
            commands::shortcuts::show(note_id, api_key_override, output).await
        }
        Command::Open { note_id } => {
            commands::shortcuts::open(note_id, api_key_override, output).await
        }
        Command::Export(command) => {
            commands::export::handle(command, api_key_override, output).await
        }
        Command::Sync(command) => commands::sync::handle(command, api_key_override, output).await,
        Command::Cache(command) => commands::cache::handle(command, output),
        Command::Config(command) => commands::config::handle(command, output),
        Command::Agent => commands::agent(output),
        Command::Completions { shell } => {
            let mut command = Cli::command();
            let name = command.get_name().to_string();
            clap_complete::generate(shell, &mut command, name, &mut io::stdout());
            Ok(())
        }
        Command::Doctor(command) => commands::doctor(command, api_key_override, output).await,
    }
}

fn arg_present(flag: &str) -> bool {
    std::env::args_os().any(|arg| arg == flag)
}
