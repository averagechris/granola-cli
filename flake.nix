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

          nativeBuildInputs = with pkgs; [pkg-config];

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
          runtimeInputs = with pkgs; [cargo-audit];
          text = ''
            cargo audit --deny warnings
          '';
        };
        ciDeny = mkRepoScript {
          name = "ci-deny";
          runtimeInputs = with pkgs; [cargo-deny];
          text = ''
            cargo deny check
          '';
        };
        repoScripts = pkgs.symlinkJoin {
          name = "granola-cli-scripts";
          paths = [ciAudit ciClippy ciDeny ciFmt ciTest];
        };
      in {
        packages = {
          default = granola;
          granola = granola;
          ci-audit = ciAudit;
          ci-fmt = ciFmt;
          ci-clippy = ciClippy;
          ci-deny = ciDeny;
          ci-test = ciTest;
          scripts = repoScripts;
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
        apps.ci-test = flake-utils.lib.mkApp {drv = ciTest;};

        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            alejandra
            cargo
            cargo-audit
            cargo-deny
            clippy
            direnv
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
