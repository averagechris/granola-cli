# granola-cli

A Rust CLI for [Granola](https://granola.ai) meeting notes, transcripts, summaries, and folders.

## Usage examples

```bash
# Store your Granola API key in the OS keyring.
granola auth login --validate

# Or, for automation, avoid putting the key in argv/shell history.
granola auth login --key-stdin --validate < ./token

# Check auth without printing secrets.
granola auth status --validate

# List recent notes.
granola notes list --since 7d --sort updated-at --order desc
granola recent
granola last --include-transcript

# Get a note by ID or copied Granola URL, including transcript, as compact JSON.
granola notes get not_1d3tmYTlCICgjy --include-transcript --output json-compact
granola notes get https://app.granola.ai/notes/not_1d3tmYTlCICgjy

# Get many full note records for agents.
granola notes get-many --since 7d --include-transcript --jsonl

# Open a note in the browser, or print its URL for scripts.
granola notes open not_1d3tmYTlCICgjy
granola notes open not_1d3tmYTlCICgjy --print

# List folders for folder-scoped note queries.
granola folders list
granola folders tree

# Agent-friendly JSON with field projection.
granola notes list --output json-compact --fields notes.id,notes.title,notes.owner.email,count,has_more,cursor

# Save reusable local search or API-backed note views.
granola views create customer-calls --folder-id fol_123 --since 30d --all
granola views create renewal-search --query 'transcript:renewal' --limit 20
granola views run customer-calls

# Generate a lightweight recent-meetings digest.
granola digest --since 7d --all

# Build a bounded Markdown bundle for agents.
granola context --since 7d --include-transcript --max-bytes 200000 -o context.md
granola context --since 7d --include-transcript --redact emails,phones,secrets

# Poll for newly visible or updated notes.
granola watch --since 2h --interval-seconds 60

# Print the machine-readable integration contract for coding agents.
granola agent --output json-compact

# Optional non-secret defaults and profiles.
granola config set profile.agent.output json
granola config set profile.agent.output json-compact
granola --profile agent notes list --since 7d

# Export one note to Markdown.
granola export note not_1d3tmYTlCICgjy --format markdown --include-transcript -o note.md

# Export many notes, one Markdown file per note.
granola export notes --since 30d --format markdown --output-dir ./granola-notes --include-transcript --frontmatter --only-changed

# Debug a documented Granola endpoint without exposing headers.
granola api get /v1/notes --query page_size=5 --output json-compact
```

## Installation

### Nix

```bash
nix run sourcehut:averagechris/granola-cli
nix profile install sourcehut:averagechris/granola-cli
```

### Binary downloads

Hosted downloads for non-Nix users are published on SourceHut Pages:

```text
https://averagechris.srht.site/granola-cli/
```

Current macOS arm64 artifact path after publishing:

```text
https://averagechris.srht.site/granola-cli/downloads/granola-cli-v0.7.0-aarch64-darwin.tar.gz
```

Manual install example:

```bash
curl -LO https://averagechris.srht.site/granola-cli/downloads/granola-cli-v0.7.0-aarch64-darwin.tar.gz
curl -LO https://averagechris.srht.site/granola-cli/downloads/granola-cli-v0.7.0-aarch64-darwin.tar.gz.sha256
sha256sum -c granola-cli-v0.7.0-aarch64-darwin.tar.gz.sha256
tar -xzf granola-cli-v0.7.0-aarch64-darwin.tar.gz
install -m 0755 granola-cli-v0.7.0-aarch64-darwin/granola ~/.local/bin/granola
```

See [Hosted Downloads](docs/downloads.md).

## Command overview

- `granola auth login [--validate] [--key KEY | --key-stdin]`
- `granola auth logout [--force]`
- `granola auth status [--validate]`
- `granola api get /v1/notes --query page_size=5`
- `granola notes list [--created-before DATE] [--created-after DATE|--since 7d] [--updated-after DATE|--updated-since 24h] [--sort created-at|updated-at|title] [--order asc|desc] [--no-truncate]`
- `granola notes get NOTE_ID_OR_URL [--include-transcript]`
- `granola notes get-many [NOTE_ID_OR_URL ... | --notes-file FILE | --stdin | list filters] [--include-transcript] [--jsonl]`
- `granola notes open NOTE_ID_OR_URL [--print]`
- `granola folders list [--cursor CURSOR] [--page-size N] [--all] [--limit N]`
- `granola folders tree`
- `granola recent|today|yesterday|last`
- `granola export note NOTE_ID_OR_URL [--format markdown|json|text|transcript] [--include-transcript] [--frontmatter] [-o FILE] [--force|--skip-existing]`
- `granola export notes [--created-before DATE] [--created-after DATE|--since 7d] [--updated-after DATE|--updated-since 24h] [--sort created-at|updated-at|title] [--order asc|desc] [--format jsonl|markdown|json] [-o FILE | --output-dir DIR] [--include-transcript] [--frontmatter] [--only-changed] [--force|--skip-existing]`
- `granola sync [--since 30d] [--all] [--include-transcript] [--replace]`
- `granola cache status|path|verify|vacuum|export|clear`
- `granola views create|list|show|delete|run`
- `granola digest [--since 7d] [--folder-id FOL_ID] [--all] [--limit N]`
- `granola context [NOTE_ID_OR_URL ... | --notes-file FILE | --stdin | list filters] [--include-transcript] [--format markdown|json] [--max-bytes N] [-o FILE]`
- `granola watch [--since 2h] [--updated-since 30m] [--folder-id FOL_ID] [--interval-seconds 60] [--iterations N] [--include-existing]`

Redaction flags are available on note detail, get-many, export, and context commands with `--redact emails,phones,secrets,attendees`.
Batch full-note commands such as `notes get-many` and `context` require an explicit selector: note IDs/URLs, `--notes-file`, `--stdin`, a list filter, or `--all`.
- `granola config show|path|set KEY VALUE`
- `granola completions zsh|bash|fish|powershell|elvish`
- `granola completions install zsh|bash|fish|powershell|elvish`
- `granola --version`
- `granola doctor [--validate]`
- `granola agent`

Global scriptability flags:

- `--output table|list|json|json-compact|json-pretty` (the default output automatically falls back from table to list-style blocks when it exceeds terminal width; explicit `--output table` is always respected)
- `--fields a,b,c`
- `--quiet`
- `--api-key KEY` for process-local auth override only
- `--profile NAME` to apply non-secret defaults from config
- `--no-cache` to skip read-through cache hits for full-note commands
- `--no-cache-write` to skip write-through cache updates

## Contributing

The implementation follows the developer-experience patterns from `~/projects/linear-cli`: Rust, clap-based commands, Nix dev shell, direnv, keyring-backed credentials, agent-friendly JSON output, local CI scripts, and concise docs.

### Development environment

```bash
direnv allow
nix develop

cargo fmt --all
cargo test
nix run .#ci-fmt
nix run .#ci-clippy
nix run .#ci-test
nix run .#ci-deny
nix run .#ci-audit
```

Output formatting has snapshot coverage so contributors can see and preserve the expected table, list, and JSON styles:

```bash
cargo test --test output_snapshots
INSTA_UPDATE=always cargo test --test output_snapshots
```

### Hosted download publishing

```bash
nix run .#package-macos
nix run .#build-pages
nix run .#publish-pages
```

`nix run .#publish-pages` requires `hut` configuration. Configure it once with `nix run nixpkgs#hut -- init`.

## Planning docs

- [Requirements](docs/requirements.md)
- [Changelog](CHANGELOG.md)
- [Decisions](docs/decisions.md)
- [Implementation plan](docs/plan.md)
- [Granola API notes](docs/api-notes.md)
- [Command reference](docs/commands.md)
- [Authentication](docs/auth.md)
- [JSON output](docs/json.md)
- [Exporting notes](docs/export.md)
- [Local cache and search](docs/cache.md)
- [Non-secret config and profiles](docs/config.md)
- [Hosted Downloads](docs/downloads.md)
- [Shell completions](docs/shell-completions.md)
- [Troubleshooting](docs/troubleshooting.md)
- [ADRs](docs/adr/)

## Granola API facts

- Base URL: `https://public-api.granola.ai`
- Auth: `Authorization: Bearer grn_...`
- Public endpoints implemented today: `GET /v1/notes`, `GET /v1/notes/{note_id}`, `GET /v1/folders`
- Pagination: cursor plus `hasMore`; `page_size` max 30
- Rate limit: burst 25 requests, sustained 5 requests/second
- Notes appear only after AI summary and transcript generation
