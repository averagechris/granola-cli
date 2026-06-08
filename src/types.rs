use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub name: Option<String>,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub object: String,
    pub name: String,
    pub parent_folder_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteSummary {
    pub id: String,
    pub object: String,
    pub title: Option<String>,
    pub owner: User,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Speaker {
    pub source: String,
    pub diarization_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptItem {
    pub speaker: Speaker,
    pub text: String,
    pub start_time: String,
    pub end_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarInvitee {
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub event_title: Option<String>,
    pub invitees: Vec<CalendarInvitee>,
    pub organiser: Option<String>,
    pub calendar_event_id: Option<String>,
    pub scheduled_start_time: Option<String>,
    pub scheduled_end_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub object: String,
    pub title: Option<String>,
    pub owner: User,
    pub created_at: String,
    pub updated_at: String,
    pub web_url: String,
    pub calendar_event: Option<CalendarEvent>,
    pub attendees: Vec<User>,
    pub folder_membership: Vec<Folder>,
    pub summary_text: String,
    pub summary_markdown: Option<String>,
    pub transcript: Option<Vec<TranscriptItem>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListNotesResponse {
    pub notes: Vec<NoteSummary>,
    #[serde(rename = "hasMore")]
    pub has_more: bool,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListFoldersResponse {
    pub folders: Vec<Folder>,
    #[serde(rename = "hasMore")]
    pub has_more: bool,
    pub cursor: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_list_notes_response() {
        let raw = include_str!("../tests/fixtures/list_notes.json");

        let response: ListNotesResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(response.notes[0].id, "not_AAAAAAAAAAAAAA");
        assert!(response.has_more);
        assert_eq!(response.cursor.as_deref(), Some("redacted-cursor"));
    }

    #[test]
    fn deserializes_empty_list_notes_response() {
        let raw = include_str!("../tests/fixtures/list_notes_empty.json");
        let response: ListNotesResponse = serde_json::from_str(raw).unwrap();
        assert!(response.notes.is_empty());
        assert!(!response.has_more);
    }

    #[test]
    fn deserializes_list_folders_response() {
        let raw = include_str!("../tests/fixtures/list_folders.json");
        let response: ListFoldersResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(response.folders.len(), 2);
        assert_eq!(
            response.folders[1].parent_folder_id.as_deref(),
            Some("fol_AAAAAAAAAAAAAA")
        );
    }

    #[test]
    fn deserializes_note_with_null_optional_fields() {
        let raw = include_str!("../tests/fixtures/get_note.json");
        let note: Note = serde_json::from_str(raw).unwrap();
        assert_eq!(note.id, "not_AAAAAAAAAAAAAA");
        assert!(note.calendar_event.is_none());
        assert!(note.summary_markdown.is_none());
        assert!(note.transcript.is_none());
    }

    #[test]
    fn deserializes_note_with_transcript_variants() {
        let raw = include_str!("../tests/fixtures/get_note_with_transcript.json");
        let note: Note = serde_json::from_str(raw).unwrap();
        let transcript = note.transcript.expect("transcript should be present");
        assert_eq!(transcript.len(), 2);
        assert_eq!(
            transcript[0].speaker.diarization_label.as_deref(),
            Some("Speaker A")
        );
        assert!(transcript[1].speaker.diarization_label.is_none());
    }
}
