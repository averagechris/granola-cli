# Non-secret Config and Profiles

The CLI can store ergonomic defaults in the OS config directory. This config never stores API keys.

```bash
granola config set output json
granola config set compact true
granola config set profile.agent.output json
granola config set profile.agent.compact true
granola --profile agent notes list --since 7d
granola config show --output json --compact
granola config path
```

Supported keys:

- `output`: `table` or `json`
- `compact`: `true` or `false`
- `quiet`: `true` or `false`
- `profile.NAME.output`, `profile.NAME.compact`, `profile.NAME.quiet`

Command-line flags override configured defaults.
