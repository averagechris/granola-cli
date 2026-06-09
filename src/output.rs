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
        insert_selected(&mut out, value, &path);
    }

    Value::Object(out)
}

fn insert_selected(out: &mut Map<String, Value>, source: &Value, path: &[&str]) {
    let Some((head, tail)) = path.split_first() else {
        return;
    };
    let Some(value) = source.get(*head) else {
        return;
    };

    if tail.is_empty() {
        out.insert((*head).to_string(), value.clone());
        return;
    }

    match value {
        Value::Object(_) => {
            let entry = out
                .entry((*head).to_string())
                .or_insert_with(|| Value::Object(Map::new()));
            if let Value::Object(map) = entry {
                insert_selected(map, value, tail);
            }
        }
        Value::Array(items) => {
            let projected_items: Vec<Value> = items
                .iter()
                .map(|item| {
                    let mut projected = Map::new();
                    insert_selected(&mut projected, item, tail);
                    Value::Object(projected)
                })
                .collect();

            match out.get_mut(*head) {
                Some(Value::Array(existing)) => merge_projected_arrays(existing, projected_items),
                _ => {
                    out.insert((*head).to_string(), Value::Array(projected_items));
                }
            }
        }
        _ => {}
    }
}

fn merge_projected_arrays(existing: &mut [Value], projected_items: Vec<Value>) {
    for (existing_item, projected_item) in existing.iter_mut().zip(projected_items) {
        let (Value::Object(existing_map), Value::Object(projected_map)) =
            (existing_item, projected_item)
        else {
            continue;
        };
        merge_maps(existing_map, projected_map);
    }
}

fn merge_maps(into: &mut Map<String, Value>, from: Map<String, Value>) {
    for (key, value) in from {
        match (into.get_mut(&key), value) {
            (Some(Value::Object(existing)), Value::Object(new)) => merge_maps(existing, new),
            (_, value) => {
                into.insert(key, value);
            }
        }
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

    #[test]
    fn selects_nested_fields_for_arrays_inside_envelopes() {
        let value = json!({
            "notes": [
                { "id": "not_123", "owner": { "email": "a@example.com" }, "title": "A" }
            ],
            "count": 1,
            "cursor": "next"
        });

        let selected = select_fields(
            &value,
            &[
                "notes.id".to_string(),
                "notes.owner.email".to_string(),
                "count".to_string(),
            ],
        );

        assert_eq!(
            selected,
            json!({
                "notes": [{ "id": "not_123", "owner": { "email": "a@example.com" } }],
                "count": 1
            })
        );
    }
}
