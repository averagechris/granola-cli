# Hosted Downloads

Primary install remains Nix:

```bash
nix run sourcehut:averagechris/granola-cli
```

For non-Nix users, this repo can publish static binary downloads to SourceHut Pages.

## Build a macOS binary artifact

On macOS:

```bash
nix run .#package-macos
```

This creates:

```text
dist/downloads/granola-cli-v0.6.0-aarch64-darwin.tar.gz
dist/downloads/granola-cli-v0.6.0-aarch64-darwin.tar.gz.sha256
```

The tarball contains:

- `granola`
- `README.md`
- `LICENSE`

## Build the SourceHut Pages site

```bash
nix run .#build-pages
```

This creates:

```text
dist/pages/granola-cli-pages.tar.gz
```

The pages archive contains an `index.html` plus `downloads/` with tarballs and checksums.

The generated download page highlights the current release features, including URL-as-ID handling, saved views, digests, context bundles, redaction, watch polling, cache maintenance, and agent-friendly JSON output.

## Publish to SourceHut Pages

Configure `hut` once:

```bash
nix run nixpkgs#hut -- init
```

Then publish:

```bash
nix run .#publish-pages
```

Defaults:

- domain: `averagechris.srht.site`
- subdirectory: `/granola-cli`

Override if needed:

```bash
nix run .#publish-pages -- --domain example.com --subdirectory /granola-cli
```

Expected download page:

```text
https://averagechris.srht.site/granola-cli/
```

Expected macOS artifact URL:

```text
https://averagechris.srht.site/granola-cli/downloads/granola-cli-v0.6.0-aarch64-darwin.tar.gz
```

## Manual install from hosted artifact

```bash
curl -LO https://averagechris.srht.site/granola-cli/downloads/granola-cli-v0.6.0-aarch64-darwin.tar.gz
curl -LO https://averagechris.srht.site/granola-cli/downloads/granola-cli-v0.6.0-aarch64-darwin.tar.gz.sha256
sha256sum -c granola-cli-v0.6.0-aarch64-darwin.tar.gz.sha256
tar -xzf granola-cli-v0.6.0-aarch64-darwin.tar.gz
install -m 0755 granola-cli-v0.6.0-aarch64-darwin/granola ~/.local/bin/granola
```
