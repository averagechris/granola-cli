mod api;
mod commands;
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
    #[arg(long, value_enum, default_value_t = OutputFormat::Table, global = true)]
    output: OutputFormat,

    /// Emit compact JSON without whitespace.
    #[arg(long, global = true)]
    compact: bool,

    /// Limit JSON output to comma-separated field paths.
    #[arg(long, value_delimiter = ',', global = true)]
    fields: Vec<String>,

    /// Suppress non-essential human output.
    #[arg(short, long, global = true)]
    quiet: bool,

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
    /// List and retrieve Granola notes.
    Notes(commands::notes::NotesCommand),
    /// List accessible Granola folders.
    Folders(commands::folders::FoldersCommand),
    /// Export notes and transcripts.
    Export(commands::export::ExportCommand),
    /// Print agent-focused usage guidance.
    Agent,
    /// Generate shell completion scripts.
    Completions {
        /// Shell to generate completions for.
        shell: Shell,
    },
    /// Check local CLI configuration and credential availability.
    Doctor,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let output = OutputOptions::new(cli.output, cli.compact, cli.fields, cli.quiet);

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
        Command::Notes(command) => commands::notes::handle(command, api_key_override, output).await,
        Command::Folders(command) => {
            commands::folders::handle(command, api_key_override, output).await
        }
        Command::Export(command) => {
            commands::export::handle(command, api_key_override, output).await
        }
        Command::Agent => commands::agent(output),
        Command::Completions { shell } => {
            let mut command = Cli::command();
            let name = command.get_name().to_string();
            clap_complete::generate(shell, &mut command, name, &mut io::stdout());
            Ok(())
        }
        Command::Doctor => commands::doctor(api_key_override, output),
    }
}
