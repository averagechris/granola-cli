use crate::error::CliError;
use crate::OutputFormat;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserConfig {
    #[serde(default)]
    pub defaults: ConfigProfile,
    #[serde(default)]
    pub profiles: BTreeMap<String, ConfigProfile>,
    #[serde(default)]
    pub views: BTreeMap<String, SavedView>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfigProfile {
    pub output: Option<String>,
    pub quiet: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SavedView {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_before: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_after: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_after: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_since: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub all: bool,
}

impl UserConfig {
    pub fn effective_profile(&self, profile: Option<&str>) -> Result<ConfigProfile, CliError> {
        let mut effective = self.defaults.clone();
        if let Some(name) = profile {
            let Some(profile) = self.profiles.get(name) else {
                return Err(CliError::invalid_input(format!(
                    "profile '{name}' is not configured; run `granola config show --output json`"
                )));
            };
            effective.merge(profile);
        }
        Ok(effective)
    }
}

impl ConfigProfile {
    fn merge(&mut self, other: &ConfigProfile) {
        if other.output.is_some() {
            self.output = other.output.clone();
        }
        if other.quiet.is_some() {
            self.quiet = other.quiet;
        }
    }

    pub fn output_format(&self) -> Result<Option<OutputFormat>, CliError> {
        match self.output.as_deref() {
            None => Ok(None),
            Some("table") => Ok(Some(OutputFormat::Table)),
            Some("list") => Ok(Some(OutputFormat::List)),
            Some("json") => Ok(Some(OutputFormat::Json)),
            Some("json-compact") => Ok(Some(OutputFormat::JsonCompact)),
            Some("json-pretty") => Ok(Some(OutputFormat::JsonPretty)),
            Some(other) => Err(CliError::invalid_input(format!(
                "invalid configured output '{other}'; use table, list, json, json-compact, or json-pretty"
            ))),
        }
    }
}

pub fn config_path() -> Result<PathBuf, CliError> {
    let base = dirs::config_dir()
        .ok_or_else(|| CliError::general("could not determine the OS config directory"))?;
    Ok(base.join("granola-cli").join("config.toml"))
}

pub fn load() -> Result<UserConfig, CliError> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(UserConfig::default());
    }
    let content = fs::read_to_string(&path).map_err(|error| {
        CliError::general(format!("failed to read config {}: {error}", path.display()))
    })?;
    toml::from_str(&content).map_err(|error| {
        CliError::general(format!(
            "failed to parse config {}: {error}",
            path.display()
        ))
    })
}

pub fn save(config: &UserConfig) -> Result<(), CliError> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            CliError::general(format!(
                "failed to create config directory {}: {error}",
                parent.display()
            ))
        })?;
    }
    let content = toml::to_string_pretty(config)
        .map_err(|error| CliError::general(format!("failed to render config TOML: {error}")))?;
    fs::write(&path, content).map_err(|error| {
        CliError::general(format!(
            "failed to write config {}: {error}",
            path.display()
        ))
    })
}

pub fn set_value(config: &mut UserConfig, key: &str, value: &str) -> Result<(), CliError> {
    let (profile, key) = if let Some(rest) = key.strip_prefix("profile.") {
        let mut parts = rest.splitn(2, '.');
        let name = parts.next().unwrap_or_default();
        let key = parts.next().ok_or_else(|| {
            CliError::invalid_input("profile keys must look like profile.NAME.output")
        })?;
        if name.trim().is_empty() {
            return Err(CliError::invalid_input("profile name cannot be empty"));
        }
        (config.profiles.entry(name.to_string()).or_default(), key)
    } else {
        (&mut config.defaults, key)
    };

    match key {
        "output" => {
            if !matches!(
                value,
                "table" | "list" | "json" | "json-compact" | "json-pretty"
            ) {
                return Err(CliError::invalid_input(
                    "output must be table, list, json, json-compact, or json-pretty",
                ));
            }
            profile.output = Some(value.to_string());
        }
        "quiet" => profile.quiet = Some(parse_bool(value, "quiet")?),
        _ => {
            return Err(CliError::invalid_input(
                "supported config keys are output, quiet, or profile.NAME.output/quiet",
            ));
        }
    }
    Ok(())
}

fn parse_bool(value: &str, key: &str) -> Result<bool, CliError> {
    match value {
        "true" | "yes" | "1" => Ok(true),
        "false" | "no" | "0" => Ok(false),
        _ => Err(CliError::invalid_input(format!(
            "{key} must be true or false"
        ))),
    }
}
