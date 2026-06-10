## Granola CLI Project Guidance

Use `jj` for version-control actions in this repository.

### Development

- Enter the toolchain with `direnv allow` or `nix develop`.
- Prefer local checks: `nix run .#ci-fmt`, `nix run .#ci-clippy`, `nix run .#ci-test`, `nix run .#ci-deny`, `nix run .#ci-audit`.
- Keep credentials in the OS keyring only. Do not add plaintext credential storage.
- Keep command output agent-friendly: every data command should support `--output json`, `--output json-compact`, `--fields`, and quiet/no-decorative output where practical.

### Release Workflow

When Chris says "ship a new version":

1. Choose the next semver from the change type, then update `Cargo.toml`, the `granola-cli` entry in `Cargo.lock`, `CHANGELOG.md`, `README.md`, `docs/downloads.md`, and release-page copy in `scripts/build-pages.sh`.
2. Run `cargo fmt --check` and `cargo test`; prefer the Nix CI apps too when time allows.
3. Build artifacts with `nix run .#package-macos`, then `nix run .#build-pages`.
4. Describe/ship with jj: `jj describe -m "feat(release): ship vX.Y.Z"`, then `jj ship --bookmark main --tag vX.Y.Z`.
5. Publish downloads with `nix run .#publish-pages` and verify both the tag and hosted files (`manifest.json`, tarball, `.sha256`) are reachable.

### Product Constraints

- Granola's documented public API currently uses API keys, not a public OAuth flow.
- Browser-based OAuth should remain an auth abstraction target, not an implementation promise, until Granola documents a CLI-compatible OAuth flow.
- Respect API rate limits: 25-request burst, 5 requests/second sustained.
- Notes APIs only return notes with generated AI summary and transcript.

<!-- Last audited: 2026-06-10 | added ship-a-new-version release workflow -->
