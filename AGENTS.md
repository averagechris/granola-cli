## Granola CLI Project Guidance

Use `jj` for version-control actions in this repository.

### Development

- Enter the toolchain with `direnv allow` or `nix develop`.
- Prefer local checks: `nix run .#ci-fmt`, `nix run .#ci-clippy`, `nix run .#ci-test`, `nix run .#ci-deny`, `nix run .#ci-audit`, `nix run .#ci-machete`, `nix run .#ci-vet`.
- Keep credentials in the OS keyring only. Do not add plaintext credential storage.
- Keep command output agent-friendly: every data command should support `--output json`, `--output json-compact`, `--fields`, and quiet/no-decorative output where practical.

### Release Workflow

When Chris says "ship a new version", run exactly:

1. `nix run .#release -- --version X.Y.Z --check`
2. `nix run .#release -- --version X.Y.Z`

Run both from an empty `@` whose parent, local `main`, and `main@origin` agree.
The SHA-pinned Fleet GitHub backend prepares and validates the release tree,
then atomically publishes leased `main` and its annotated tag. The read-only
workflow builds macOS arm64 and Linux x86_64 artifacts. Follow
`docs/release.md` to verify its six files, manually publish four GitHub Release
assets, and dispatch Pages. No app or workflow automatically publishes release
assets or Pages.

Notes:

- `nix build .#release-artifact` produces the reproducible tarball
  `granola-cli-vX.Y.Z-<arch>-<os>.tar.gz` + `.sha256` containing
  `granola`, `README.md`, `LICENSE`, and `CHANGELOG.md`.
- `nix run .#package-macos` is a deprecated local packaging alias.
- `builds/release-linux-x86_64.yml`, `build-pages`, and `publish-pages` are
  archival SourceHut or local-preview tools. Never use them to publish a new
  release. Do not dual-publish future tags.
- `ci-deny`, `ci-audit`, and `ci-vet` stay in repository lint because their
  remote advisory/audit inputs are network-volatile; they are not release gates.

### Product Constraints

- Granola's documented public API currently uses API keys, not a public OAuth flow.
- Browser-based OAuth should remain an auth abstraction target, not an implementation promise, until Granola documents a CLI-compatible OAuth flow.
- Respect API rate limits: 25-request burst, 5 requests/second sustained.
- Notes APIs only return notes with generated AI summary and transcript.

<!-- Last audited: 2026-09-28 | moved future releases to manual GitHub publication -->
