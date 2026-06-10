# ADR 0003: Agent-friendly CLI contract

## Status

Accepted.

## Context

This CLI should be useful for humans and AI agents. `linear-cli` shows that stable JSON, quiet modes, concise commands, and clear exit codes reduce integration friction.

## Decision

Every data command should support:

- `--output table|list|json|json-compact|json-pretty`
- `--fields` for JSON field projection where practical
- `--quiet` for suppressing decorative output

Errors should map to stable exit codes and JSON error objects when JSON output is requested.

Proposed exit codes:

- `0`: success
- `1`: general error
- `2`: not found
- `3`: auth error
- `4`: rate limited
- `5`: invalid input

## Consequences

- Scripts and agents can consume output predictably.
- Human output can remain friendly without compromising automation.
- Tests must cover output contracts, not just API calls.
