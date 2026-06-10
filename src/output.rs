use crate::error::CliError;
use crate::OutputFormat;
use serde::Serialize;
use serde_json::{json, Map, Value};
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone)]
pub struct OutputOptions {
    pub format: OutputFormat,
    pub format_explicit: bool,
    pub fields: Vec<String>,
    pub quiet: bool,
}

impl OutputOptions {
    pub fn new(
        format: OutputFormat,
        format_explicit: bool,
        fields: Vec<String>,
        quiet: bool,
    ) -> Self {
        Self {
            format,
            format_explicit,
            fields,
            quiet,
        }
    }

    pub fn is_json(&self) -> bool {
        matches!(
            self.format,
            OutputFormat::Json | OutputFormat::JsonCompact | OutputFormat::JsonPretty
        )
    }

    pub fn json_compact(&self) -> bool {
        match self.format {
            OutputFormat::JsonCompact => true,
            OutputFormat::JsonPretty => false,
            OutputFormat::Json => false,
            OutputFormat::Table | OutputFormat::List => false,
        }
    }
}

pub fn print_rows(headers: &[&str], rows: Vec<Vec<String>>, output: &OutputOptions) {
    match output.format {
        OutputFormat::Json => unreachable!("JSON rows should be emitted with print_json"),
        OutputFormat::JsonCompact => unreachable!("JSON rows should be emitted with print_json"),
        OutputFormat::JsonPretty => unreachable!("JSON rows should be emitted with print_json"),
        OutputFormat::List => print_row_list(headers, &rows),
        OutputFormat::Table if output.format_explicit => print_table(headers, &rows),
        OutputFormat::Table => print_adaptive_table(headers, &rows),
    }
}

pub fn print_json<T: Serialize>(value: &T, output: &OutputOptions) -> Result<(), CliError> {
    let mut value = serde_json::to_value(value)?;

    if !output.fields.is_empty() {
        value = select_fields(&value, &output.fields);
    }

    let text = if output.json_compact() {
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

fn print_adaptive_table(headers: &[&str], rows: &[Vec<String>]) {
    let widths = table_widths(headers, rows);
    if terminal_width().is_some_and(|terminal_width| table_width(&widths) > terminal_width) {
        print_row_list(headers, rows);
        return;
    }

    print_table_with_widths(headers, rows, &widths);
}

fn print_table(headers: &[&str], rows: &[Vec<String>]) {
    let widths = table_widths(headers, rows);
    print_table_with_widths(headers, rows, &widths);
}

fn print_table_with_widths(headers: &[&str], rows: &[Vec<String>], widths: &[usize]) {
    print_table_line(widths);
    print_table_row(headers.iter().copied(), widths);
    print_table_line(widths);
    for row in rows {
        print_table_row(row.iter().map(String::as_str), widths);
    }
    print_table_line(widths);
}

fn table_widths(headers: &[&str], rows: &[Vec<String>]) -> Vec<usize> {
    let mut widths: Vec<usize> = headers
        .iter()
        .map(|header| UnicodeWidthStr::width(*header))
        .collect();
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(UnicodeWidthStr::width(cell.as_str()));
        }
    }
    widths
}

fn table_width(widths: &[usize]) -> usize {
    1 + widths.iter().map(|width| width + 3).sum::<usize>()
}

fn terminal_width() -> Option<usize> {
    if let Some(width) = std::env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|width| *width > 0)
    {
        return Some(width);
    }

    terminal_size::terminal_size().map(|(width, _)| usize::from(width.0))
}

fn print_table_line(widths: &[usize]) {
    print!("+");
    for width in widths {
        print!("-{:-<width$}-+", "", width = width);
    }
    println!();
}

fn print_table_row<'a>(cells: impl IntoIterator<Item = &'a str>, widths: &[usize]) {
    print!("|");
    for (cell, width) in cells.into_iter().zip(widths) {
        let padding = width.saturating_sub(UnicodeWidthStr::width(cell));
        print!(" {cell}{} |", " ".repeat(padding));
    }
    println!();
}

fn print_row_list(headers: &[&str], rows: &[Vec<String>]) {
    for (row_index, row) in rows.iter().enumerate() {
        if row_index > 0 {
            println!();
        }
        for (header, cell) in headers.iter().zip(row) {
            println!("{header}: {cell}");
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

    #[test]
    fn calculates_ascii_table_width() {
        let rows = vec![vec!["not_123".to_string(), "Planning".to_string()]];
        let widths = table_widths(&["id", "title"], &rows);

        assert_eq!(widths, vec![7, 8]);
        assert_eq!(table_width(&widths), 22);
    }

    #[test]
    fn calculates_table_width_with_wide_unicode() {
        let rows = vec![vec!["not_123".to_string(), "📜 EPD × CS".to_string()]];
        let widths = table_widths(&["id", "title"], &rows);

        assert_eq!(widths, vec![7, 11]);
        assert_eq!(table_width(&widths), 25);
    }
}
