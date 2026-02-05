<p align="center">
  <img src="assets/logo.png" alt="confcli" width="400">
</p>

<h1 align="center">confcli</h1>

<p align="center">A scrappy little Confluence CLI for you and your clanker</p>

## Installation

### Binary (recommended)

Download the latest release for your platform from
[GitHub Releases](https://github.com/hochej/confcli/releases).

```bash
# macOS / Linux
chmod +x confcli
mv confcli ~/.local/bin/
```

> Make sure `~/.local/bin` is in your `PATH`.

### Cargo

```bash
cargo install confcli
```

### Shell Completions

Bash:
```bash
confcli completions bash > /usr/local/etc/bash_completion.d/confcli
```

Zsh:
```bash
confcli completions zsh > ~/.zsh/completions/_confcli
```

Fish:
```bash
confcli completions fish > ~/.config/fish/completions/confcli.fish
```

## Quick Start

```bash
# Login (interactive prompts for domain/email/token)
confcli auth login

# Or provide credentials directly
confcli auth login --domain yourcompany.atlassian.net --email you@example.com --token <api-token>

# Verify authentication
confcli auth status

# List spaces
confcli space list
```

> **Tip:** Generate an API token at
> https://id.atlassian.com/manage-profile/security/api-tokens

## Commands

### Spaces

```bash
confcli space list                     # List all spaces
confcli space list --all               # Fetch all pages of results
confcli space get MFS                  # Get space by key
confcli space pages MFS                # List pages in space
confcli space pages MFS --tree         # Show page hierarchy
```

### Pages

```bash
confcli page list --space MFS          # List pages in space
confcli page get 12345                 # Get page by ID
confcli page get MFS:Overview          # Get page by Space:Title
confcli page body MFS:Overview         # Get page body as markdown
confcli page body MFS:Overview --format storage   # Raw storage format
confcli page history MFS:Overview      # Show version history
confcli page open MFS:Overview         # Open page in browser

# Create page
confcli page create --space MFS --title "New Page" --body "<p>Hello</p>"
echo "<p>Hello</p>" | confcli page create --space MFS --title "New Page" --body-file -

# Update page
confcli page update MFS:Overview --body-file content.html

# Delete page
confcli page delete 12345
confcli page delete 12345 --purge      # Permanent deletion
```

### Search

```bash
confcli search "meeting notes"         # Text search
confcli search "type=page AND title ~ Template"   # CQL query
confcli search "confluence" --space MFS           # Search within space
```

### Attachments

```bash
confcli attachment list --page MFS:Overview
confcli attachment download att12345 --dest file.png
confcli attachment upload MFS:Overview ./image.png
confcli attachment delete att12345
```

### Labels

```bash
confcli label list
confcli label add MFS:Overview "important"
confcli label remove MFS:Overview "important"
confcli label pages "important"        # Find pages with label
```

### Comments

```bash
confcli comment list MFS:Overview
confcli comment add MFS:Overview --body "LGTM"
confcli comment delete 123456
```

### Export

```bash
confcli export MFS:Overview --dest ./exports --format md
confcli export MFS:Overview --dest ./exports --format storage --skip-attachments
confcli export MFS:Overview --dest ./exports --pattern "*.png"
```

### Copy Tree

```bash
confcli copy-tree MFS:Overview MFS:TargetParent
confcli copy-tree MFS:Overview MFS:TargetParent "Overview (Backup)" --exclude "*draft*"
confcli --dry-run copy-tree MFS:Overview MFS:TargetParent
```

### Edit

```bash
confcli page edit MFS:Overview
confcli page edit MFS:Overview --format adf --diff
```

### Output Formats

All commands support `-o` for output format:

```bash
confcli space list -o json             # JSON output
confcli space list -o table            # Table output (default)
confcli page get MFS:Overview -o md    # Markdown output
```

### Dry Run

Use `--dry-run` to preview destructive operations without executing them:

```bash
confcli --dry-run page create --space MFS --title "Test" --body "<p>Hello</p>"
confcli --dry-run page delete 12345
confcli --dry-run label add MFS:Overview "important"
```

## Security

Credentials are stored as plaintext JSON in your OS config directory:

- Linux: `~/.config/confcli/config.json` (or `$XDG_CONFIG_HOME/confcli/config.json`)
- macOS: `~/Library/Application Support/confcli/config.json`
- Windows: `%APPDATA%\\confcli\\config.json`

`confcli auth status` prints the resolved config path when using file-based auth.
On Unix systems the file is created with `0600` permissions (owner read/write only).

For CI/CD or shared environments, use environment variables instead of the config file:

```bash
export CONFLUENCE_DOMAIN=yourcompany.atlassian.net
export CONFLUENCE_EMAIL=you@example.com
export CONFLUENCE_TOKEN=<api-token>
# Also accepted (compat with other CLIs):
export CONFLUENCE_API_TOKEN=<api-token>
# Override API path if your instance is weird/proxied or you're on Server/DC:
export CONFLUENCE_API_PATH=/wiki/rest/api   # or /rest/api
# Or for OAuth:
export CONFLUENCE_BEARER_TOKEN=<bearer-token>
```
