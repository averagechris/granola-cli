# Changelog

## v0.6.0 - 2026-06-10

### Added

- Note commands now accept copied Granola note URLs anywhere a raw `not_...` note ID is accepted.
- Saved reusable note selectors with `granola views create|list|show|run|delete` for local search views and API-backed list views.
- `granola digest` for lightweight recent-meeting summaries grouped by day and owner.
- `granola context` for bounded Markdown or JSON note bundles tailored for agent workflows.
- Privacy redaction with `--redact emails,phones,secrets,attendees` on full note output paths.
- `granola watch` for polling newly visible or updated notes.
- Cache maintenance utilities: `granola cache path`, `verify`, `vacuum`, and `export --format jsonl`.

### Changed

- Documentation now covers saved views, context bundles, redaction, watch polling, and cache maintenance workflows.
- Test coverage now exercises the new workflow commands, context rendering, digest grouping, watch deduplication, redaction, and cache export behavior.

## v0.5.0 - 2026-06-10

### Added

- Top-level `granola --version` output for quick local version checks.
- `granola doctor` now reports the installed version, checks hosted release metadata for the latest version, and flags when an update is available.
- Doctor JSON output now includes version-check fields for automation: `latest_version`, `update_available`, `version_check_url`, `version_check_error`, and `version_check_skipped`.

## v0.4.0 - 2026-06-10

### Added

- Adaptive default data output that keeps table rendering when it fits and falls back to list-style rows in narrow terminals.
- Explicit `--output list`, `--output json-compact`, and `--output json-pretty` formats for predictable human and agent-facing output.
- Snapshot coverage for table, list, compact JSON, and pretty JSON output regressions.
- Dependency-injection seams for command data access, making note and folder command behavior easier to unit test with fake API/cache implementations.

### Changed

- Explicit `--output table` now always renders an ASCII table, even in narrow terminals.
- Table width calculations now account for Unicode display width.
- JSON output mode is now format-driven: `--output json` and `--output json-pretty` emit pretty JSON, while `--output json-compact` emits compact JSON.
- Removed the old `--compact`, `--pretty`, and config `compact` output controls in favor of explicit output formats.

## v0.3.0 - 2026-06-09

### Added

- SQLite-backed local cache with FTS5 search, relevance ranking, field-qualified queries, phrase searches, and transcript-aware result labels.
- Default write-through caching for note list/get/hydrate/open and shortcut commands, with global `--no-cache` to opt out.
- Multi-argument and stdin-powered search queries, including `granola search attendees:will "async config"` and `printf 'transcript:renewal' | granola search`.
- Search result ergonomics that distinguish empty caches from no-match searches and report cache/index coverage.
- Integration coverage for search argument parsing, stdin search, cache status, and cache clearing.

### Changed

- `granola cache status` now reports SQLite cache path, summary count, hydrated note count, transcript-indexed note count, and legacy JSON cache presence.
- `granola cache clear` removes both the SQLite cache and legacy JSON cache file.
- Search documentation now highlights FTS field filters, phrase queries, and cache completeness hints.

## v0.2.1 - 2026-06-09

### Fixed

- Hosted `.sha256` files now contain relative artifact filenames instead of build-machine absolute paths, so `sha256sum -c` works after download.

## v0.2.0 - 2026-06-09

This release focuses on making `granola` seamless for humans, scripts, and coding agents.

### Added

- Paginated JSON envelopes for list commands, including `count`, `has_more`, `cursor`, and `page_size`.
- Batch note hydration with `granola notes hydrate` for fetching full note records and transcripts without shell loops.
- A richer machine-readable agent manifest via `granola agent --output json-compact`.
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
