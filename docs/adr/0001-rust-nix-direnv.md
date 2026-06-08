# ADR 0001: Rust with Nix and direnv

## Status

Accepted.

## Context

The CLI should be fast, distributable, agent-friendly, and pleasant to develop locally. `linear-cli` already establishes the preferred developer workflow: Rust, Nix flakes, direnv, and local CI scripts.

## Decision

Build the CLI in Rust. Use a Nix flake for the development shell and repo-local check commands. Use `.envrc` with `use flake` for direnv integration.

## Consequences

- Contributors get a reproducible Rust toolchain with `nix develop` or `direnv allow`.
- Local checks are explicit and package-manager independent.
- The flake provides both a development shell and an initial package/app output for `granola`.
