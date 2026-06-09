# Command Reference

Global flags available on every command:

- `--output table|json`
- `--compact`
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

granola notes get NOTE_ID [--include transcript]
granola notes hydrate [NOTE_ID ...] [--ids-file FILE] [--stdin] [--include-transcript] [--jsonl]
granola notes hydrate [list filters] [--include-transcript] [--jsonl]
granola notes search [--stdin] [QUERY ...] [--limit N]
granola notes open NOTE_ID [--print]
```

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
granola show NOTE_ID
granola open NOTE_ID
```

## Export

```bash
granola export note NOTE_ID \
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
granola config show
granola config path
granola config set output json
granola config set profile.agent.compact true
granola doctor [--validate]
granola agent
granola completions zsh|bash|fish|powershell|elvish
granola completions install zsh|bash|fish|powershell|elvish
```
