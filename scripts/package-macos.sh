#!/usr/bin/env bash
set -euo pipefail

# DEPRECATED: package-macos is a compatibility alias for the standard release
# artifact flow. It delegates to `nix build .#release-artifact` and copies the
# tarball + checksum into dist/downloads (where build-pages expects them).
# Prefer:
#   nix build .#release-artifact
# or the full pipeline:
#   nix run .#release -- --version X.Y.Z

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

if [[ $# -gt 0 ]]; then
  printf 'package-macos no longer accepts arguments (got: %s)\n' "$*" >&2
  exit 1
fi

if [[ "$(uname -s)" != "Darwin" ]]; then
  printf 'package-macos must run on macOS; got %s\n' "$(uname -s)" >&2
  printf 'on other platforms, use: nix build .#release-artifact\n' >&2
  exit 1
fi

printf 'package-macos is deprecated; delegating to nix build .#release-artifact\n' >&2

nix build .#release-artifact --out-link result-release-artifact
mkdir -p dist/downloads
cp -p result-release-artifact/* dist/downloads/
chmod u+w dist/downloads/*

for f in result-release-artifact/*; do
  printf 'created %s\n' "dist/downloads/$(basename "$f")"
done
