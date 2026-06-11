# Exporting Notes

Single-note exports:

```bash
granola export note NOTE_ID --format markdown
granola export note NOTE_ID --format markdown --include-transcript -o note.md
granola export note NOTE_ID --format markdown --frontmatter -o note.md
granola export note NOTE_ID --format json -o note.json
granola export note NOTE_ID --format transcript -o transcript.txt
granola export note NOTE_ID --format markdown -o note.md --skip-existing
```

Multi-note summary exports:

```bash
granola export notes --created-after 2026-06-01 --format jsonl -o notes.jsonl
granola export notes --since 30d --sort updated-at --order desc --format jsonl -o recent.jsonl
granola export notes --folder-id FOL_ID --all --format markdown -o notes.md
granola export notes --created-after 2026-06-01 --format markdown --output-dir ./notes --include-transcript
granola export notes --since 90d --format markdown --output-dir ./notes --frontmatter --only-changed
granola export notes --created-after 2026-06-01 --format json --output-dir ./notes-json --skip-existing
```

File writes are atomic: the CLI writes a temporary file, flushes it, and renames it into place. Existing files are not overwritten unless `--force` is passed. Use `--skip-existing` to leave existing files untouched without failing.

Use `--frontmatter` with markdown exports to include YAML metadata for tools like Obsidian, Cursor, and knowledge-base importers. Use `--only-changed` with `--output-dir` to maintain `.granola-export-manifest.json` and skip notes whose `updated_at` timestamp has not changed.

When `--output-dir` is used, the CLI writes one file per note using a safe filename:

```text
YYYY-MM-DD-slugified-title-not_XXXXXXXXXXXXXX.md
```

`--output-dir` supports `markdown` and `json`; use `-o FILE` for `jsonl`.

Export formats:

- `markdown`: portable human-readable notes
- `json`: pretty JSON
- `jsonl`: one JSON object per line for multi-note exports
- `text`: title plus plain summary
- `transcript`: transcript speaker labels and text
