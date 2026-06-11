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
            OutputFormat::Table | OutputFormat::Text | OutputFormat::List => false,
        }
    }
}

pub fn print_rows(
    headers: &[&str],
    rows: Vec<Vec<String>>,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let (headers, rows) = select_row_fields(headers, rows, &output.fields)?;
    if headers.is_empty() && !output.fields.is_empty() {
        return Ok(());
    }

    match output.format {
        OutputFormat::Json => unreachable!("JSON rows should be emitted with print_json"),
        OutputFormat::JsonCompact => unreachable!("JSON rows should be emitted with print_json"),
        OutputFormat::JsonPretty => unreachable!("JSON rows should be emitted with print_json"),
        OutputFormat::Text => print_text_rows(&rows),
        OutputFormat::List => print_row_list(&headers, &rows),
        OutputFormat::Table if output.format_explicit => print_table(&headers, &rows),
        OutputFormat::Table => print_adaptive_table(&headers, &rows),
    }
    Ok(())
}

fn select_row_fields(
    headers: &[&str],
    rows: Vec<Vec<String>>,
    fields: &[String],
) -> Result<(Vec<String>, Vec<Vec<String>>), CliError> {
    if fields.is_empty() {
        return Ok((
            headers.iter().map(|header| (*header).to_string()).collect(),
            rows,
        ));
    }

    let mut selected_indices = Vec::new();
    let mut unknown_fields = Vec::new();
    for field in fields {
        let original = field.trim();
        if let Some(index) = headers
            .iter()
            .position(|header| *header == original)
            .or_else(|| {
                let top_level = original.split('.').next().unwrap_or(original).trim();
                headers.iter().position(|header| *header == top_level)
            })
        {
            selected_indices.push(index);
        } else {
            unknown_fields.push(original.to_string());
        }
    }

    if !unknown_fields.is_empty() {
        return Err(unknown_fields_error(&unknown_fields, headers));
    }

    if selected_indices.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    let selected_headers = selected_indices
        .iter()
        .map(|index| headers[*index].to_string())
        .collect();
    let selected_rows = rows
        .into_iter()
        .map(|row| {
            selected_indices
                .iter()
                .map(|index| row.get(*index).cloned().unwrap_or_default())
                .collect()
        })
        .collect();

    Ok((selected_headers, selected_rows))
}

fn unknown_fields_error(unknown_fields: &[String], available_fields: &[&str]) -> CliError {
    let field_label = if unknown_fields.len() == 1 {
        format!("unknown field '{}'", unknown_fields[0])
    } else {
        format!("unknown fields: {}", unknown_fields.join(", "))
    };
    CliError::invalid_input(format!(
        "{field_label}; available fields: {}",
        available_fields.join(", ")
    ))
}

pub fn print_json<T: Serialize>(value: &T, output: &OutputOptions) -> Result<(), CliError> {
    let mut value = serde_json::to_value(value)?;

    if !output.fields.is_empty() {
        value = select_fields(&value, &output.fields)?;
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

fn select_fields(value: &Value, fields: &[String]) -> Result<Value, CliError> {
    match value {
        Value::Array(items) => Ok(Value::Array(
            items
                .iter()
                .map(|item| select_object(item, fields))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        Value::Object(_) => select_object(value, fields),
        other => Ok(other.clone()),
    }
}

fn select_object(value: &Value, fields: &[String]) -> Result<Value, CliError> {
    let mut out = Map::new();
    let mut unknown_fields = Vec::new();

    for field in fields {
        let path: Vec<&str> = field.split('.').filter(|part| !part.is_empty()).collect();
        if path.is_empty() {
            continue;
        }
        if !insert_selected(&mut out, value, &path) {
            unknown_fields.push(field.to_string());
        }
    }

    if !unknown_fields.is_empty() {
        let available = available_json_fields(value);
        let available_refs: Vec<&str> = available.iter().map(String::as_str).collect();
        return Err(unknown_fields_error(&unknown_fields, &available_refs));
    }

    Ok(Value::Object(out))
}

fn insert_selected(out: &mut Map<String, Value>, source: &Value, path: &[&str]) -> bool {
    let Some((head, tail)) = path.split_first() else {
        return true;
    };
    let Some(value) = source.get(*head) else {
        return false;
    };

    if tail.is_empty() {
        out.insert((*head).to_string(), value.clone());
        return true;
    }

    match value {
        Value::Object(_) => {
            let entry = out
                .entry((*head).to_string())
                .or_insert_with(|| Value::Object(Map::new()));
            if let Value::Object(map) = entry {
                insert_selected(map, value, tail)
            } else {
                false
            }
        }
        Value::Array(items) => {
            if items.is_empty() {
                out.insert((*head).to_string(), Value::Array(Vec::new()));
                return true;
            }
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
            true
        }
        _ => false,
    }
}

fn available_json_fields(value: &Value) -> Vec<String> {
    match value {
        Value::Object(map) => map.keys().cloned().collect(),
        Value::Array(items) => items
            .iter()
            .find_map(|item| match item {
                Value::Object(map) => Some(map.keys().cloned().collect()),
                _ => None,
            })
            .unwrap_or_default(),
        _ => Vec::new(),
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

fn print_adaptive_table(headers: &[String], rows: &[Vec<String>]) {
    let widths = table_widths(headers, rows);
    if terminal_width().is_some_and(|terminal_width| table_width(&widths) > terminal_width) {
        print_row_list(headers, rows);
        return;
    }

    print_table_with_widths(headers, rows, &widths);
}

fn print_table(headers: &[String], rows: &[Vec<String>]) {
    let widths = table_widths(headers, rows);
    print_table_with_widths(headers, rows, &widths);
}

fn print_table_with_widths(headers: &[String], rows: &[Vec<String>], widths: &[usize]) {
    print_table_line(widths);
    print_table_row(headers.iter().map(String::as_str), widths);
    print_table_line(widths);
    for row in rows {
        print_table_row(row.iter().map(String::as_str), widths);
    }
    print_table_line(widths);
}

fn table_widths<T: AsRef<str>>(headers: &[T], rows: &[Vec<String>]) -> Vec<usize> {
    let mut widths: Vec<usize> = headers
        .iter()
        .map(|header| display_width(header.as_ref()))
        .collect();
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(display_width(cell.as_str()));
        }
    }
    widths
}

fn display_width(text: &str) -> usize {
    let width = UnicodeWidthStr::width(text);
    width + emoji_variation_width_adjustment(text)
}

fn emoji_variation_width_adjustment(text: &str) -> usize {
    let mut extra_width = 0;
    let mut previous = None;

    for character in text.chars() {
        if character == '\u{fe0f}' {
            if let Some(previous) = previous {
                extra_width += emoji_variation_extra_width(previous);
            }
        } else {
            previous = Some(character);
        }
    }

    extra_width
}

fn emoji_variation_extra_width(base: char) -> usize {
    let base = base.to_string();
    let emoji = format!("{base}\u{fe0f}");
    let base_width = UnicodeWidthStr::width(base.as_str());
    let emoji_width = UnicodeWidthStr::width(emoji.as_str());

    usize::from(base_width == 1 && emoji_width == base_width)
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
        let padding = width.saturating_sub(display_width(cell));
        print!(" {cell}{} |", " ".repeat(padding));
    }
    println!();
}

fn print_row_list(headers: &[String], rows: &[Vec<String>]) {
    for (row_index, row) in rows.iter().enumerate() {
        if row_index > 0 {
            println!();
        }
        for (header, cell) in headers.iter().zip(row) {
            println!("{header}: {cell}");
        }
    }
}

fn print_text_rows(rows: &[Vec<String>]) {
    for row in rows {
        println!("{}", row.join("\t"));
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
        let selected =
            select_fields(&value, &["id".to_string(), "owner.email".to_string()]).unwrap();
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
        )
        .unwrap();

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

    #[test]
    fn calculates_table_width_with_emoji_variation_selectors() {
        let rows = vec![vec![
            "not_123".to_string(),
            "Engineering Guild ⚔️".to_string(),
        ]];
        let widths = table_widths(&["id", "title"], &rows);

        assert_eq!(display_width("⚔️"), 2);
        assert_eq!(widths, vec![7, 20]);
        assert_eq!(table_width(&widths), 34);
    }

    #[test]
    fn selects_table_rows_by_field_name() {
        let (headers, rows) = select_row_fields(
            &["id", "title", "owner"],
            vec![vec![
                "not_123".to_string(),
                "Planning".to_string(),
                "a@example.com".to_string(),
            ]],
            &["id".to_string()],
        )
        .unwrap();

        assert_eq!(headers, vec!["id"]);
        assert_eq!(rows, vec![vec!["not_123"]]);
    }

    #[test]
    fn unknown_table_fields_error() {
        let error = select_row_fields(
            &["id", "title"],
            vec![vec!["not_123".to_string(), "Planning".to_string()]],
            &["id".to_string(), "NOPE".to_string()],
        )
        .unwrap_err();

        assert!(error.to_string().contains("unknown field 'NOPE'"));
        assert!(error.to_string().contains("available fields: id, title"));
    }

    #[test]
    fn unknown_json_fields_error() {
        let value = json!({ "id": "not_123", "title": "Planning" });

        let error = select_fields(&value, &["NODOESNTEXIST".to_string()]).unwrap_err();

        assert!(error.to_string().contains("unknown field 'NODOESNTEXIST'"));
    }
}
