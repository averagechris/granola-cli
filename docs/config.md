# Non-secret Config and Profiles

The CLI can store ergonomic defaults in the OS config directory. This config never stores API keys.

```bash
granola config set output json
granola config set profile.agent.output json-compact
granola --profile agent notes list --since 7d
granola config show --output json-compact
granola config path
```

Supported keys:

- `output`: `table`, `list`, `json`, `json-compact`, or `json-pretty`
- `quiet`: `true` or `false`
- `profile.NAME.output`, `profile.NAME.quiet`

Command-line flags override configured defaults.
