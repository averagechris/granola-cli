use crate::error::CliError;
use crate::OutputFormat;
use serde::Serialize;
use serde_json::{json, Map, Value};

#[derive(Debug, Clone)]
pub struct OutputOptions {
    pub format: OutputFormat,
    pub compact: bool,
    pub fields: Vec<String>,
    pub quiet: bool,
}

impl OutputOptions {
    pub fn new(format: OutputFormat, compact: bool, fields: Vec<String>, quiet: bool) -> Self {
        Self {
            format,
            compact,
            fields,
            quiet,
        }
    }

    pub fn is_json(&self) -> bool {
        self.format == OutputFormat::Json
    }
}

pub fn print_json<T: Serialize>(value: &T, output: &OutputOptions) -> Result<(), CliError> {
    let mut value = serde_json::to_value(value)?;

    if !output.fields.is_empty() {
        value = select_fields(&value, &output.fields);
    }

    let text = if output.compact {
        serde_json::to_string(&value)?
    } else {
        serde_json::to_string_pretty(&value)?
    };
    println!("{text}");
    Ok(())
}

pub fn emit_error_json(error: &CliError, compact: bool) {
    let mut value = json!({
        "error": true,
        "message": error.message,
        "code": error.code(),
    });

    if let Some(details) = &error.details {
        value["details"] = details.clone();
    }
    if let Some(retry_after) = error.retry_after {
        value["retry_after"] = json!(retry_after);
    }

    let rendered = if compact {
        serde_json::to_string(&value)
    } else {
        serde_json::to_string_pretty(&value)
    };

    match rendered {
        Ok(text) => eprintln!("{text}"),
        Err(_) => eprintln!("{{\"error\":true,\"message\":\"failed to render error\"}}"),
    }
}

fn select_fields(value: &Value, fields: &[String]) -> Value {
    match value {
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| select_object(item, fields))
                .collect(),
        ),
        Value::Object(_) => select_object(value, fields),
        other => other.clone(),
    }
}

fn select_object(value: &Value, fields: &[String]) -> Value {
    let mut out = Map::new();

    for field in fields {
        let path: Vec<&str> = field.split('.').filter(|part| !part.is_empty()).collect();
        if path.is_empty() {
            continue;
        }
        if let Some(selected) = get_path(value, &path) {
            insert_path(&mut out, &path, selected.clone());
        }
    }

    Value::Object(out)
}

fn get_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for part in path {
        current = current.get(*part)?;
    }
    Some(current)
}

fn insert_path(out: &mut Map<String, Value>, path: &[&str], value: Value) {
    if path.len() == 1 {
        out.insert(path[0].to_string(), value);
        return;
    }

    let entry = out
        .entry(path[0].to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if let Value::Object(map) = entry {
        insert_path(map, &path[1..], value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn selects_nested_fields_for_arrays() {
        let value =
            json!([{ "id": "not_123", "owner": { "email": "a@example.com" }, "title": "A" }]);
        let selected = select_fields(&value, &["id".to_string(), "owner.email".to_string()]);
        assert_eq!(
            selected,
            json!([{ "id": "not_123", "owner": { "email": "a@example.com" } }])
        );
    }
}
