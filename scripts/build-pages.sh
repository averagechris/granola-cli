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

download_dir="${repo_root}/dist/downloads"
site_dir="${repo_root}/dist/pages/site"
pages_tarball="${repo_root}/dist/pages/granola-cli-pages.tar.gz"

shopt -s nullglob
artifacts=("${download_dir}"/*.tar.gz)
if [[ ${#artifacts[@]} -eq 0 ]]; then
  printf 'no download artifacts found in %s; run nix run .#package-macos first\n' "${download_dir}" >&2
  exit 1
fi

artifacts_newest_first=()
for ((i = ${#artifacts[@]} - 1; i >= 0; i--)); do
  artifacts_newest_first+=("${artifacts[$i]}")
done

rm -rf "${site_dir}"
mkdir -p "${site_dir}/downloads" "$(dirname "${pages_tarball}")"

cp "${download_dir}"/* "${site_dir}/downloads/"

latest_artifact="$(basename "${artifacts_newest_first[0]}")"
latest_checksum="${latest_artifact}.sha256"

cat > "${site_dir}/index.html" <<EOF
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>granola-cli downloads</title>
  <style>
    body { font-family: system-ui, sans-serif; max-width: 760px; margin: 3rem auto; padding: 0 1rem; line-height: 1.5; }
    code, pre { background: #f4f4f4; padding: 0.15rem 0.3rem; border-radius: 4px; }
    pre { padding: 1rem; overflow-x: auto; }
    .artifact { margin: 1rem 0; padding: 1rem; border: 1px solid #ddd; border-radius: 8px; }
  </style>
</head>
<body>
  <h1>granola-cli downloads</h1>
  <p>A Rust CLI for Granola meeting notes.</p>

  <h2>What's new in v${version}</h2>
  <ul>
    <li>Add <code>--output text</code> and default single-field output to plain text for shell pipelines.</li>
    <li>Use requested note fields to plan transcript fetching, so <code>--fields transcript</code> hydrates transcript data automatically.</li>
    <li>Expose <code>granola notes fields [list|search|get]</code> for discoverable field metadata used by CLI validation and agent guidance.</li>
    <li>Support cached content fields and <code>--redact emails,phones,secrets,attendees</code> in <code>notes search</code> output.</li>
    <li>Share note field validation, projection, rendering, and transcript planning across structured note commands.</li>
  </ul>

  <h2>Binary downloads</h2>
  <p>Current packaged version: <code>v${version}</code></p>
EOF

for artifact_path in "${artifacts_newest_first[@]}"; do
  artifact="$(basename "${artifact_path}")"
  checksum="${artifact}.sha256"
  sha="$(cut -d ' ' -f1 "${download_dir}/${checksum}")"
  cat >> "${site_dir}/index.html" <<EOF
  <div class="artifact">
    <h3>${artifact}</h3>
    <ul>
      <li><a href="downloads/${artifact}">Download tarball</a></li>
      <li><a href="downloads/${checksum}">SHA-256 checksum</a></li>
    </ul>
    <pre><code>${sha}  ${artifact}</code></pre>
  </div>
EOF
done

cat >> "${site_dir}/index.html" <<EOF
  <h2>Manual install</h2>
  <pre><code>curl -LO https://averagechris.srht.site/granola-cli/downloads/${latest_artifact}
curl -LO https://averagechris.srht.site/granola-cli/downloads/${latest_checksum}
sha256sum -c ${latest_checksum}
tar -xzf ${latest_artifact}
install -m 0755 ${latest_artifact%.tar.gz}/granola ~/.local/bin/granola</code></pre>

  <h2>Nix install</h2>
  <p>If you use Nix, this is the easiest path:</p>
  <pre><code>nix run sourcehut:averagechris/granola-cli
nix profile install sourcehut:averagechris/granola-cli</code></pre>

  <h2>Source</h2>
  <p><a href="https://git.sr.ht/~averagechris/granola-cli">git.sr.ht/~averagechris/granola-cli</a></p>
</body>
</html>
EOF

tar \
  --sort=name \
  --format=ustar \
  --mtime='@1' \
  --owner=0 \
  --group=0 \
  --numeric-owner \
  -C "${site_dir}" \
  -czf "${pages_tarball}" \
  .

printf 'created %s\n' "${pages_tarball}"
