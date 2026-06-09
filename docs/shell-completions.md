# Shell Completions

Generate completion scripts directly:

```bash
granola completions zsh > ~/.zfunc/_granola
granola completions bash > ~/.local/share/bash-completion/completions/granola
granola completions fish > ~/.config/fish/completions/granola.fish
```

Print shell-specific install commands:

```bash
granola completions install zsh
granola completions install fish
```

Supported shells are provided by clap: Bash, Elvish, Fish, PowerShell, and Zsh.
