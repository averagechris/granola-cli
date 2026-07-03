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

The orchestrator runs prepare → validate → tag → artifact → pages:

```bash
nix run .#release -- --version X.Y.Z [--publish-pages] [--submit-linux-build]
```

See `nix run .#release -- --help` for skip flags.
