## Granola CLI Project Guidance

Use `jj` for version-control actions in this repository.

### Development

- Enter the toolchain with `direnv allow` or `nix develop`.
- Prefer local checks: `nix run .#ci-fmt`, `nix run .#ci-clippy`, `nix run .#ci-test`, `nix run .#ci-deny`, `nix run .#ci-audit`.
- Keep credentials in the OS keyring only. Do not add plaintext credential storage.
- Keep command output agent-friendly: every data command should support `--output json`, `--output json-compact`, `--fields`, and quiet/no-decorative output where practical.

### Product Constraints

- Granola's documented public API currently uses API keys, not a public OAuth flow.
- Browser-based OAuth should remain an auth abstraction target, not an implementation promise, until Granola documents a CLI-compatible OAuth flow.
- Respect API rate limits: 25-request burst, 5 requests/second sustained.
- Notes APIs only return notes with generated AI summary and transcript.
