# Hosted Downloads

Primary install remains Nix:

```bash
nix run sourcehut:averagechris/granola-cli
```

For non-Nix users, this repo publishes static binary downloads to SourceHut Pages.

## Build a release artifact

On any supported platform:

```bash
nix build .#release-artifact
```

This produces (for the current host platform, e.g. Apple silicon):

```text
result/granola-cli-vX.Y.Z-aarch64-darwin.tar.gz
result/granola-cli-vX.Y.Z-aarch64-darwin.tar.gz.sha256
```

The tarball is reproducible (fixed mtime/owner/ordering) and contains:

- `granola`
- `README.md`
- `LICENSE`
- `CHANGELOG.md`

`nix run .#package-macos` remains as a deprecated alias that delegates to
`nix build .#release-artifact` and copies the outputs into `dist/downloads/`.

Linux x86_64 artifacts are built on SourceHut via the manifest in
`builds/release-linux-x86_64.yml` (submitted explicitly with
`hut builds submit` or `nix run .#release -- --submit-linux-build`; it does not
run automatically on push because it lives in `builds/`, not `.builds/`).

## Build the SourceHut Pages site

```bash
nix run .#build-pages -- --include-existing-downloads
```

This creates:

```text
dist/pages/granola-cli-pages.tar.gz
```

The pages archive contains `index.html`, `manifest.json`, and `downloads/` with
tarballs and checksums. Release copy on the page is generated from the matching
`CHANGELOG.md` entry.

`--include-existing-downloads` fetches previously published artifacts from the
live `manifest.json` at
`https://averagechris.srht.site/granola-cli/manifest.json` so a republish keeps
older platforms/versions available. `manifest.json` has the schema:

```json
{
  "version": "vX.Y.Z",
  "artifacts": [
    {"name": "...", "sha256": "...", "url": "..."}
  ]
}
```

## Publish to SourceHut Pages

Configure `hut` once:

```bash
nix run nixpkgs#hut -- init
```

Then publish:

```bash
nix run .#publish-pages
```

## Full release pipeline

Tiny-safe preflight is read-only and changes no files, jj operations, local
refs, or remote refs:

```bash
nix run .#release -- --version X.Y.Z --check
```

Then use the one normal release command:

```bash
nix run .#release -- --version X.Y.Z
```

Run it from an empty jj working-copy commit whose parent, local `main`, and
`main@origin` agree. It prepares metadata, validates the prepared tree with the
standard gates plus deterministic `ci-machete` and `release-contract`, builds
and verifies the artifact and checksum, and only then atomically publishes
leased `main` plus the annotated tag. If publication succeeded but upload or
refresh failed, rerun the exact command: matching state resumes idempotently;
any mismatch fails closed. `ci-deny`, `ci-audit`, and `ci-vet` remain repository
lints because their advisory/audit inputs require network services and are not
deterministic publication gates. Do not use obsolete skip or pages-publication
flags; `--submit-linux-build` remains available when explicitly wanted.
