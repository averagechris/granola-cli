# Historical downloads and local previews

Future releases use the manual GitHub process in [release.md](release.md).
This page documents the historical SourceHut site and local preview tools.
Do not use `publish-pages` or the SourceHut build manifest for a new release.

Primary install remains Nix:

```bash
nix run sourcehut:averagechris/granola-cli
```

Historical static binary downloads remain on SourceHut Pages.

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

Historical Linux x86_64 artifacts were built on SourceHut via
`builds/release-linux-x86_64.yml`. The manifest remains as an archive. Do not
submit it for a new tag.

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

## Retired SourceHut publication

`publish-pages` is retained as a compatibility name, but it now exits with an
error. It cannot publish a future release:

```bash
nix run .#publish-pages
```

## Historical release pipeline

The old SourceHut release pipeline is retired. Historical tags, artifact URLs,
and mixed-platform Pages entries remain valid. Use `docs/release.md` for all
future tags.
