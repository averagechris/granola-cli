mod api;
mod cache;
mod commands;
mod config;
mod error;
mod keyring;
mod note_ref;
mod output;
mod redaction;
mod types;

use cache::CacheMode;
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use error::CliError;
use output::{emit_error_json, OutputOptions};
use std::io;

#[derive(Debug, Parser)]
#[command(name = "granola")]
#[command(version)]
#[command(about = "A Rust CLI for Granola meeting notes")]
struct Cli {
    /// Output format for data commands.
    #[arg(long, value_enum, global = true)]
    output: Option<OutputFormat>,

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

    /// Skip reading from the local note cache for read-through commands.
    #[arg(long, global = true)]
    no_cache: bool,

    /// Disable automatic write-through updates to the local note cache.
    #[arg(long, global = true)]
    no_cache_write: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    #[default]
    Table,
    List,
    Json,
    JsonCompact,
    JsonPretty,
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
    #[command(
        long_about = "Search the local SQLite FTS index. Supports SQLite FTS5 syntax such as field filters and phrases. Examples: `granola search apple`, `granola search attendees:will async config`, `granola search attendees:will \"async config\"`, `granola search transcript:renewal`."
    )]
    Search {
        /// Read additional search query text from stdin. If QUERY is omitted and stdin is piped, stdin is read automatically.
        #[arg(long)]
        stdin: bool,
        /// Search query. Multiple arguments are joined, so `granola search attendees:will "async config"` works without quoting the entire query.
        #[arg(value_name = "QUERY", num_args = 0..)]
        query: Vec<String>,
    },
    /// Show one note by ID.
    Show { note_id: String },
    /// Open one note in the browser.
    Open { note_id: String },
    /// Export notes and transcripts.
    Export(commands::export::ExportCommand),
    /// Summarize recent note metadata for review.
    Digest(commands::digest::DigestCommand),
    /// Build a bounded note bundle for agents.
    Context(commands::context::ContextCommand),
    /// Poll for newly visible or updated notes.
    Watch(commands::watch::WatchCommand),
    /// Sync notes into the local non-secret cache.
    Sync(commands::sync::SyncCommand),
    /// Inspect or clear the local non-secret cache.
    Cache(commands::cache::CacheCommand),
    /// Manage non-secret CLI defaults and profiles.
    Config(commands::config::ConfigCommand),
    /// Save and run named note views.
    Views(commands::views::ViewsCommand),
    /// Print agent-focused usage guidance.
    Agent,
    /// Generate shell completion scripts.
    Completions {
        /// Shell to generate completions for.
        shell: Option<Shell>,
        #[command(subcommand)]
        command: Option<CompletionsSubcommand>,
    },
    /// Check local CLI configuration and credential availability.
    Doctor(commands::DoctorCommand),
}

#[derive(Debug, Subcommand)]
enum CompletionsSubcommand {
    /// Print shell-specific completion installation instructions.
    Install { shell: Shell },
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
    let output_format_explicit = arg_present("--output") || configured_output.is_some();
    let quiet = if arg_present("--quiet") || arg_present("-q") {
        cli.quiet
    } else {
        effective_config.quiet.unwrap_or(cli.quiet)
    };
    let output = OutputOptions::new(output_format, output_format_explicit, cli.fields, quiet);

    let cache_mode = CacheMode::new(!cli.no_cache, !cli.no_cache_write);
    if let Err(err) = run(cli.command, cli.api_key, cache_mode, &output).await {
        if output.is_json() {
            emit_error_json(&err, output.json_compact());
        } else {
            eprintln!("error: {err}");
        }
        std::process::exit(err.code().into());
    }
}

async fn run(
    command: Command,
    api_key_override: Option<String>,
    cache_mode: CacheMode,
    output: &OutputOptions,
) -> Result<(), CliError> {
    match command {
        Command::Auth(command) => commands::auth::handle(command, api_key_override, output).await,
        Command::Api(command) => commands::api::handle(command, api_key_override, output).await,
        Command::Notes(command) => {
            commands::notes::handle(command, api_key_override, cache_mode, output).await
        }
        Command::Folders(command) => {
            commands::folders::handle(command, api_key_override, output).await
        }
        Command::Recent => {
            commands::shortcuts::recent(api_key_override, cache_mode.write, output).await
        }
        Command::Today => {
            commands::shortcuts::today(api_key_override, cache_mode.write, output).await
        }
        Command::Yesterday => {
            commands::shortcuts::yesterday(api_key_override, cache_mode.write, output).await
        }
        Command::Last(command) => {
            commands::shortcuts::last(command, api_key_override, cache_mode, output).await
        }
        Command::Search { query, stdin } => {
            commands::shortcuts::search(query, stdin, api_key_override, output).await
        }
        Command::Show { note_id } => {
            commands::shortcuts::show(note_id, api_key_override, cache_mode, output).await
        }
        Command::Open { note_id } => {
            commands::shortcuts::open(note_id, api_key_override, cache_mode, output).await
        }
        Command::Export(command) => {
            commands::export::handle(command, api_key_override, cache_mode, output).await
        }
        Command::Digest(command) => {
            commands::digest::handle(command, api_key_override, output).await
        }
        Command::Context(command) => {
            commands::context::handle(command, api_key_override, output).await
        }
        Command::Watch(command) => commands::watch::handle(command, api_key_override, output).await,
        Command::Sync(command) => commands::sync::handle(command, api_key_override, output).await,
        Command::Cache(command) => commands::cache::handle(command, output),
        Command::Config(command) => commands::config::handle(command, output),
        Command::Views(command) => commands::views::handle(command, api_key_override, output).await,
        Command::Agent => commands::agent(output),
        Command::Completions { shell, command } => handle_completions(shell, command),
        Command::Doctor(command) => commands::doctor(command, api_key_override, output).await,
    }
}

fn arg_present(flag: &str) -> bool {
    std::env::args_os().any(|arg| arg == flag)
}

fn handle_completions(
    shell: Option<Shell>,
    command: Option<CompletionsSubcommand>,
) -> Result<(), CliError> {
    match (shell, command) {
        (Some(shell), None) => {
            let mut command = Cli::command();
            let name = command.get_name().to_string();
            clap_complete::generate(shell, &mut command, name, &mut io::stdout());
            Ok(())
        }
        (None, Some(CompletionsSubcommand::Install { shell })) => {
            print_completion_install(shell);
            Ok(())
        }
        _ => Err(CliError::invalid_input(
            "use `granola completions SHELL` or `granola completions install SHELL`",
        )),
    }
}

fn print_completion_install(shell: Shell) {
    match shell {
        Shell::Zsh => {
            println!("mkdir -p ~/.zfunc");
            println!("granola completions zsh > ~/.zfunc/_granola");
            println!("# Add to ~/.zshrc if needed: fpath=(~/.zfunc $fpath); autoload -Uz compinit; compinit");
        }
        Shell::Bash => {
            println!("mkdir -p ~/.local/share/bash-completion/completions");
            println!(
                "granola completions bash > ~/.local/share/bash-completion/completions/granola"
            );
        }
        Shell::Fish => {
            println!("mkdir -p ~/.config/fish/completions");
            println!("granola completions fish > ~/.config/fish/completions/granola.fish");
        }
        Shell::PowerShell => {
            println!("granola completions powershell >> $PROFILE");
        }
        Shell::Elvish => {
            println!("mkdir -p ~/.elvish/lib");
            println!("granola completions elvish > ~/.elvish/lib/granola.elv");
        }
        _ => {
            println!("granola completions {shell} > /path/to/your/completion/file");
        }
    }
}
