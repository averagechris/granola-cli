# Local Cache and Search

The local cache is a simple non-secret JSON file in the OS cache directory. It stores fetched note data so humans and agents can search recent notes without repeatedly calling the API.

```bash
granola sync --since 30d --all --include-transcripts
granola cache status --output json --compact
granola notes search "renewal risk" --output json --compact
granola cache clear
```

Design constraints:

- The cache never stores API keys or other CLI credentials.
- `sync` merges fetched notes into the existing cache by note ID.
- Pass `--replace` when you want the cache to contain only the current sync result.
- Search is intentionally simple and robust: case-insensitive substring matching over note IDs, titles, summaries, owner/attendee emails, folder names, and cached transcript text.
