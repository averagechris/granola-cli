# Authentication

Granola's documented public API uses bearer API keys. Browser OAuth is not implemented because Granola does not currently document a CLI-compatible public OAuth flow.

## Store an API key

Interactive prompt:

```bash
granola auth login --validate
```

Automation-safe stdin:

```bash
granola auth login --key-stdin --validate < ./scratch/token
```

`--key-stdin` is preferred over `--key` for automation because it avoids exposing the secret in argv/process listings and shell history.

## Check status

```bash
granola auth status --validate --output json --compact
```

The CLI reports whether auth is configured without printing secrets.

## Remove credentials

```bash
granola auth logout
granola auth logout --force
```

Credentials are stored in the OS keyring only. Plaintext credential config is intentionally unsupported.
