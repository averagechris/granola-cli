use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "granola")]
#[command(about = "A Rust CLI for Granola meeting notes")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Show planned CLI surface for the first implementation pass.
    Plan,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Plan) => {
            println!("planned: auth, notes, folders, export, completions, doctor, agent");
        }
        None => {
            println!("granola-cli scaffold initialized. See docs/plan.md for implementation work.");
        }
    }
}
