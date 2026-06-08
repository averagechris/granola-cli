# granola-cli

A Rust CLI for [Granola](https://granola.ai) meeting notes, transcripts, summaries, and folders.

This repository is initialized for planning-first development. The implementation should follow the developer-experience patterns from `~/projects/linear-cli`: Rust, clap-based commands, Nix dev shell, direnv, keyring-backed credentials, agent-friendly JSON output, local CI scripts, and concise docs.

## Development environment

```bash
direnv allow
nix develop

cargo fmt --all
cargo test
nix run .#ci-fmt
nix run .#ci-clippy
nix run .#ci-test
```

`Cargo.lock` is intentionally not hand-written. Generate it from inside the Nix shell when implementation begins:

```bash
cargo generate-lockfile
```

## Planned quick start

```bash
# Store API key in the OS keyring.
granola auth login

# List recent notes.
granola notes list --created-after 2026-01-01 --output table

# Get a note, including transcript.
granola notes get not_1d3tmYTlCICgjy --include transcript --output json

# List folders for folder-scoped note queries.
granola folders list --output table
```

## Planning docs

- [Requirements](docs/requirements.md)
- [Decisions](docs/decisions.md)
- [Implementation plan](docs/plan.md)
- [Granola API notes](docs/api-notes.md)
- [ADRs](docs/adr/)

## Granola API facts

- Base URL: `https://public-api.granola.ai`
- Auth: `Authorization: Bearer grn_...`
- Public endpoints today: `GET /v1/notes`, `GET /v1/notes/{note_id}`, `GET /v1/folders`
- Pagination: cursor plus `hasMore`; `page_size` max 30
- Rate limit: burst 25 requests, sustained 5 requests/second
- Notes appear only after AI summary and transcript generation
