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
        let raw = r#"{
          "notes": [{
            "id": "not_1d3tmYTlCICgjy",
            "object": "note",
            "title": "Quarterly yoghurt budget review",
            "owner": { "name": "Oat Benson", "email": "oat@granola.ai" },
            "created_at": "2026-01-27T15:30:00Z",
            "updated_at": "2026-01-27T16:45:00Z"
          }],
          "hasMore": false,
          "cursor": null
        }"#;

        let response: ListNotesResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(response.notes[0].id, "not_1d3tmYTlCICgjy");
        assert!(!response.has_more);
    }
}
