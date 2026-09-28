#!/usr/bin/env bash
set -euo pipefail

printf '%s\n' 'SourceHut Pages publication is retired; follow docs/release.md for GitHub-only releases' >&2
exit 1

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

domain="averagechris.srht.site"
subdirectory="/granola-cli"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --domain)
      domain="$2"
      shift 2
      ;;
    --subdirectory)
      subdirectory="$2"
      shift 2
      ;;
    -h|--help)
      printf 'usage: publish-pages [--domain DOMAIN] [--subdirectory PATH]\n'
      exit 0
      ;;
    *)
      printf 'unknown argument: %s\n' "$1" >&2
      exit 1
      ;;
  esac
done

pages_tarball="${repo_root}/dist/pages/granola-cli-pages.tar.gz"
if [[ ! -f "${pages_tarball}" ]]; then
  printf 'pages tarball not found: %s\nrun nix run .#build-pages first\n' "${pages_tarball}" >&2
  exit 1
fi

exec hut pages publish "${pages_tarball}" --domain "${domain}" --subdirectory "${subdirectory}"
