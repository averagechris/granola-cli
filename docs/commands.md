# Command Reference

Global flags available on every command:

- `--output table|list|json|json-compact|json-pretty`
- `--fields a,b,c`
- `--quiet`
- `--api-key KEY` for one process only
- `--profile NAME`
- `--no-cache` to disable automatic write-through updates to the local note cache

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

granola notes get NOTE_ID_OR_URL [--include transcript]
granola notes hydrate [NOTE_ID_OR_URL ...] [--ids-file FILE] [--stdin] [--include-transcript] [--jsonl]
granola notes hydrate [list filters] [--include-transcript] [--jsonl]
granola notes search [--stdin] [QUERY ...] [--limit N]
granola notes open NOTE_ID_OR_URL [--print]
```

Any argument named `NOTE_ID_OR_URL` accepts either a raw Granola note ID like `not_...` or a copied Granola note URL containing that ID.

Search uses the local SQLite FTS index. Query examples:

```bash
granola notes search apple
granola notes search attendees:will "async config"
granola notes search transcript:renewal
printf 'attendees:will "async config"' | granola notes search
```

## Folders

```bash
granola folders list [--cursor CURSOR] [--page-size N] [--all] [--limit N]
granola folders tree [--page-size N]
```

## Shortcuts

```bash
granola recent
granola today
granola yesterday
granola last [--include-transcript]
granola search [--stdin] [QUERY ...]
granola show NOTE_ID_OR_URL
granola open NOTE_ID_OR_URL
```

## Export

```bash
granola export note NOTE_ID_OR_URL \
  [--format markdown|json|txt|transcript] \
  [--include-transcript] \
  [--frontmatter] \
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
  [--frontmatter] \
  [--only-changed] \
  [--force|--skip-existing]
```

## Utilities

```bash
granola sync [--since 30d] [--all] [--include-transcripts] [--replace]
granola cache status
granola cache clear
granola views create NAME [--query QUERY | list filters] [--limit N] [--all]
granola views list
granola views show NAME
granola views run NAME
granola views delete NAME
granola digest [--since 7d] [--folder-id FOL_ID] [--all] [--limit N]
granola context [NOTE_ID_OR_URL ... | --ids-file FILE | --stdin | list filters] [--include-transcript] [--format markdown|json] [--max-bytes N] [-o FILE]
granola watch [--since 2h] [--updated-since 30m] [--folder-id FOL_ID] [--interval-seconds 60] [--iterations N] [--include-existing]
granola config show
granola config path
granola config set output json
granola config set profile.agent.output json-compact
granola doctor [--validate]
granola agent
granola completions zsh|bash|fish|powershell|elvish
granola completions install zsh|bash|fish|powershell|elvish
```

Commands that print or write full note content support privacy redaction with repeated or comma-separated `--redact emails,phones,secrets,attendees` values.
