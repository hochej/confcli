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
sudo mv confcli /usr/local/bin/
```

### Cargo

```bash
cargo install confcli
```

### Shell Completions

```bash
# Bash
confcli completions bash > /usr/local/etc/bash_completion.d/confcli

# Zsh
confcli completions zsh > ~/.zsh/completions/_confcli

# Fish
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
confcli attachment download att12345 -o file.png
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

### Output Formats

All commands support `-o` for output format:

```bash
confcli space list -o json             # JSON output
confcli space list -o table            # Table output (default)
confcli page get MFS:Overview -o md    # Markdown output
```
