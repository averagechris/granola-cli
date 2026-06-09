# Changelog

## v0.2.1 - 2026-06-09

### Fixed

- Hosted `.sha256` files now contain relative artifact filenames instead of build-machine absolute paths, so `sha256sum -c` works after download.

## v0.2.0 - 2026-06-09

This release focuses on making `granola` seamless for humans, scripts, and coding agents.

### Added

- Paginated JSON envelopes for list commands, including `count`, `has_more`, `cursor`, and `page_size`.
- Batch note hydration with `granola notes hydrate` for fetching full note records and transcripts without shell loops.
- A richer machine-readable agent manifest via `granola agent --output json --compact`.
- Simple non-secret local cache and search workflows:
  - `granola sync`
  - `granola cache status`
  - `granola cache clear`
  - `granola notes search`
- Ergonomic shortcuts for common workflows:
  - `granola recent`
  - `granola today`
  - `granola yesterday`
  - `granola last`
  - `granola search`
  - `granola show`
  - `granola open`
- Folder hierarchy rendering with `granola folders tree`.
- Non-secret config defaults and named profiles with `granola config` and `--profile`.
- API reachability/auth validation in `granola doctor --validate`.
- Shell completion install guidance via `granola completions install SHELL`.
- Markdown frontmatter and incremental output-dir exports with `--frontmatter` and `--only-changed`.

### Changed

- `export notes` now supports the same relative date and sorting ergonomics as `notes list`, including `--since`, `--updated-since`, `--sort`, and `--order`.
- JSON field projection now supports nested arrays inside response envelopes, such as `--fields notes.id,notes.title,count`.
- Documentation now emphasizes agent-friendly workflows, cached search, config profiles, and incremental exports.

### Fixed

- `doctor` reports keyring access errors as diagnostics instead of failing before producing output.
- The downloads page now points manual install instructions at the newest packaged artifact when multiple versions are present locally.

## v0.1.0 - 2026-06-09

Initial release with keyring-backed API-key auth, notes/folders listing, note retrieval with optional transcripts, guarded raw API requests, exports, JSON output, shell completions, hosted macOS downloads, and agent-oriented docs.
