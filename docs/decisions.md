# Decisions

- **Repository shape:** start as a fresh Rust binary crate, not a fork.
- **Executable name:** publish `granola`; keep Cargo package `granola-cli`.
- **Development shell:** use Nix flakes plus direnv. All contributors should be able to run checks from `nix develop`.
- **HTTP client:** use `reqwest` with rustls native roots and system proxy support.
- **Credential storage:** keyring-only persistence. Support a process-local `--api-key` override for one-off use; do not persist secrets in config files.
- **OAuth:** do not promise browser OAuth initially. Granola's public API docs only document API keys; the Granola MCP uses browser OAuth but that is not the same as a documented public CLI OAuth flow.
- **Output contract:** table for humans, JSON for agents/scripts. JSON output should be stable and compactable.
- **Pagination:** default to one page unless `--all` is passed; expose `--page-size` and `--cursor` for precise control.
- **Rate limiting:** add client-side retry/backoff for 429 responses and keep default concurrency conservative.
- **Generated API types:** start with hand-written serde models from the small OpenAPI surface; revisit generated types only if the API grows materially.
- **Config:** keep non-secret metadata in a config file; keep secrets in the OS keyring.
