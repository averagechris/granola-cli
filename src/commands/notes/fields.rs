use super::{
    cached_label, display_title, render_note_full, render_note_summary, render_transcript,
    NoteFieldsCommand,
};
use crate::cache::CacheSearchHit;
use crate::error::CliError;
use crate::output::{print_rows, OutputOptions};
use crate::types::{Note, NoteSummary};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NoteOutputCommand {
    List,
    Search,
    Get,
}

impl NoteOutputCommand {
    fn name(self) -> &'static str {
        match self {
            NoteOutputCommand::List => "list",
            NoteOutputCommand::Search => "search",
            NoteOutputCommand::Get => "get",
        }
    }
}

impl From<NoteFieldsCommand> for NoteOutputCommand {
    fn from(command: NoteFieldsCommand) -> Self {
        match command {
            NoteFieldsCommand::List => NoteOutputCommand::List,
            NoteFieldsCommand::Search => NoteOutputCommand::Search,
            NoteFieldsCommand::Get => NoteOutputCommand::Get,
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct NoteFieldSpec {
    pub(crate) name: &'static str,
    pub(crate) commands: &'static [&'static str],
    pub(crate) description: &'static str,
    pub(crate) requires_transcript: bool,
    pub(crate) json_only: bool,
}

pub(crate) fn note_field_specs() -> &'static [NoteFieldSpec] {
    &[
        NoteFieldSpec {
            name: "id",
            commands: &["list", "search", "get"],
            description: "Granola note ID. Useful for pipelines into `notes get` or `notes get-many --stdin`.",
            requires_transcript: false,
            json_only: false,
        },
        NoteFieldSpec {
            name: "title",
            commands: &["list", "search", "get"],
            description: "Note title.",
            requires_transcript: false,
            json_only: false,
        },
        NoteFieldSpec {
            name: "owner",
            commands: &["list", "search", "get"],
            description: "Owner email in human/text output; owner object in JSON output.",
            requires_transcript: false,
            json_only: false,
        },
        NoteFieldSpec {
            name: "owner.email",
            commands: &["list", "search", "get"],
            description: "Owner email address.",
            requires_transcript: false,
            json_only: false,
        },
        NoteFieldSpec {
            name: "created_at",
            commands: &["list", "search", "get"],
            description: "Creation timestamp.",
            requires_transcript: false,
            json_only: false,
        },
        NoteFieldSpec {
            name: "updated_at",
            commands: &["list", "search", "get"],
            description: "Last update timestamp.",
            requires_transcript: false,
            json_only: false,
        },
        NoteFieldSpec {
            name: "cached",
            commands: &["search"],
            description: "Search-cache detail level: summary, full, or transcript.",
            requires_transcript: false,
            json_only: false,
        },
        NoteFieldSpec {
            name: "url",
            commands: &["get"],
            description: "Granola web URL. Alias for web_url in human/text output.",
            requires_transcript: false,
            json_only: false,
        },
        NoteFieldSpec {
            name: "web_url",
            commands: &["get"],
            description: "Granola web URL.",
            requires_transcript: false,
            json_only: false,
        },
        NoteFieldSpec {
            name: "summary",
            commands: &["search", "get"],
            description: "Generated summary, preferring Markdown when available and falling back to plain summary text.",
            requires_transcript: false,
            json_only: false,
        },
        NoteFieldSpec {
            name: "summary_text",
            commands: &["search", "get"],
            description: "Generated plain-text summary.",
            requires_transcript: false,
            json_only: false,
        },
        NoteFieldSpec {
            name: "summary_markdown",
            commands: &["search", "get"],
            description: "Generated Markdown summary when available.",
            requires_transcript: false,
            json_only: false,
        },
        NoteFieldSpec {
            name: "transcript",
            commands: &["search", "get"],
            description: "Transcript text in human/text output; raw transcript array in JSON output.",
            requires_transcript: true,
            json_only: false,
        },
        NoteFieldSpec {
            name: "transcript_text",
            commands: &["search", "get"],
            description: "Rendered transcript text with speaker labels.",
            requires_transcript: true,
            json_only: false,
        },
        NoteFieldSpec {
            name: "full",
            commands: &["get"],
            description: "Human-readable title, metadata, summary, and transcript text.",
            requires_transcript: true,
            json_only: false,
        },
        NoteFieldSpec {
            name: "attendees",
            commands: &["get"],
            description: "Attendee objects. Available in JSON output.",
            requires_transcript: false,
            json_only: true,
        },
        NoteFieldSpec {
            name: "calendar_event",
            commands: &["get"],
            description: "Calendar event object. Available in JSON output.",
            requires_transcript: false,
            json_only: true,
        },
        NoteFieldSpec {
            name: "folder_membership",
            commands: &["get"],
            description: "Folder membership array. Available in JSON output.",
            requires_transcript: false,
            json_only: true,
        },
    ]
}

pub(crate) fn specs_for_command(
    command: NoteOutputCommand,
    include_json_only: bool,
) -> impl Iterator<Item = &'static NoteFieldSpec> {
    note_field_specs().iter().filter(move |spec| {
        spec.commands.contains(&command.name()) && (include_json_only || !spec.json_only)
    })
}

pub(crate) fn validate_fields(
    requested: &[String],
    command: NoteOutputCommand,
    output: &OutputOptions,
) -> Result<Vec<&'static NoteFieldSpec>, CliError> {
    validate_field_names(requested.iter().map(String::as_str), command, output)
}

fn validate_field_names<'a>(
    requested: impl IntoIterator<Item = &'a str>,
    command: NoteOutputCommand,
    output: &OutputOptions,
) -> Result<Vec<&'static NoteFieldSpec>, CliError> {
    let available: Vec<&'static NoteFieldSpec> =
        specs_for_command(command, output.is_json()).collect();
    let mut selected = Vec::new();
    let mut unknown = Vec::new();

    for field in requested {
        let field = field.trim();
        if field.is_empty() {
            continue;
        }
        if let Some(spec) = available.iter().find(|spec| spec.name == field) {
            selected.push(*spec);
        } else {
            unknown.push(field.to_string());
        }
    }

    if !unknown.is_empty() {
        let names: Vec<&str> = available.iter().map(|spec| spec.name).collect();
        return Err(unknown_fields_error(&unknown, &names));
    }

    Ok(selected)
}

pub(crate) fn fields_require_transcript(selected: &[&NoteFieldSpec]) -> bool {
    selected.iter().any(|field| field.requires_transcript)
}

pub(crate) fn requested_fields_require_transcript(
    requested: &[String],
    command: NoteOutputCommand,
    output: &OutputOptions,
) -> Result<bool, CliError> {
    let selected = validate_fields(requested, command, output)?;
    Ok(fields_require_transcript(&selected))
}

pub(crate) fn render_summary_records(
    notes: &[NoteSummary],
    no_truncate: bool,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let selected = selected_or_default(
        output,
        NoteOutputCommand::List,
        &["id", "title", "owner", "created_at", "updated_at"],
    )?;

    if notes.is_empty() {
        if output.fields.is_empty() {
            println!("No notes found");
        }
        return Ok(());
    }

    render_records(
        &selected,
        notes.iter().map(|summary| NoteRecord::Summary {
            summary,
            no_truncate,
        }),
        output,
    )
}

pub(crate) fn render_search_records(
    hits: &[CacheSearchHit],
    output: &OutputOptions,
) -> Result<(), CliError> {
    let selected = selected_or_default(
        output,
        NoteOutputCommand::Search,
        &["id", "title", "owner", "updated_at", "cached"],
    )?;

    if hits.is_empty() {
        return Ok(());
    }

    render_records(&selected, hits.iter().map(NoteRecord::SearchHit), output)
}

pub(crate) fn render_note_records(notes: &[Note], output: &OutputOptions) -> Result<(), CliError> {
    let selected = selected_or_default(
        output,
        NoteOutputCommand::Get,
        &["id", "title", "owner", "created_at", "updated_at"],
    )?;

    if notes.is_empty() {
        if output.fields.is_empty() {
            println!("No notes found");
        }
        return Ok(());
    }

    render_records(&selected, notes.iter().map(NoteRecord::Note), output)
}

pub(crate) fn render_single_note_selected(
    note: &Note,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let selected = validate_fields(&output.fields, NoteOutputCommand::Get, output)?;
    if selected.is_empty() {
        return Ok(());
    }
    render_records(&selected, std::iter::once(NoteRecord::Note(note)), output)
}

fn selected_or_default(
    output: &OutputOptions,
    command: NoteOutputCommand,
    defaults: &[&str],
) -> Result<Vec<&'static NoteFieldSpec>, CliError> {
    if output.fields.is_empty() {
        validate_field_names(defaults.iter().copied(), command, output)
    } else {
        validate_fields(&output.fields, command, output)
    }
}

fn render_records<'a>(
    selected: &[&NoteFieldSpec],
    records: impl IntoIterator<Item = NoteRecord<'a>>,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let headers: Vec<&str> = selected.iter().map(|field| field.name).collect();
    let rows = records
        .into_iter()
        .map(|record| {
            selected
                .iter()
                .map(|field| render_record_field(record, field.name))
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut row_output = output.clone();
    row_output.fields.clear();
    print_rows(&headers, rows, &row_output)
}

#[derive(Clone, Copy)]
enum NoteRecord<'a> {
    Summary {
        summary: &'a NoteSummary,
        no_truncate: bool,
    },
    SearchHit(&'a CacheSearchHit),
    Note(&'a Note),
}

fn render_record_field(record: NoteRecord<'_>, field: &str) -> Result<String, CliError> {
    match record {
        NoteRecord::Summary {
            summary,
            no_truncate,
        } => render_summary_field(summary, no_truncate, field),
        NoteRecord::SearchHit(hit) => Ok(render_search_field(hit, field)),
        NoteRecord::Note(note) => render_note_field(note, field),
    }
}

fn render_summary_field(
    summary: &NoteSummary,
    no_truncate: bool,
    field: &str,
) -> Result<String, CliError> {
    match field {
        "id" => Ok(summary.id.clone()),
        "title" => Ok(display_title(summary, no_truncate)),
        "owner" | "owner.email" => Ok(summary.owner.email.clone()),
        "created_at" => Ok(summary.created_at.clone()),
        "updated_at" => Ok(summary.updated_at.clone()),
        _ => Err(CliError::invalid_input(format!(
            "field '{field}' cannot be rendered for notes list output"
        ))),
    }
}

fn render_search_field(hit: &CacheSearchHit, field: &str) -> String {
    match field {
        "id" => hit.summary.id.clone(),
        "title" => hit.summary.title.as_deref().unwrap_or("").to_string(),
        "owner" | "owner.email" => hit.summary.owner.email.clone(),
        "created_at" => hit.summary.created_at.clone(),
        "updated_at" => hit.summary.updated_at.clone(),
        "cached" => cached_label(hit),
        "summary" => hit
            .note
            .as_ref()
            .map(render_note_summary)
            .unwrap_or_default(),
        "summary_text" => hit
            .note
            .as_ref()
            .map(|note| note.summary_text.clone())
            .unwrap_or_default(),
        "summary_markdown" => hit
            .note
            .as_ref()
            .and_then(|note| note.summary_markdown.clone())
            .unwrap_or_default(),
        "transcript" | "transcript_text" => {
            hit.note.as_ref().map(render_transcript).unwrap_or_default()
        }
        _ => String::new(),
    }
}

pub(crate) fn render_note_field(note: &Note, field: &str) -> Result<String, CliError> {
    match field {
        "id" => Ok(note.id.clone()),
        "title" => Ok(note.title.as_deref().unwrap_or("").to_string()),
        "owner" | "owner.email" => Ok(note.owner.email.clone()),
        "created_at" => Ok(note.created_at.clone()),
        "updated_at" => Ok(note.updated_at.clone()),
        "url" | "web_url" => Ok(note.web_url.clone()),
        "summary" => Ok(render_note_summary(note)),
        "summary_text" => Ok(note.summary_text.clone()),
        "summary_markdown" => Ok(note.summary_markdown.clone().unwrap_or_default()),
        "transcript" | "transcript_text" => Ok(render_transcript(note)),
        "full" => Ok(render_note_full(note)),
        _ => Err(CliError::invalid_input(format!(
            "field '{field}' cannot be rendered for notes get output"
        ))),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OutputFormat;

    fn text_output(fields: &[&str]) -> OutputOptions {
        OutputOptions::new(
            OutputFormat::Text,
            false,
            fields.iter().map(|field| (*field).to_string()).collect(),
            false,
        )
    }

    #[test]
    fn validates_command_scoped_fields() {
        let selected = validate_fields(
            &["id".to_string(), "summary".to_string()],
            NoteOutputCommand::Search,
            &text_output(&[]),
        )
        .unwrap();

        assert_eq!(
            selected.iter().map(|field| field.name).collect::<Vec<_>>(),
            ["id", "summary"]
        );
    }

    #[test]
    fn rejects_unknown_fields_with_available_list() {
        let error = validate_fields(
            &["NODOESNTEXIST".to_string()],
            NoteOutputCommand::Search,
            &text_output(&[]),
        )
        .unwrap_err();

        assert!(error.to_string().contains("unknown field 'NODOESNTEXIST'"));
        assert!(error.to_string().contains("available fields: id, title"));
    }

    #[test]
    fn transcript_fields_require_transcript() {
        let selected = validate_fields(
            &["transcript".to_string()],
            NoteOutputCommand::Get,
            &text_output(&[]),
        )
        .unwrap();

        assert!(fields_require_transcript(&selected));
    }
}
