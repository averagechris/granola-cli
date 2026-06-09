use crate::config;
use crate::error::CliError;
use crate::output::{print_json, OutputOptions};
use clap::{Args, Subcommand};
use serde_json::json;

#[derive(Debug, Args)]
pub struct ConfigCommand {
    #[command(subcommand)]
    command: ConfigSubcommand,
}

#[derive(Debug, Subcommand)]
enum ConfigSubcommand {
    /// Print non-secret CLI configuration.
    Show,
    /// Set a non-secret default or profile value.
    Set { key: String, value: String },
    /// Print the config file path.
    Path,
}

pub fn handle(command: ConfigCommand, output: &OutputOptions) -> Result<(), CliError> {
    match command.command {
        ConfigSubcommand::Show => show(output),
        ConfigSubcommand::Set { key, value } => set(key, value, output),
        ConfigSubcommand::Path => path(output),
    }
}

fn show(output: &OutputOptions) -> Result<(), CliError> {
    let path = config::config_path()?;
    let config = config::load()?;
    if output.is_json() {
        return print_json(&json!({ "path": path, "config": config }), output);
    }
    println!("config path: {}", path.display());
    println!("{}", toml::to_string_pretty(&config).unwrap_or_default());
    Ok(())
}

fn set(key: String, value: String, output: &OutputOptions) -> Result<(), CliError> {
    let mut config = config::load()?;
    config::set_value(&mut config, &key, &value)?;
    config::save(&config)?;
    let path = config::config_path()?;
    if output.is_json() {
        return print_json(
            &json!({ "path": path, "key": key, "value": value, "written": true }),
            output,
        );
    }
    if !output.quiet {
        println!("set {key} in {}", path.display());
    }
    Ok(())
}

fn path(output: &OutputOptions) -> Result<(), CliError> {
    let path = config::config_path()?;
    if output.is_json() {
        return print_json(&json!({ "path": path }), output);
    }
    println!("{}", path.display());
    Ok(())
}
