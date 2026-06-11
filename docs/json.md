# JSON Output

Use JSON for scripts and AI agents:

```bash
granola notes list --output json-compact
granola folders list --output json --fields id,name
granola notes get not_AAAAAAAAAAAAAA --output json
granola notes get not_AAAAAAAAAAAAAA --fields id,title,summary,transcript --output json
granola api get /v1/notes --query page_size=5 --output json
```

## Field selection

`--fields` accepts comma-separated dot paths:

```bash
granola notes list --output json --fields id,title,owner.email
granola notes get NOTE_ID --fields summary
granola notes get NOTE_ID --fields transcript
```

For row-oriented commands, exactly one requested field defaults to plain `text` output for pipelines; two or more fields default to `table` unless `--output` is set explicitly.
For `notes get`, JSON output without `--fields` returns the full note record and fetches transcript data automatically.
Discover note fields programmatically with:

```bash
granola notes fields --output json-compact
granola notes fields get --output json-compact
```

For arrays, field selection is applied to each item.

## Paginated list shape

Paginated JSON commands return an envelope so scripts and agents can continue from the next cursor without parsing table output:

```json
{
  "notes": [],
  "count": 0,
  "has_more": false,
  "cursor": null,
  "page_size": 10
}
```

`folders list` uses the same shape with a `folders` array.

## Batch note retrieval

Use `notes get-many` when an agent needs full note records for several IDs or a filtered list without shell loops:

```bash
granola notes get-many not_A not_B --include-transcript --output json-compact
granola notes get-many --since 7d --include-transcript --jsonl
```

## Agent manifest

Agents can discover the stable command, output, error, and constraint contract with:

```bash
granola agent --output json-compact
```

## Cached search

Use the simple local cache for repeated search workflows:

```bash
granola sync --since 30d --all --include-transcript
granola notes search "pricing" --output json-compact --fields notes.id,notes.title,count,cache_synced_at
```

`notes search` searches only the local cache; there is no remote/API-backed note search endpoint.

## Error shape

When `--output json` is requested, errors are emitted to stderr as JSON:

```json
{
  "error": true,
  "message": "Granola API authentication failed",
  "code": 3
}
```

Exit codes:

- `0`: success
- `1`: general error
- `2`: not found
- `3`: auth error
- `4`: rate limited
- `5`: invalid input
