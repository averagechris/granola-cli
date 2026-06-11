use crate::types::{CalendarEvent, Note, NoteSummary, User};
use clap::ValueEnum;
use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RedactionKind {
    Emails,
    Phones,
    Secrets,
    Attendees,
}

pub fn redact_note(note: &mut Note, kinds: &[RedactionKind]) {
    if kinds.is_empty() {
        return;
    }
    redact_user(&mut note.owner, kinds);
    for attendee in &mut note.attendees {
        redact_user(attendee, kinds);
    }
    if kinds.contains(&RedactionKind::Attendees) {
        note.attendees.clear();
    }
    if let Some(event) = &mut note.calendar_event {
        redact_calendar_event(event, kinds);
    }
    redact_string(&mut note.summary_text, kinds);
    if let Some(markdown) = &mut note.summary_markdown {
        redact_string(markdown, kinds);
    }
    if let Some(transcript) = &mut note.transcript {
        for item in transcript {
            redact_string(&mut item.text, kinds);
        }
    }
}

pub fn redact_notes(notes: &mut [Note], kinds: &[RedactionKind]) {
    for note in notes {
        redact_note(note, kinds);
    }
}

pub fn redact_note_summary(summary: &mut NoteSummary, kinds: &[RedactionKind]) {
    if kinds.is_empty() {
        return;
    }
    redact_user(&mut summary.owner, kinds);
    if let Some(title) = &mut summary.title {
        redact_string(title, kinds);
    }
}

fn redact_user(user: &mut User, kinds: &[RedactionKind]) {
    if kinds.contains(&RedactionKind::Attendees) {
        user.name = user.name.as_ref().map(|_| "[redacted name]".to_string());
        user.email = "[redacted email]".to_string();
        return;
    }
    if kinds.contains(&RedactionKind::Emails) {
        user.email = "[redacted email]".to_string();
    }
    if let Some(name) = &mut user.name {
        redact_string(name, kinds);
    }
}

fn redact_calendar_event(event: &mut CalendarEvent, kinds: &[RedactionKind]) {
    if let Some(title) = &mut event.event_title {
        redact_string(title, kinds);
    }
    if let Some(organiser) = &mut event.organiser {
        redact_string(organiser, kinds);
    }
    for invitee in &mut event.invitees {
        if kinds.contains(&RedactionKind::Emails) || kinds.contains(&RedactionKind::Attendees) {
            invitee.email = "[redacted email]".to_string();
        }
    }
}

fn redact_string(value: &mut String, kinds: &[RedactionKind]) {
    if kinds.contains(&RedactionKind::Emails) {
        *value = email_regex()
            .replace_all(value, "[redacted email]")
            .into_owned();
    }
    if kinds.contains(&RedactionKind::Phones) {
        *value = phone_regex()
            .replace_all(value, "[redacted phone]")
            .into_owned();
    }
    if kinds.contains(&RedactionKind::Secrets) {
        *value = secret_regex()
            .replace_all(value, "[redacted secret]")
            .into_owned();
    }
}

fn email_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").unwrap())
}

fn phone_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?x)\b(?:\+?1[-.\s]?)?(?:\(?\d{3}\)?[-.\s]?)\d{3}[-.\s]?\d{4}\b").unwrap()
    })
}

fn secret_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(?:grn|api|sk|pk|key|token|secret)[A-Za-z0-9_\-]{12,}\b").unwrap()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_basic_patterns() {
        let mut value =
            "email me at test@example.com or +1 555-123-4567 with grn_testsecret12345".to_string();
        redact_string(
            &mut value,
            &[
                RedactionKind::Emails,
                RedactionKind::Phones,
                RedactionKind::Secrets,
            ],
        );
        assert!(!value.contains("test@example.com"));
        assert!(!value.contains("555-123-4567"));
        assert!(!value.contains("grn_testsecret12345"));
    }

    #[test]
    fn redacts_note_metadata_summary_and_transcript() {
        let mut note: Note = serde_json::from_str(include_str!(
            "../tests/fixtures/get_note_with_transcript.json"
        ))
        .unwrap();
        note.owner.email = "owner@example.com".to_string();
        note.summary_text = "Call test@example.com at 555-123-4567".to_string();
        note.transcript.as_mut().unwrap()[0].text = "secret grn_testsecret12345".to_string();

        redact_note(
            &mut note,
            &[
                RedactionKind::Emails,
                RedactionKind::Phones,
                RedactionKind::Secrets,
                RedactionKind::Attendees,
            ],
        );

        assert_eq!(note.owner.email, "[redacted email]");
        assert!(note.attendees.is_empty());
        assert!(note.summary_text.contains("[redacted email]"));
        assert!(note.summary_text.contains("[redacted phone]"));
        assert!(note.transcript.unwrap()[0]
            .text
            .contains("[redacted secret]"));
    }
}
