# JSON Output

Use JSON for scripts and AI agents:

```bash
granola notes list --output json --compact
granola folders list --output json --fields id,name
granola notes get not_AAAAAAAAAAAAAA --include transcript --output json
granola api get /v1/notes --query page_size=5 --output json
```

## Field selection

`--fields` accepts comma-separated dot paths:

```bash
granola notes list --output json --fields id,title,owner.email
```

For arrays, field selection is applied to each item.

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
