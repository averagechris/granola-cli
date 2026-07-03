#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

domain="averagechris.srht.site"
subdirectory="/granola-cli"
include_existing_downloads=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --domain) domain="$2"; shift 2 ;;
    --subdirectory) subdirectory="$2"; shift 2 ;;
    --include-existing-downloads) include_existing_downloads=1; shift ;;
    -h|--help)
      printf 'usage: build-pages [--domain DOMAIN] [--subdirectory PATH] [--include-existing-downloads]\n'
      exit 0
      ;;
    *) printf 'unknown argument: %s\n' "$1" >&2; exit 1 ;;
  esac
done

export GRANOLA_PAGES_DOMAIN="$domain"
export GRANOLA_PAGES_SUBDIRECTORY="$subdirectory"
export GRANOLA_INCLUDE_EXISTING_DOWNLOADS="$include_existing_downloads"

exec python3 - <<'PY'
from __future__ import annotations

import html
import json
import os
import pathlib
import re
import shutil
import subprocess
import tomllib
import urllib.error
import urllib.request

repo = pathlib.Path.cwd()
download_dir = repo / "dist" / "downloads"
site_dir = repo / "dist" / "pages" / "site"
pages_tarball = repo / "dist" / "pages" / "granola-cli-pages.tar.gz"

with (repo / "Cargo.toml").open("rb") as handle:
    version = tomllib.load(handle)["package"]["version"]
tag = f"v{version}"
domain = os.environ["GRANOLA_PAGES_DOMAIN"].rstrip("/")
subdirectory = "/" + os.environ["GRANOLA_PAGES_SUBDIRECTORY"].strip("/")
base_url = f"https://{domain}{subdirectory}"
include_existing_downloads = os.environ["GRANOLA_INCLUDE_EXISTING_DOWNLOADS"] == "1"

download_dir.mkdir(parents=True, exist_ok=True)

# Drop the legacy single-artifact manifest emitted by the old package-macos
# flow; the pages manifest.json below supersedes it.
legacy_manifest = download_dir / "manifest.json"
if legacy_manifest.exists():
    legacy_manifest.unlink()


def include_existing_downloads_from_pages() -> None:
    manifest_url = f"{base_url}/manifest.json"
    try:
        with urllib.request.urlopen(manifest_url, timeout=30) as response:
            manifest = json.load(response)
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return
        raise
    except urllib.error.URLError as error:
        raise SystemExit(f"failed to fetch existing downloads manifest {manifest_url}: {error}") from error

    for artifact in manifest.get("artifacts", []):
        name = artifact.get("name")
        url = artifact.get("url")
        if not isinstance(name, str) or not isinstance(url, str):
            continue
        artifact_path = download_dir / name
        checksum_path = download_dir / f"{name}.sha256"
        if not artifact_path.exists():
            print(f"fetching existing download {name}")
            urllib.request.urlretrieve(url, artifact_path)
        if not checksum_path.exists():
            sha = artifact.get("sha256")
            if isinstance(sha, str) and sha:
                checksum_path.write_text(f"{sha}  {name}\n")
            else:
                urllib.request.urlretrieve(f"{url}.sha256", checksum_path)


if include_existing_downloads:
    include_existing_downloads_from_pages()


def artifact_sort_key(path: pathlib.Path) -> tuple[int, int, int, str]:
    match = re.match(r"granola-cli-v(\d+)\.(\d+)\.(\d+)-(.+)\.tar\.gz$", path.name)
    if not match:
        return (-1, -1, -1, path.name)
    major, minor, patch, platform = match.groups()
    return (int(major), int(minor), int(patch), platform)


artifacts = sorted(download_dir.glob("*.tar.gz"), key=artifact_sort_key, reverse=True)
if not artifacts:
    raise SystemExit(f"no download artifacts found in {download_dir}; run nix build .#release-artifact first")

if site_dir.exists():
    shutil.rmtree(site_dir)
(site_dir / "downloads").mkdir(parents=True)
pages_tarball.parent.mkdir(parents=True, exist_ok=True)

for path in sorted(download_dir.iterdir()):
    if path.is_file():
        shutil.copy2(path, site_dir / "downloads" / path.name)


def current_changelog() -> str:
    path = repo / "CHANGELOG.md"
    if not path.exists():
        return "- See the tagged commit history for this release."
    text = path.read_text()
    match = re.search(rf"(?m)^## {re.escape(tag)}(?:\s+-\s+.*)?\s*$", text)
    if not match:
        return "- See the tagged commit history for this release."
    next_match = re.search(r"(?m)^## ", text[match.end():])
    end = match.end() + next_match.start() if next_match else len(text)
    body = text[match.end():end].strip()
    return body or "- Maintenance release."


def markdownish_to_html(markdown: str) -> str:
    lines = markdown.splitlines()
    out: list[str] = []
    in_list = False
    for line in lines:
        if line.startswith("### "):
            if in_list:
                out.append("</ul>")
                in_list = False
            out.append(f"<h3>{html.escape(line[4:])}</h3>")
        elif line.startswith("- "):
            if not in_list:
                out.append("<ul>")
                in_list = True
            out.append(f"<li>{html.escape(line[2:])}</li>")
        elif line.strip():
            if in_list:
                out.append("</ul>")
                in_list = False
            out.append(f"<p>{html.escape(line.strip())}</p>")
    if in_list:
        out.append("</ul>")
    return "\n".join(out)


def artifact_info(path: pathlib.Path) -> dict[str, str]:
    match = re.match(r"granola-cli-(v\d+\.\d+\.\d+)-(.+)\.tar\.gz$", path.name)
    if not match:
        return {"version": "other", "platform": path.name.removesuffix(".tar.gz")}
    release_version, platform = match.groups()
    return {"version": release_version, "platform": platform}


def platform_label(platform: str) -> str:
    labels = {
        "aarch64-darwin": "macOS Apple silicon",
        "x86_64-darwin": "macOS Intel",
        "aarch64-linux": "Linux aarch64",
        "x86_64-linux": "Linux x86_64",
    }
    return labels.get(platform, platform.replace("-", " "))


def build_count_label(count: int) -> str:
    return f"{count} build" if count == 1 else f"{count} builds"


latest = artifacts[0]
latest_checksum = latest.name + ".sha256"
artifact_groups: dict[str, list[dict[str, str]]] = {}
manifest = {"version": tag, "artifacts": []}
for artifact in artifacts:
    checksum_path = download_dir / f"{artifact.name}.sha256"
    if not checksum_path.exists():
        raise SystemExit(f"missing checksum for {artifact.name}: {checksum_path}")
    sha = checksum_path.read_text().split()[0]
    info = artifact_info(artifact)
    artifact_groups.setdefault(info["version"], []).append({
        "name": artifact.name,
        "platform": info["platform"],
        "sha": sha,
    })
    manifest["artifacts"].append({
        "name": artifact.name,
        "url": f"{base_url}/downloads/{artifact.name}",
        "sha256": sha,
    })

latest_version = tag if tag in artifact_groups else artifact_info(latest)["version"]


def render_build(build: dict[str, str]) -> str:
    name = build["name"]
    sha = build["sha"]
    return f"""
      <article class="build">
        <h4>{html.escape(platform_label(build['platform']))}</h4>
        <p class="filename"><code>{html.escape(name)}</code></p>
        <p class="download-links">
          <a class="primary-link" href="downloads/{html.escape(name)}">Download tarball</a>
          <a href="downloads/{html.escape(name)}.sha256">Checksum</a>
        </p>
        <details>
          <summary>SHA-256</summary>
          <pre><code>{html.escape(sha)}  {html.escape(name)}</code></pre>
        </details>
      </article>"""


def render_release(release_version: str, builds: list[dict[str, str]], *, latest_release: bool) -> str:
    builds_html = "".join(render_build(build) for build in builds)
    label = "Latest release" if latest_release else "Release"
    latest_badge = "<span class=\"badge\">Latest</span>" if latest_release else ""
    title_html = f"{html.escape(release_version)} {latest_badge}" if latest_release else html.escape(release_version)
    class_names = "release latest" if latest_release else "release"
    return f"""
    <section class="{class_names}">
      <div class="release-heading">
        <div>
          <p class="eyebrow">{label}</p>
          <h3>{title_html}</h3>
        </div>
        <span class="build-count">{html.escape(build_count_label(len(builds)))}</span>
      </div>
      <div class="build-grid">
        {builds_html}
      </div>
    </section>"""


latest_downloads = render_release(latest_version, artifact_groups[latest_version], latest_release=True)
previous_downloads = "".join(
    render_release(release_version, builds, latest_release=False)
    for release_version, builds in artifact_groups.items()
    if release_version != latest_version
)
previous_downloads_section = f"""
  <h3 class="previous-heading">Previous releases</h3>
  {previous_downloads}""" if previous_downloads else ""

(site_dir / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
(site_dir / "index.html").write_text(f"""<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>granola-cli downloads</title>
  <style>
    body {{ font-family: system-ui, sans-serif; max-width: 920px; margin: 3rem auto; padding: 0 1rem; line-height: 1.5; }}
    code, pre {{ background: #f4f4f4; padding: 0.15rem 0.3rem; border-radius: 4px; }}
    pre {{ padding: 1rem; overflow-x: auto; }}
    .release {{ margin: 1rem 0 1.5rem; padding: 1rem; border: 1px solid #ddd; border-radius: 12px; }}
    .release.latest {{ background: #f7fbff; border-color: #9bc7f5; box-shadow: 0 1px 8px rgba(32, 105, 180, 0.12); }}
    .release-heading {{ display: flex; justify-content: space-between; gap: 1rem; align-items: flex-start; margin-bottom: 1rem; }}
    .release-heading h3 {{ margin: 0.1rem 0 0; }}
    .eyebrow {{ color: #555; font-size: 0.85rem; font-weight: 700; letter-spacing: 0.04em; margin: 0; text-transform: uppercase; }}
    .badge, .build-count {{ border-radius: 999px; display: inline-block; font-size: 0.8rem; font-weight: 700; padding: 0.15rem 0.5rem; white-space: nowrap; }}
    .badge {{ background: #0b66c3; color: white; margin-left: 0.35rem; vertical-align: middle; }}
    .build-count {{ background: #eee; color: #333; }}
    .build-grid {{ display: grid; gap: 1rem; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); }}
    .build {{ background: white; border: 1px solid #e5e5e5; border-radius: 10px; padding: 1rem; }}
    .build h4 {{ margin: 0 0 0.5rem; }}
    .filename {{ margin: 0 0 0.75rem; overflow-wrap: anywhere; }}
    .download-links {{ display: flex; flex-wrap: wrap; gap: 0.75rem; margin: 0.75rem 0; }}
    .primary-link {{ font-weight: 700; }}
    details pre {{ margin-bottom: 0; }}
    .previous-heading {{ margin-top: 2rem; }}
  </style>
</head>
<body>
  <h1>granola-cli downloads</h1>
  <p>A Rust CLI for Granola meeting notes.</p>
  <p><a href="https://git.sr.ht/~averagechris/granola-cli">Source repository</a></p>

  <h2>What's new in {html.escape(tag)}</h2>
  {markdownish_to_html(current_changelog())}

  <h2>Binary downloads</h2>
  <p>Choose the build for your platform. The latest release is highlighted first; older releases are grouped below by version.</p>
  {latest_downloads}
  {previous_downloads_section}

  <h2>Manual install</h2>
  <pre><code>curl -LO {html.escape(base_url)}/downloads/{html.escape(latest.name)}
curl -LO {html.escape(base_url)}/downloads/{html.escape(latest_checksum)}
sha256sum -c {html.escape(latest_checksum)}
tar -xzf {html.escape(latest.name)}
install -m 0755 {html.escape(latest.name.removesuffix('.tar.gz'))}/granola ~/.local/bin/granola</code></pre>

  <h2>Nix install</h2>
  <pre><code>nix run sourcehut:averagechris/granola-cli
nix profile install sourcehut:averagechris/granola-cli</code></pre>
</body>
</html>
""")

subprocess.run([
    "tar", "--sort=name", "--format=ustar", "--mtime=@1", "--owner=0", "--group=0", "--numeric-owner",
    "-C", str(site_dir), "-czf", str(pages_tarball), "."
], check=True)
print(f"created {pages_tarball}")
PY
