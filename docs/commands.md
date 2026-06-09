# Command Reference

Global flags available on every command:

- `--output table|json`
- `--compact`
- `--fields a,b,c`
- `--quiet`
- `--api-key KEY` for one process only

## Auth

```bash
granola auth login [--validate] [--key KEY | --key-stdin]
granola auth status [--validate]
granola auth logout [--force]
```

## Raw API

```bash
granola api get /v1/notes --query page_size=5
granola api get /v1/folders --output json
```

Raw API requests are guarded: paths must start with `/v1/`, cannot include a URL scheme, and cannot include an inline query string. Pass query parameters with repeated `--query key=value` flags.

## Notes

```bash
granola notes list \
  [--created-before DATE] \
  [--created-after DATE] \
  [--since 7d] \
  [--updated-after DATE] \
  [--updated-since 24h] \
  [--folder-id FOL_ID] \
  [--cursor CURSOR] \
  [--page-size N] \
  [--all] \
  [--limit N] \
  [--sort created-at|updated-at|title] \
  [--order asc|desc] \
  [--no-truncate]

granola notes get NOTE_ID [--include transcript]
granola notes hydrate [NOTE_ID ...] [--ids-file FILE] [--stdin] [--include-transcript] [--jsonl]
granola notes hydrate [list filters] [--include-transcript] [--jsonl]
granola notes open NOTE_ID [--print]
```

## Folders

```bash
granola folders list [--cursor CURSOR] [--page-size N] [--all] [--limit N]
```

## Export

```bash
granola export note NOTE_ID \
  [--format markdown|json|txt|transcript] \
  [--include-transcript] \
  [-o FILE] \
  [--force|--skip-existing]

granola export notes \
  [--format jsonl|markdown|json] \
  [--created-after DATE] \
  [--since 7d] \
  [--updated-after DATE] \
  [--updated-since 24h] \
  [--sort created-at|updated-at|title] \
  [--order asc|desc] \
  [-o FILE | --output-dir DIR] \
  [--include-transcript] \
  [--force|--skip-existing]
```

## Utilities

```bash
granola doctor
granola agent
granola completions zsh|bash|fish|powershell|elvish
```
