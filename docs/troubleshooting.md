# Troubleshooting

## `auth status --validate` returns false

- Confirm the API key is active in Granola.
- Confirm your workspace plan supports API keys.
- Re-run `granola auth login --validate`.

## Notes are missing

Granola only returns notes that have generated AI summaries and transcripts. Notes may be absent if they are still processing, never summarized, or outside the API key's scopes.

Check:

- Personal vs public note scopes on the API key.
- Whether the note is in a folder accessible to the key.
- Whether the note has completed AI summary/transcript generation.

## Rate limits

Granola documents a 25-request burst and 5 requests/second sustained limit. The CLI paces requests and retries safe GETs on transient failures, but large `--all` exports may still take time.

## Keyring issues

Run:

```bash
granola doctor --output json
```

On Linux, Secret Service requires a working D-Bus/keyring daemon.
