# ADR 0002: Keyring-first authentication

## Status

Accepted.

## Context

Granola's public API documentation currently supports bearer API keys. The docs also mention browser-based OAuth for Granola MCP, but do not document a public OAuth flow for arbitrary CLI clients.

## Decision

Implement API-key authentication first:

- `granola auth login` prompts for an API key and stores it in the OS keyring.
- `granola auth logout` removes the keyring entry.
- `granola auth status` reports whether credentials are present without printing secrets.
- `--api-key` may override credentials for a single process invocation only.
- Plaintext secret config and environment-variable based persistent auth are out of scope.

Keep the auth module shaped so a future OAuth provider can be added if Granola documents a CLI-compatible OAuth flow.

## Consequences

- Initial auth works with the documented API.
- Credentials are not stored in repo files or plaintext config.
- Browser OAuth remains possible later without overpromising it now.
