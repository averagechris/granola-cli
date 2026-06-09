# Local Cache and Search

The local cache is a non-secret SQLite database in the OS cache directory. It stores fetched note summaries and hydrated note details so humans and agents can search notes without repeatedly calling the API.

```bash
granola sync --since 30d --all --include-transcripts
granola cache status --output json --compact
granola notes search attendees:will "async config" --output json --compact
printf 'transcript:renewal' | granola search
granola cache clear
```

Design constraints:

- The cache never stores API keys or other CLI credentials.
- The primary cache file is `notes.sqlite`; `cache clear` also removes the legacy `notes.json` cache if present.
- `sync` and write-through fetch commands merge fetched notes into the existing cache by note ID.
- Pass `--replace` when you want the cache to contain only the current sync result.
- List-style commands cache searchable note summaries; get/hydrate/sync-style commands cache full note details.
- Pass global `--no-cache` to disable write-through cache updates for an invocation.
- Search uses SQLite FTS5 over note IDs, titles, owners, summaries, attendees, folders, and cached transcript text.
- FTS field filters are supported, including `title:`, `owner:`, `attendees:`, `folders:`, `summary_text:`, `summary_markdown:`, and `transcript:`.
- Phrase queries work with quoted terms, for example `granola search attendees:will "async config"`.
