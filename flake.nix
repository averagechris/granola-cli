{
  description = "Nix flake for granola-cli";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fleet.url = "git+https://git.sr.ht/~averagechris/averagechris.srht.site";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    fleet,
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = import nixpkgs {inherit system;};
        lib = pkgs.lib;
        cargoToml = fromTOML (builtins.readFile ./Cargo.toml);
        cliConfig = fromTOML (builtins.readFile ./config/cli.toml);
        package = cargoToml.package;
        cliProgram = cliConfig.cli.program_name;
        fleetApps = fleet.lib.fleet.presets.rust {
          inherit pkgs self;
          srhtPackage = fleet.packages.${system}.srht;
          pname = "granola-cli";
          binaries = ["granola"];
          subdir = "granola-cli";
          srhtRepo = "granola-cli";
          versionMode = "package";
          versionFile = "Cargo.toml";
          lockPackages = ["granola-cli"];
          extraStaticChecks = [ciMachete];
        };
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
        mkRepoScript = {
          name,
          runtimeInputs ? [],
          text,
        }:
          pkgs.writeShellApplication {
            inherit name runtimeInputs text;
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
          runtimeInputs = with pkgs; [hut];
          text = ''
            exec bash ./scripts/publish-pages.sh "$@"
          '';
        };
        repoScripts = pkgs.symlinkJoin {
          name = "granola-cli-scripts";
          paths =
            [
              buildPages
              ciAudit
              ciDeny
              ciMachete
              ciVet
              packageMacos
              publishPages
            ]
            ++ [
              fleetApps.apps."ci-clippy".program
              fleetApps.apps."ci-fmt".program
              fleetApps.apps."ci-test".program
              fleetApps.apps."prepare-release".program
              fleetApps.apps.release.program
              fleetApps.apps."release-tag".program
              fleetApps.apps."static-checks".program
            ];
        };
        fmtCheck =
          pkgs.runCommand "granola-cli-fmt-check" {
            src = lib.cleanSource ./.;
          } ''
            export HOME="$TMPDIR"
            cp -R "$src" source
            chmod -R +w source
            cd source
            ${fleetApps.apps.ci-fmt.program}
            mkdir -p "$out"
          '';
        # `nix fmt` invokes the formatter app without path arguments. Alejandra
        # treats no arguments as "format stdin", which fails on empty stdin, so
        # keep the formatter as Alejandra but default it to formatting the repo.
        nixFormatter = pkgs.writeShellApplication {
          name = "alejandra";
          runtimeInputs = with pkgs; [alejandra];
          text = ''
            if [[ $# -eq 0 ]]; then
              exec alejandra .
            fi

            exec alejandra "$@"
          '';
        };
      in {
        formatter = nixFormatter;

        packages = {
          default = granola;
          granola = granola;
          ci-audit = ciAudit;
          ci-deny = ciDeny;
          ci-machete = ciMachete;
          ci-vet = ciVet;
          package-macos = packageMacos;
          build-pages = buildPages;
          publish-pages = publishPages;
          scripts = repoScripts;
          release-artifact = fleetApps.releaseArtifact system;
        };

        apps =
          {
            default = flake-utils.lib.mkApp {
              drv = granola;
              exePath = "/bin/${cliProgram}";
            };
            granola = flake-utils.lib.mkApp {
              drv = granola;
              exePath = "/bin/${cliProgram}";
            };
            ci-audit = flake-utils.lib.mkApp {drv = ciAudit;};
            ci-deny = flake-utils.lib.mkApp {drv = ciDeny;};
            ci-machete = flake-utils.lib.mkApp {drv = ciMachete;};
            ci-vet = flake-utils.lib.mkApp {drv = ciVet;};
            package-macos = flake-utils.lib.mkApp {drv = packageMacos;};
            build-pages = flake-utils.lib.mkApp {drv = buildPages;};
            publish-pages = flake-utils.lib.mkApp {drv = publishPages;};
          }
          // fleetApps.apps;

        checks = {
          build = granola;
          fmt = fmtCheck;
          release-artifact = fleetApps.releaseArtifact system;
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
