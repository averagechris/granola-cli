## Granola CLI Project Guidance

Use `jj` for version-control actions in this repository.

### Development

- Enter the toolchain with `direnv allow` or `nix develop`.
- Prefer local checks: `nix run .#ci-fmt`, `nix run .#ci-clippy`, `nix run .#ci-test`, `nix run .#ci-deny`, `nix run .#ci-audit`, `nix run .#ci-machete`, `nix run .#ci-vet`.
- Keep credentials in the OS keyring only. Do not add plaintext credential storage.
- Keep command output agent-friendly: every data command should support `--output json`, `--output json-compact`, `--fields`, and quiet/no-decorative output where practical.

### Release Workflow

When Chris says "ship a new version", use the standard Nix release pipeline:

1. Choose the next semver, run the read-only Tiny-safe preflight
   `nix run .#release -- --version X.Y.Z --check`, then run exactly
   `nix run .#release -- --version X.Y.Z` from an empty `@` whose parent, local
   `main`, and `main@origin` agree. This runs `prepare-release`
   (bumps `Cargo.toml`/`Cargo.lock`, converts CHANGELOG `## Unreleased` to a
   dated `## vX.Y.Z - YYYY-MM-DD` entry — generating bullets from conventional
   commits if empty — and updates artifact names in
   `builds/release-linux-x86_64.yml`), validates the prepared tree (standard
   Rust gates, deterministic `ci-machete`, and `release-contract`), builds and
   verifies the artifact before atomically publishing leased `main` plus the
   annotated tag, uploads it, and requests the central Pages refresh.
2. Verify the tag plus hosted files (`manifest.json`, tarball,
   `.sha256`) are reachable at
   https://averagechris.srht.site/granola-cli/.
3. Submit the Linux x86_64 build with `--submit-linux-build` (or
   `hut builds submit builds/release-linux-x86_64.yml`); it builds the Linux
   release artifact and republishes pages including existing downloads. The
   manifest lives in `builds/` (not `.builds/`) so it never runs on push.
4. If publication succeeded but upload/refresh failed, rerun the exact command;
   matching state resumes idempotently and mismatches fail closed. Obsolete
   skip/pages flags must not be used.

Notes:

- `nix build .#release-artifact` produces the reproducible tarball
  `granola-cli-vX.Y.Z-<arch>-<os>.tar.gz` + `.sha256` containing
  `granola`, `README.md`, `LICENSE`, and `CHANGELOG.md`.
- `nix run .#package-macos` is a deprecated alias that delegates to
  `release-artifact` and copies outputs to `dist/downloads/`.
- Release-page copy is generated from `CHANGELOG.md`; no manual edits to
  `scripts/build-pages.sh` are needed per release.
- `ci-deny`, `ci-audit`, and `ci-vet` stay in repository lint because their
  remote advisory/audit inputs are network-volatile; they are not release gates.

### Product Constraints

- Granola's documented public API currently uses API keys, not a public OAuth flow.
- Browser-based OAuth should remain an auth abstraction target, not an implementation promise, until Granola documents a CLI-compatible OAuth flow.
- Respect API rate limits: 25-request burst, 5 requests/second sustained.
- Notes APIs only return notes with generated AI summary and transcript.

<!-- Last audited: 2026-06-10 | added ship-a-new-version release workflow -->
