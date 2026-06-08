# Exporting Notes

Single-note exports:

```bash
granola export note NOTE_ID --format markdown
granola export note NOTE_ID --format markdown --include-transcript -o note.md
granola export note NOTE_ID --format json -o note.json
granola export note NOTE_ID --format transcript -o transcript.txt
```

Multi-note summary exports:

```bash
granola export notes --created-after 2026-06-01 --format jsonl -o notes.jsonl
granola export notes --folder-id FOL_ID --all --format markdown -o notes.md
```

File writes are atomic: the CLI writes a temporary file, flushes it, and renames it into place. Existing files are not overwritten unless `--force` is passed.

Export formats:

- `markdown`: portable human-readable notes
- `json`: pretty JSON
- `jsonl`: one JSON object per line for multi-note exports
- `txt`: title plus plain summary
- `transcript`: transcript speaker labels and text
