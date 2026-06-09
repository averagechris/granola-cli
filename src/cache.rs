use crate::error::CliError;
use crate::types::Note;
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

const CACHE_VERSION: u8 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotesCache {
    pub version: u8,
    pub synced_at: String,
    pub notes: Vec<Note>,
}

impl Default for NotesCache {
    fn default() -> Self {
        Self {
            version: CACHE_VERSION,
            synced_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            notes: Vec::new(),
        }
    }
}

pub fn cache_path() -> Result<PathBuf, CliError> {
    let base = dirs::cache_dir()
        .ok_or_else(|| CliError::general("could not determine the OS cache directory"))?;
    Ok(base.join("granola-cli").join("notes.json"))
}

pub fn load() -> Result<Option<NotesCache>, CliError> {
    let path = cache_path()?;
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&path).map_err(|error| {
        CliError::general(format!("failed to read cache {}: {error}", path.display()))
    })?;
    let cache: NotesCache = serde_json::from_str(&content).map_err(|error| {
        CliError::general(format!("failed to parse cache {}: {error}", path.display()))
    })?;

    if cache.version != CACHE_VERSION {
        return Err(CliError::general(format!(
            "unsupported cache version {}; run `granola cache clear` and sync again",
            cache.version
        )));
    }

    Ok(Some(cache))
}

pub fn save(cache: &NotesCache) -> Result<(), CliError> {
    let path = cache_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            CliError::general(format!(
                "failed to create cache directory {}: {error}",
                parent.display()
            ))
        })?;
    }

    let temp_path = path.with_extension("json.tmp");
    let content = serde_json::to_string_pretty(cache).map_err(CliError::from)?;
    fs::write(&temp_path, content).map_err(|error| {
        CliError::general(format!(
            "failed to write temporary cache {}: {error}",
            temp_path.display()
        ))
    })?;
    fs::rename(&temp_path, &path).map_err(|error| {
        let _ = fs::remove_file(&temp_path);
        CliError::general(format!(
            "failed to replace cache {}: {error}",
            path.display()
        ))
    })?;
    Ok(())
}

pub fn clear() -> Result<bool, CliError> {
    let path = cache_path()?;
    if !path.exists() {
        return Ok(false);
    }
    fs::remove_file(&path).map_err(|error| {
        CliError::general(format!(
            "failed to remove cache {}: {error}",
            path.display()
        ))
    })?;
    Ok(true)
}

pub fn merge_notes(existing: Option<NotesCache>, fetched: Vec<Note>) -> NotesCache {
    let mut by_id: BTreeMap<String, Note> = existing
        .unwrap_or_default()
        .notes
        .into_iter()
        .map(|note| (note.id.clone(), note))
        .collect();

    for note in fetched {
        by_id.insert(note.id.clone(), note);
    }

    NotesCache {
        version: CACHE_VERSION,
        synced_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        notes: by_id.into_values().collect(),
    }
}

pub fn search_notes<'a>(cache: &'a NotesCache, query: &str) -> Vec<&'a Note> {
    let query = query.to_lowercase();
    cache
        .notes
        .iter()
        .filter(|note| note_matches(note, &query))
        .collect()
}

fn note_matches(note: &Note, query: &str) -> bool {
    contains(note.id.as_str(), query)
        || note
            .title
            .as_deref()
            .is_some_and(|title| contains(title, query))
        || contains(&note.summary_text, query)
        || note
            .summary_markdown
            .as_deref()
            .is_some_and(|summary| contains(summary, query))
        || note
            .owner
            .name
            .as_deref()
            .is_some_and(|name| contains(name, query))
        || contains(&note.owner.email, query)
        || note.attendees.iter().any(|user| {
            user.name
                .as_deref()
                .is_some_and(|name| contains(name, query))
                || contains(&user.email, query)
        })
        || note
            .folder_membership
            .iter()
            .any(|folder| contains(&folder.id, query) || contains(&folder.name, query))
        || note
            .transcript
            .as_ref()
            .is_some_and(|transcript| transcript.iter().any(|item| contains(&item.text, query)))
}

fn contains(value: &str, query: &str) -> bool {
    value.to_lowercase().contains(query)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Note;

    #[test]
    fn search_matches_summary_text() {
        let note: Note =
            serde_json::from_str(include_str!("../tests/fixtures/get_note.json")).unwrap();
        let cache = NotesCache {
            notes: vec![note],
            ..Default::default()
        };

        assert_eq!(search_notes(&cache, "summary").len(), 1);
        assert!(search_notes(&cache, "definitely absent").is_empty());
    }
}
