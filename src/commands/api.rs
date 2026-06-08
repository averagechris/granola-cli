use crate::api::{resolve_api_key, GranolaClient};
use crate::error::CliError;
use crate::output::{print_json, OutputOptions};
use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub struct ApiCommand {
    #[command(subcommand)]
    command: ApiSubcommand,
}

#[derive(Debug, Subcommand)]
enum ApiSubcommand {
    /// Make a guarded GET request to a documented Granola API path.
    Get {
        /// API path. Must start with /v1/ and must not include a URL scheme or query string.
        path: String,
        /// Query parameter as key=value. Repeat for multiple query params.
        #[arg(long = "query", value_name = "KEY=VALUE")]
        query: Vec<String>,
    },
}

pub async fn handle(
    command: ApiCommand,
    api_key_override: Option<String>,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let api_key = resolve_api_key(api_key_override)?;
    let client = GranolaClient::new(api_key)?;

    match command.command {
        ApiSubcommand::Get { path, query } => {
            let path = validate_api_path(&path)?;
            let query = parse_query_params(&query)?;
            let value = client.get_raw_json(path, query).await?;
            print_json(&value, output)
        }
    }
}

fn validate_api_path(path: &str) -> Result<&str, CliError> {
    if path.contains("://") || path.contains('?') || !path.starts_with("/v1/") {
        return Err(CliError::invalid_input(
            "raw API path must start with /v1/ and must not include a scheme or query string",
        ));
    }
    Ok(path)
}

fn parse_query_params(params: &[String]) -> Result<Vec<(String, String)>, CliError> {
    params
        .iter()
        .map(|param| {
            let (key, value) = param.split_once('=').ok_or_else(|| {
                CliError::invalid_input(format!(
                    "invalid query parameter '{param}'; expected key=value"
                ))
            })?;
            if key.trim().is_empty() {
                return Err(CliError::invalid_input(format!(
                    "invalid query parameter '{param}'; key cannot be empty"
                )));
            }
            Ok((key.to_string(), value.to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_raw_api_paths() {
        assert_eq!(validate_api_path("/v1/notes").unwrap(), "/v1/notes");
        assert!(validate_api_path("https://public-api.granola.ai/v1/notes").is_err());
        assert!(validate_api_path("/v2/notes").is_err());
        assert!(validate_api_path("/v1/notes?page_size=1").is_err());
    }

    #[test]
    fn parses_query_params() {
        let params =
            parse_query_params(&["page_size=5".to_string(), "cursor=abc".to_string()]).unwrap();
        assert_eq!(
            params,
            vec![
                ("page_size".to_string(), "5".to_string()),
                ("cursor".to_string(), "abc".to_string())
            ]
        );
        assert!(parse_query_params(&["bad".to_string()]).is_err());
        assert!(parse_query_params(&["=value".to_string()]).is_err());
    }
}
