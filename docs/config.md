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
# Saved views

Saved views are non-secret reusable selectors stored in the same TOML config file.
They can either run a local cache search query or an API-backed notes list filter:

```bash
granola views create customer-calls --folder-id fol_123 --since 30d --all
granola views create renewal-search --query 'transcript:renewal' --limit 20
granola views list
granola views run renewal-search --output json-compact
```

Search views require a populated cache. Run `granola sync --since 30d --all --include-transcript` first when using transcript searches.
