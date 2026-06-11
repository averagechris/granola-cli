# Requirements

## Product requirements

- Provide a fast Rust CLI for Granola notes, folders, summaries, and transcripts.
- Cover the documented public API first:
  - `GET /v1/notes`
  - `GET /v1/notes/{note_id}`
  - `GET /v1/folders`
- Support human-readable tables and stable machine-readable JSON for every data command.
- Make common workflows terse:
  - authenticate
  - list recent notes
  - fetch a note with optional transcript
  - list folders
  - filter notes by date or folder
  - export notes/summaries/transcripts to files or stdout
- Preserve privacy by default: avoid logging secrets, avoid ambient secret propagation, and store credentials only in the OS keyring.

## Developer-experience requirements

- Use Rust with clap, tokio, reqwest, serde, tabled, and keyring.
- Use a Nix flake dev shell and `.envrc` direnv integration.
- Provide local CI entrypoints through Nix apps:
  - `ci-fmt`
  - `ci-clippy`
  - `ci-test`
- Follow `linear-cli` ergonomics where they fit:
  - explicit subcommands with short aliases once stable
  - keyring-backed auth
  - `--output json|json-compact|json-pretty|table|text|list`
  - `--fields`
  - clear exit codes
  - agent-oriented help/documentation

## Non-requirements for the first implementation

- No write APIs: Granola's documented public API is read-only today.
- No webhook support: Granola documents polling as the current integration model.
- No browser OAuth implementation until Granola exposes and documents a CLI-compatible OAuth flow for the public API.
- No self-update flow.
