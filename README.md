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

# Get a note, including transcript, as compact JSON.
granola notes get not_1d3tmYTlCICgjy --include transcript --output json --compact

# Hydrate many notes into full note records for agents.
granola notes hydrate --since 7d --include-transcript --jsonl

# Open a note in the browser, or print its URL for scripts.
granola notes open not_1d3tmYTlCICgjy
granola notes open not_1d3tmYTlCICgjy --print

# List folders for folder-scoped note queries.
granola folders list

# Agent-friendly JSON with field projection.
granola notes list --output json --compact --fields id,title,owner.email

# Export one note to Markdown.
granola export note not_1d3tmYTlCICgjy --format markdown --include-transcript -o note.md

# Export many notes, one Markdown file per note.
granola export notes --since 30d --format markdown --output-dir ./granola-notes --include-transcript --skip-existing

# Debug a documented Granola endpoint without exposing headers.
granola api get /v1/notes --query page_size=5 --output json --compact
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
https://averagechris.srht.site/granola-cli/downloads/granola-cli-v0.1.0-aarch64-darwin.tar.gz
```

Manual install example:

```bash
curl -LO https://averagechris.srht.site/granola-cli/downloads/granola-cli-v0.1.0-aarch64-darwin.tar.gz
curl -LO https://averagechris.srht.site/granola-cli/downloads/granola-cli-v0.1.0-aarch64-darwin.tar.gz.sha256
sha256sum -c granola-cli-v0.1.0-aarch64-darwin.tar.gz.sha256
tar -xzf granola-cli-v0.1.0-aarch64-darwin.tar.gz
install -m 0755 granola-cli-v0.1.0-aarch64-darwin/granola ~/.local/bin/granola
```

See [Hosted Downloads](docs/downloads.md).

## Command overview

- `granola auth login [--validate] [--key KEY | --key-stdin]`
- `granola auth logout [--force]`
- `granola auth status [--validate]`
- `granola api get /v1/notes --query page_size=5`
- `granola notes list [--created-before DATE] [--created-after DATE|--since 7d] [--updated-after DATE|--updated-since 24h] [--sort created-at|updated-at|title] [--order asc|desc] [--no-truncate]`
- `granola notes get NOTE_ID [--include transcript]`
- `granola notes hydrate [NOTE_ID ... | --ids-file FILE | --stdin | list filters] [--include-transcript] [--jsonl]`
- `granola notes open NOTE_ID [--print]`
- `granola folders list [--cursor CURSOR] [--page-size N] [--all] [--limit N]`
- `granola export note NOTE_ID [--format markdown|json|txt|transcript] [--include-transcript] [-o FILE] [--force|--skip-existing]`
- `granola export notes [--created-before DATE] [--created-after DATE|--since 7d] [--updated-after DATE|--updated-since 24h] [--sort created-at|updated-at|title] [--order asc|desc] [--format jsonl|markdown|json] [-o FILE | --output-dir DIR] [--include-transcript] [--force|--skip-existing]`
- `granola completions zsh|bash|fish|powershell|elvish`
- `granola doctor`
- `granola agent`

Global scriptability flags:

- `--output table|json`
- `--compact`
- `--fields a,b,c`
- `--quiet`
- `--api-key KEY` for process-local auth override only

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

### Hosted download publishing

```bash
nix run .#package-macos
nix run .#build-pages
nix run .#publish-pages
```

`nix run .#publish-pages` requires `hut` configuration. Configure it once with `nix run nixpkgs#hut -- init`.

## Planning docs

- [Requirements](docs/requirements.md)
- [Decisions](docs/decisions.md)
- [Implementation plan](docs/plan.md)
- [Granola API notes](docs/api-notes.md)
- [Command reference](docs/commands.md)
- [Authentication](docs/auth.md)
- [JSON output](docs/json.md)
- [Exporting notes](docs/export.md)
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
