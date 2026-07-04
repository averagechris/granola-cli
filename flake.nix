{
  description = "Nix flake for granola-cli";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = import nixpkgs {inherit system;};
        lib = pkgs.lib;
        cargoToml = fromTOML (builtins.readFile ./Cargo.toml);
        cliConfig = fromTOML (builtins.readFile ./config/cli.toml);
        package = cargoToml.package;
        cliProgram = cliConfig.cli.program_name;
        granola = pkgs.rustPlatform.buildRustPackage {
          pname = cliProgram;
          version = package.version;
          src = lib.cleanSource ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = with pkgs; [cmake pkg-config];

          # reqwest's rustls-platform-verifier needs a CA bundle even to
          # construct a client; the Linux nix sandbox has no /etc/ssl, so
          # the wiremock-backed tests fail with "builder error" without this.
          nativeCheckInputs = [pkgs.cacert];
          env.SSL_CERT_FILE = "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt";

          meta = lib.attrsets.filterAttrs (_: value: value != null) {
            description = package.description or null;
            license =
              if (package.license or null) == "MIT"
              then lib.licenses.mit
              else null;
            mainProgram = cliProgram;
          };
        };
        releaseArtifactPlatform =
          if pkgs.stdenv.hostPlatform.isDarwin && pkgs.stdenv.hostPlatform.isAarch64
          then "aarch64-darwin"
          else if pkgs.stdenv.hostPlatform.isDarwin && pkgs.stdenv.hostPlatform.isx86_64
          then "x86_64-darwin"
          else if pkgs.stdenv.hostPlatform.isLinux && pkgs.stdenv.hostPlatform.isAarch64
          then "aarch64-linux"
          else if pkgs.stdenv.hostPlatform.isLinux && pkgs.stdenv.hostPlatform.isx86_64
          then "x86_64-linux"
          else null;
        releaseArtifactName =
          if releaseArtifactPlatform == null
          then null
          else "granola-cli-v${package.version}-${releaseArtifactPlatform}.tar.gz";
        releaseArtifact =
          if releaseArtifactName == null
          then null
          else
            pkgs.runCommand "granola-cli-release-artifact-${package.version}-${releaseArtifactPlatform}" {
              nativeBuildInputs = with pkgs; [
                coreutils
                gnutar
                gzip
              ];
            } ''
              mkdir -p "$out" "$TMPDIR/stage/${lib.removeSuffix ".tar.gz" releaseArtifactName}"
              stage="$TMPDIR/stage/${lib.removeSuffix ".tar.gz" releaseArtifactName}"

              cp -p ${granola}/bin/${cliProgram} "$stage/${cliProgram}"
              chmod 0555 "$stage/${cliProgram}"
              cp -p ${./README.md} "$stage/README.md"
              cp -p ${./LICENSE} "$stage/LICENSE"
              cp -p ${./CHANGELOG.md} "$stage/CHANGELOG.md"

              tar \
                --sort=name \
                --format=ustar \
                --mtime='@1' \
                --owner=0 \
                --group=0 \
                --numeric-owner \
                -C "$TMPDIR/stage" \
                -cf - \
                "${lib.removeSuffix ".tar.gz" releaseArtifactName}" | gzip -n > "$out/${releaseArtifactName}"

              sha="$(sha256sum "$out/${releaseArtifactName}" | cut -d ' ' -f1)"
              printf '%s  %s\n' "$sha" "${releaseArtifactName}" > "$out/${releaseArtifactName}.sha256"
            '';
        prepareReleaseScript = ''
          exec python3 - "$@" <<'PY'
          from __future__ import annotations

          import argparse
          import datetime as dt
          import pathlib
          import re
          import subprocess
          import sys
          import tomllib

          SEMVER_RE = re.compile(r"^v?(\d+)\.(\d+)\.(\d+)$")

          def run(args: list[str], *, check: bool = True) -> str:
              completed = subprocess.run(args, check=check, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
              return completed.stdout

          def read_version(cargo_toml: pathlib.Path) -> str:
              with cargo_toml.open("rb") as handle:
                  version = tomllib.load(handle).get("package", {}).get("version")
              if not isinstance(version, str) or not SEMVER_RE.match(version):
                  raise SystemExit("Cargo.toml package.version must be X.Y.Z")
              return version

          def write_version(cargo_toml: pathlib.Path, version: str) -> None:
              if not SEMVER_RE.match(version):
                  raise SystemExit(f"invalid semver version: {version}")
              bare = version.removeprefix("v")
              text = cargo_toml.read_text()
              updated, count = re.subn(r'(?m)^version = "[^"]+"$', f'version = "{bare}"', text, count=1)
              if count != 1:
                  raise SystemExit("could not update package.version in Cargo.toml")
              cargo_toml.write_text(updated)

          def update_linux_build_manifest(repo_root: pathlib.Path, version: str) -> None:
              manifest = repo_root / "builds" / "release-linux-x86_64.yml"
              if not manifest.exists():
                  return
              tag = f"v{version.removeprefix('v')}"
              text = manifest.read_text()
              text = re.sub(
                  r"granola-cli-v\d+\.\d+\.\d+-x86_64-linux\.tar\.gz",
                  f"granola-cli-{tag}-x86_64-linux.tar.gz",
                  text,
              )
              manifest.write_text(text)

          def update_lockfile_version(repo_root: pathlib.Path, version: str) -> None:
              lockfile = repo_root / "Cargo.lock"
              if not lockfile.exists():
                  return
              text = lockfile.read_text()
              updated, count = re.subn(
                  r'(\[\[package\]\]\nname = "granola-cli"\nversion = ")([^"]+)(")',
                  rf'\g<1>{version.removeprefix("v")}\3',
                  text,
                  count=1,
              )
              if count != 1:
                  raise SystemExit("could not update granola-cli version in Cargo.lock")
              lockfile.write_text(updated)

          def version_key(version: str) -> tuple[int, int, int]:
              match = SEMVER_RE.match(version)
              if not match:
                  raise ValueError(version)
              return tuple(int(part) for part in match.groups())

          def latest_tag_before(version: str) -> str | None:
              tags = []
              for line in run(["jj", "tag", "list", "--no-pager", "--color=never"], check=False).splitlines():
                  name = line.split(":", 1)[0].strip()
                  if SEMVER_RE.match(name) and version_key(name) < version_key(version):
                      tags.append(name)
              if not tags:
                  return None
              return sorted(tags, key=version_key)[-1]

          def commit_summaries(baseline: str | None, revision: str) -> list[str]:
              revset = f"{baseline}..{revision}" if baseline else revision
              output = run(
                  [
                      "jj", "log", "-r", revset, "--no-graph", "--color=never",
                      "-T", 'description.first_line() ++ "\\n"',
                  ],
                  check=False,
              )
              ignored = re.compile(r"^(chore|build)(\([^)]*\))?: (bump version|release)\b", re.IGNORECASE)
              return [line.strip() for line in output.splitlines() if line.strip() and not ignored.search(line)]

          def bullet_from_commit(summary: str) -> tuple[str, str]:
              match = re.match(r"^(?P<type>[a-z]+)(?:\([^)]*\))?(?P<breaking>!)?:\s*(?P<body>.+)$", summary)
              if not match:
                  return "Changed", summary[0].upper() + summary[1:]
              kind = match.group("type")
              body = match.group("body")
              sentence = body[0].upper() + body[1:]
              if match.group("breaking"):
                  return "Breaking", sentence
              if kind == "feat":
                  return "Added", sentence
              if kind == "fix":
                  return "Fixed", sentence
              if kind == "docs":
                  return "Documentation", sentence
              if kind == "perf":
                  return "Performance", sentence
              return "Changed", sentence

          def generated_changelog(commits: list[str]) -> str:
              if not commits:
                  return "### Changed\n\n- Maintenance release.\n"
              sections: dict[str, list[str]] = {}
              for summary in commits:
                  section, bullet = bullet_from_commit(summary)
                  sections.setdefault(section, []).append(bullet.rstrip("."))
              order = ["Breaking", "Added", "Changed", "Fixed", "Performance", "Documentation"]
              parts: list[str] = []
              for section in order:
                  bullets = sections.get(section)
                  if not bullets:
                      continue
                  parts.append(f"### {section}\n")
                  parts.extend(f"- {bullet}." for bullet in bullets)
                  parts.append("")
              return "\n".join(parts).rstrip() + "\n"

          def unreleased_body(text: str) -> tuple[tuple[int, int] | None, str]:
              match = re.search(r"(?m)^## Unreleased\s*$", text)
              if not match:
                  return None, ""
              next_heading = re.search(r"(?m)^## ", text[match.end():])
              body_start = match.end()
              body_end = match.end() + next_heading.start() if next_heading else len(text)
              return (body_start, body_end), text[body_start:body_end].strip()

          def update_changelog(changelog: pathlib.Path, version: str, date: str, commits: list[str]) -> None:
              tag = f"v{version.removeprefix('v')}"
              entry_heading = f"## {tag} - {date}"
              if not changelog.exists():
                  changelog.write_text(f"# Changelog\n\n## Unreleased\n\n{entry_heading}\n\n{generated_changelog(commits)}")
                  return
              text = changelog.read_text()
              if re.search(rf"(?m)^## {re.escape(tag)}(?:\s+-\s+.*)?$", text):
                  return
              body_range, body = unreleased_body(text)
              entry_body = body if body else generated_changelog(commits).strip()
              entry = f"{entry_heading}\n\n{entry_body}\n"
              if body_range:
                  start, end = body_range
                  text = text[:start] + f"\n\n{entry}\n" + text[end:].lstrip("\n")
              else:
                  if not text.startswith("# Changelog"):
                      text = "# Changelog\n\n" + text
                  text = text.rstrip() + f"\n\n## Unreleased\n\n{entry}"
              changelog.write_text(text.rstrip() + "\n")

          def main() -> int:
              parser = argparse.ArgumentParser(description="Prepare Cargo.toml and CHANGELOG.md for a deterministic release.")
              parser.add_argument("--version", help="release version to write; defaults to Cargo.toml package.version")
              parser.add_argument("--revision", default="@", help="jj revision to summarize for the changelog")
              parser.add_argument("--date", default=dt.date.today().isoformat(), help="release date for CHANGELOG.md")
              parser.add_argument("--repo-root", default=".", help="repository root")
              args = parser.parse_args()
              repo_root = pathlib.Path(args.repo_root).resolve()
              cargo_toml = repo_root / "Cargo.toml"
              version = args.version.removeprefix("v") if args.version else read_version(cargo_toml)
              if args.version:
                  write_version(cargo_toml, version)
              update_lockfile_version(repo_root, version)
              update_linux_build_manifest(repo_root, version)
              baseline = latest_tag_before(version)
              commits = commit_summaries(baseline, args.revision)
              update_changelog(repo_root / "CHANGELOG.md", version, args.date, commits)
              print(f"prepared {version}")
              if baseline:
                  print(f"baseline: {baseline}")
              print(f"changelog commits: {len(commits)}")
              return 0

          sys.exit(main())
          PY
        '';
        releaseTagScript = ''
          if [[ $# -eq 1 && ( "$1" == "-h" || "$1" == "--help" ) ]]; then
            printf 'usage: %s [--revision REV]\n' "$0"
            printf 'Create and push a release tag from Cargo.toml version.\n\n'
            printf 'In jj repos, uses jj tag set + git push.\n'
            printf 'In plain git repos, uses git tag + git push.\n'
            exit 0
          fi

          revision="@"
          while [[ $# -gt 0 ]]; do
            case "$1" in
              --revision) revision="$2"; shift 2 ;;
              *) printf 'unknown argument: %s\n' "$1" >&2; exit 1 ;;
            esac
          done

          repo_root="$(git rev-parse --show-toplevel 2>/dev/null || jj workspace root)"

          version="$(${pkgs.python3}/bin/python3 -c '
          import pathlib, sys, tomllib
          path = pathlib.Path(sys.argv[1])
          with path.open("rb") as f:
              data = tomllib.load(f)
          version = data.get("package", {}).get("version")
          if not isinstance(version, str) or not version:
              raise SystemExit("Cargo.toml is missing package.version")
          print(version)
          ' "$repo_root/Cargo.toml")"

          if [[ "$version" =~ ^v?[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
            tag="v''${version#v}"
          else
            printf 'Cargo.toml version must be semver in the form X.Y.Z\n' >&2
            exit 1
          fi

          if git rev-parse --verify --quiet "refs/tags/$tag" >/dev/null; then
            printf 'local tag already exists: %s\n' "$tag" >&2
            exit 1
          fi

          if [[ -n "$(git ls-remote --tags origin "refs/tags/$tag" 2>/dev/null)" ]]; then
            printf 'remote tag already exists on origin: %s\n' "$tag" >&2
            exit 1
          fi

          if [[ -d .jj ]]; then
            if [[ "$(jj log -r "$revision" --no-graph --color=never -T 'empty()' 2>/dev/null)" == "true" ]]; then
              printf 'refusing to tag empty jj revision: %s\n' "$revision" >&2
              printf 'pass the non-empty release revision explicitly (for example --revision @-)\n' >&2
              exit 1
            fi
            jj tag set "$tag" --revision "$revision" --no-pager --color=never
            printf 'created tag %s via jj\n' "$tag"
          else
            if ! git diff --quiet || ! git diff --cached --quiet; then
              printf 'working tree must be clean before tagging\n' >&2
              exit 1
            fi
            git tag -a "$tag" HEAD -m "Release $tag"
            printf 'created annotated tag %s\n' "$tag"
          fi

          if ! git push origin "refs/tags/$tag"; then
            printf 'failed to push %s. run manually:\n  git push origin "refs/tags/%s"\n' "$tag" "$tag" >&2
            exit 1
          fi

          printf 'pushed %s to origin\n' "$tag"
        '';
        releaseScript = ''
          repo_root="$(git rev-parse --show-toplevel 2>/dev/null || jj workspace root)"
          cd "$repo_root"

          version=""
          revision="@"
          validate=1
          tag_release=1
          build_artifact=1
          build_pages=1
          publish_pages=0
          submit_linux_build=0
          domain="averagechris.srht.site"
          subdirectory="/granola-cli"
          linux_manifest="builds/release-linux-x86_64.yml"

          usage() {
            cat <<'EOF'
          usage: release [options]

          Prepare changelog/version metadata, validate, tag, build local release artifacts,
          build SourceHut Pages content, and optionally publish/submit Linux builds.

          Options:
            --version X.Y.Z            update Cargo.toml before preparing the release
            --revision REV             jj revision to tag/summarize (default: @)
            --skip-validate            skip nix flake check, ci-test, and ci-clippy
            --skip-tag                 do not create/push the release tag
            --skip-artifact            do not build/copy the local release artifact
            --skip-pages               do not build the static downloads page
            --publish-pages            publish dist/pages/granola-cli-pages.tar.gz with hut
            --submit-linux-build       submit builds/release-linux-x86_64.yml with hut
            --domain DOMAIN            SourceHut Pages domain (default: averagechris.srht.site)
            --subdirectory PATH        SourceHut Pages subdirectory (default: /granola-cli)
            -h, --help                 show this help
          EOF
          }

          while [[ $# -gt 0 ]]; do
            case "$1" in
              --version) version="$2"; shift 2 ;;
              --revision) revision="$2"; shift 2 ;;
              --skip-validate) validate=0; shift ;;
              --skip-tag) tag_release=0; shift ;;
              --skip-artifact) build_artifact=0; shift ;;
              --skip-pages) build_pages=0; shift ;;
              --publish-pages) publish_pages=1; shift ;;
              --submit-linux-build) submit_linux_build=1; shift ;;
              --domain) domain="$2"; shift 2 ;;
              --subdirectory) subdirectory="$2"; shift 2 ;;
              -h|--help) usage; exit 0 ;;
              *) printf 'unknown argument: %s\n' "$1" >&2; usage >&2; exit 1 ;;
            esac
          done

          prepare_args=("--revision" "$revision" "--repo-root" "$repo_root")
          if [[ -n "$version" ]]; then
            prepare_args+=("--version" "$version")
          fi
          prepare-release "''${prepare_args[@]}"

          cargo check --locked --quiet

          version="$(python3 - <<'PY'
          import pathlib, tomllib
          with pathlib.Path('Cargo.toml').open('rb') as handle:
              print(tomllib.load(handle)['package']['version'])
          PY
          )"
          tag="v$version"

          # Ensure the release commit is described before tagging: prepare-release
          # stages version/changelog changes into the working copy, which may be a
          # fresh commit with no description — release-tag must never tag an
          # undescribed commit.
          if [[ -d .jj ]]; then
            if [[ -z "$(jj log -r "$revision" --no-graph --color=never -T 'description' 2>/dev/null)" ]]; then
              jj describe "$revision" -m "chore: release $tag" --no-pager --color=never
            fi
          elif ! git diff --quiet || ! git diff --cached --quiet; then
            git add -A
            git commit -m "chore: release $tag"
          fi

          if [[ $validate -eq 1 ]]; then
            nix flake check --no-write-lock-file
            nix run .#ci-test
            nix run .#ci-clippy
          fi

          if [[ $tag_release -eq 1 ]]; then
            release_revision="$(jj log -r "$revision" --no-graph --color=never -T 'commit_id.short()')"
            nix run .#release-tag -- --revision "$release_revision"
            if [[ -d .jj ]]; then
              jj bookmark set main --revision "$release_revision" --no-pager --color=never
              jj git push --remote origin --bookmark main --no-pager --color=never
            fi
          fi

          if [[ $build_artifact -eq 1 ]]; then
            nix build .#release-artifact --out-link result-release-artifact
            mkdir -p dist/downloads
            cp -p result-release-artifact/* dist/downloads/
            printf 'copied release artifact(s) to dist/downloads\n'
          fi

          if [[ $build_pages -eq 1 ]]; then
            nix run .#build-pages -- --domain "$domain" --subdirectory "$subdirectory" --include-existing-downloads
          fi

          if [[ $publish_pages -eq 1 ]]; then
            nix run .#publish-pages -- --domain "$domain" --subdirectory "$subdirectory"
          else
            printf 'pages not published; run: nix run .#publish-pages -- --domain %q --subdirectory %q\n' "$domain" "$subdirectory"
          fi

          if [[ $submit_linux_build -eq 1 ]]; then
            hut builds submit "$linux_manifest" --note "granola-cli $tag linux release" --tags "granola-cli/$tag/release" --visibility unlisted
          else
            printf 'linux build not submitted; run: hut builds submit %s --note %q --tags %q --visibility unlisted\n' \
              "$linux_manifest" "granola-cli $tag linux release" "granola-cli/$tag/release"
          fi
        '';
        mkRepoScript = {
          name,
          runtimeInputs ? [],
          text,
        }:
          pkgs.writeShellApplication {
            inherit name runtimeInputs text;
          };
        ciFmt = mkRepoScript {
          name = "ci-fmt";
          runtimeInputs = with pkgs; [alejandra cargo rustfmt];
          text = ''
            cargo fmt --all --check
            alejandra --check flake.nix
          '';
        };
        ciClippy = mkRepoScript {
          name = "ci-clippy";
          runtimeInputs = with pkgs; [cargo clippy rustc];
          text = ''
            cargo clippy --locked --all-targets -- -D warnings
          '';
        };
        ciTest = mkRepoScript {
          name = "ci-test";
          runtimeInputs = with pkgs; [cargo rustc];
          text = ''
            cargo test --locked
          '';
        };
        ciAudit = mkRepoScript {
          name = "ci-audit";
          runtimeInputs = with pkgs; [cargo cargo-audit];
          text = ''
            cargo audit --deny warnings
          '';
        };
        ciDeny = mkRepoScript {
          name = "ci-deny";
          runtimeInputs = with pkgs; [cargo cargo-deny];
          text = ''
            cargo deny check
          '';
        };
        ciMachete = mkRepoScript {
          name = "ci-machete";
          runtimeInputs = with pkgs; [cargo cargo-machete];
          text = ''
            cargo machete
          '';
        };
        ciVet = mkRepoScript {
          name = "ci-vet";
          runtimeInputs = with pkgs; [cargo cargo-vet];
          text = ''
            cargo vet --locked
          '';
        };
        # Deprecated alias for the old macOS packaging flow. The canonical
        # artifact builder is `nix build .#release-artifact`; this wrapper
        # keeps `nix run .#package-macos` working by delegating to it.
        packageMacos = mkRepoScript {
          name = "package-macos";
          runtimeInputs = with pkgs; [
            coreutils
            git
            nix
          ];
          text = ''
            exec bash ./scripts/package-macos.sh "$@"
          '';
        };
        buildPages = mkRepoScript {
          name = "build-pages";
          runtimeInputs = with pkgs; [
            coreutils
            git
            gnutar
            gzip
            python3
          ];
          text = ''
            exec bash ./scripts/build-pages.sh "$@"
          '';
        };
        publishPages = mkRepoScript {
          name = "publish-pages";
          runtimeInputs = with pkgs; [
            hut
          ];
          text = ''
            exec bash ./scripts/publish-pages.sh "$@"
          '';
        };
        prepareRelease = mkRepoScript {
          name = "prepare-release";
          runtimeInputs = with pkgs; [
            jujutsu
            python3
          ];
          text = prepareReleaseScript;
        };
        releaseTag = mkRepoScript {
          name = "release-tag";
          runtimeInputs = with pkgs; [
            git
            jujutsu
            python3
          ];
          text = releaseTagScript;
        };
        release = mkRepoScript {
          name = "release";
          runtimeInputs = with pkgs; [
            cargo
            cmake
            coreutils
            git
            hut
            jujutsu
            nix
            pkg-config
            prepareRelease
            python3
            rustc
          ];
          text = releaseScript;
        };
        repoScripts = pkgs.symlinkJoin {
          name = "granola-cli-scripts";
          paths = [buildPages ciAudit ciClippy ciDeny ciFmt ciMachete ciTest ciVet packageMacos prepareRelease publishPages release releaseTag];
        };
        fmtCheck =
          pkgs.runCommand "granola-cli-fmt-check" {
            nativeBuildInputs = [ciFmt];
            src = lib.cleanSource ./.;
          } ''
            export HOME="$TMPDIR"
            cp -R "$src" source
            chmod -R +w source
            cd source
            ci-fmt
            mkdir -p "$out"
          '';
        # `nix fmt` invokes the formatter app without path arguments. Alejandra
        # treats no arguments as "format stdin", which fails on empty stdin, so
        # keep the formatter as Alejandra but default it to formatting the repo.
        nixFormatter = pkgs.writeShellApplication {
          name = "alejandra";
          runtimeInputs = with pkgs; [
            alejandra
          ];
          text = ''
            if [[ $# -eq 0 ]]; then
              exec alejandra .
            fi

            exec alejandra "$@"
          '';
        };
      in {
        formatter = nixFormatter;

        packages =
          {
            default = granola;
            granola = granola;
            ci-audit = ciAudit;
            ci-fmt = ciFmt;
            ci-clippy = ciClippy;
            ci-deny = ciDeny;
            ci-machete = ciMachete;
            ci-test = ciTest;
            ci-vet = ciVet;
            package-macos = packageMacos;
            build-pages = buildPages;
            prepare-release = prepareRelease;
            publish-pages = publishPages;
            release = release;
            release-tag = releaseTag;
            scripts = repoScripts;
          }
          // lib.optionalAttrs (releaseArtifact != null) {
            "release-artifact" = releaseArtifact;
          };

        apps.default = flake-utils.lib.mkApp {
          drv = granola;
          exePath = "/bin/${cliProgram}";
        };
        apps.granola = flake-utils.lib.mkApp {
          drv = granola;
          exePath = "/bin/${cliProgram}";
        };
        apps.ci-audit = flake-utils.lib.mkApp {drv = ciAudit;};
        apps.ci-fmt = flake-utils.lib.mkApp {drv = ciFmt;};
        apps.ci-clippy = flake-utils.lib.mkApp {drv = ciClippy;};
        apps.ci-deny = flake-utils.lib.mkApp {drv = ciDeny;};
        apps.ci-machete = flake-utils.lib.mkApp {drv = ciMachete;};
        apps.ci-test = flake-utils.lib.mkApp {drv = ciTest;};
        apps.ci-vet = flake-utils.lib.mkApp {drv = ciVet;};
        apps.package-macos = flake-utils.lib.mkApp {drv = packageMacos;};
        apps.build-pages = flake-utils.lib.mkApp {drv = buildPages;};
        apps.prepare-release = flake-utils.lib.mkApp {drv = prepareRelease;};
        apps.publish-pages = flake-utils.lib.mkApp {drv = publishPages;};
        apps.release = flake-utils.lib.mkApp {drv = release;};
        apps.release-tag = flake-utils.lib.mkApp {drv = releaseTag;};

        checks =
          {
            build = granola;
            fmt = fmtCheck;
          }
          // lib.optionalAttrs (releaseArtifact != null) {
            "release-artifact" = releaseArtifact;
          };

        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            alejandra
            cargo
            cargo-audit
            cargo-deny
            cargo-machete
            cargo-outdated
            cargo-vet
            cmake
            clippy
            direnv
            hut
            jujutsu
            nixd
            pkg-config
            rust-analyzer
            rustc
            rustfmt
            repoScripts
          ];
        };
      }
    );
}
