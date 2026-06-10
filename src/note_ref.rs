use crate::error::CliError;

const NOTE_ID_PREFIX: &str = "not_";

pub fn normalize_note_id(input: &str) -> Result<String, CliError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(CliError::invalid_input("note ID cannot be empty"));
    }

    if input.starts_with(NOTE_ID_PREFIX) && !input.contains('/') && !input.contains('?') {
        return Ok(input.to_string());
    }

    if let Ok(url) = url::Url::parse(input) {
        if let Some(id) = note_id_from_url(&url) {
            return Ok(id);
        }
        return Err(CliError::invalid_input(format!(
            "could not find a Granola note ID in URL '{input}'"
        )));
    }

    Err(CliError::invalid_input(format!(
        "invalid note ID or URL '{input}'; expected an ID like not_... or a Granola note URL"
    )))
}

fn note_id_from_url(url: &url::Url) -> Option<String> {
    if let Some(id) = url
        .path_segments()
        .into_iter()
        .flatten()
        .find(|segment| segment.starts_with(NOTE_ID_PREFIX))
    {
        return Some(clean_note_id(id).to_string());
    }

    url.query_pairs().find_map(|(_, value)| {
        value
            .starts_with(NOTE_ID_PREFIX)
            .then(|| clean_note_id(&value).to_string())
    })
}

fn clean_note_id(value: &str) -> &str {
    value.split(['?', '#', '&']).next().unwrap_or(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_raw_note_ids() {
        assert_eq!(normalize_note_id("not_abc123").unwrap(), "not_abc123");
    }

    #[test]
    fn extracts_note_id_from_path() {
        assert_eq!(
            normalize_note_id("https://app.granola.ai/notes/not_abc123").unwrap(),
            "not_abc123"
        );
    }

    #[test]
    fn extracts_note_id_from_query_value() {
        assert_eq!(
            normalize_note_id("https://app.granola.ai/share?note=not_abc123").unwrap(),
            "not_abc123"
        );
    }
}
