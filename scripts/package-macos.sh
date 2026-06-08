#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

version="$(python3 - <<'PY'
import pathlib, tomllib
with pathlib.Path('Cargo.toml').open('rb') as f:
    print(tomllib.load(f)['package']['version'])
PY
)"

machine="$(uname -m)"
case "${machine}" in
  arm64|aarch64) arch="aarch64" ;;
  x86_64|amd64) arch="x86_64" ;;
  *)
    printf 'unsupported macOS architecture: %s\n' "${machine}" >&2
    exit 1
    ;;
esac

if [[ "$(uname -s)" != "Darwin" ]]; then
  printf 'package-macos must run on macOS; got %s\n' "$(uname -s)" >&2
  exit 1
fi

platform="${arch}-darwin"
artifact="granola-cli-v${version}-${platform}.tar.gz"
download_dir="${repo_root}/dist/downloads"
stage_dir="${repo_root}/dist/stage/${artifact%.tar.gz}"
binary_path="${GRANOLA_BIN:-$(command -v granola || true)}"

if [[ -z "${binary_path}" ]]; then
  printf 'could not find granola binary on PATH; run via nix run .#package-macos\n' >&2
  exit 1
fi

rm -rf "${stage_dir}"
mkdir -p "${stage_dir}" "${download_dir}"

cp "${binary_path}" "${stage_dir}/granola"
chmod 0755 "${stage_dir}/granola"
cp README.md LICENSE "${stage_dir}/"

tar \
  --sort=name \
  --format=ustar \
  --mtime='@1' \
  --owner=0 \
  --group=0 \
  --numeric-owner \
  -C "$(dirname "${stage_dir}")" \
  -czf "${download_dir}/${artifact}" \
  "$(basename "${stage_dir}")"

sha256sum "${download_dir}/${artifact}" > "${download_dir}/${artifact}.sha256"

cat > "${download_dir}/manifest.json" <<EOF
{
  "version": "${version}",
  "platform": "${platform}",
  "artifact": "${artifact}",
  "checksum": "${artifact}.sha256"
}
EOF

printf 'created %s\n' "${download_dir}/${artifact}"
printf 'created %s\n' "${download_dir}/${artifact}.sha256"
