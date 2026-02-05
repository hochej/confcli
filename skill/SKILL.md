---
name: confcli
description:
  Interact with Confluence Cloud from the command line. Use when reading,
  creating, updating, or searching Confluence pages, managing attachments,
  labels, comments, or exporting content.
---

# confcli

CLI for Confluence Cloud.

## Prerequisites

Check auth status first:

```bash
confcli auth status
```

If not authenticated, user must run `confcli auth login` interactively or set
environment variables:

- `CONFLUENCE_DOMAIN` - e.g. `yourcompany.atlassian.net`
- `CONFLUENCE_EMAIL`
- `CONFLUENCE_TOKEN` (or `CONFLUENCE_API_TOKEN`)
- `CONFLUENCE_BEARER_TOKEN` - for OAuth
- `CONFLUENCE_API_PATH` - override v1 API path for Server/DC

## Page References

Pages can be referenced by:

- ID: `12345`
- URL: `https://company.atlassian.net/wiki/spaces/MFS/pages/12345/Title`
- Space:Title: `MFS:Overview`

## Important

Write operations (create, update, delete, purge, edit, label add/remove,
attachment upload/delete, comment add/delete, copy-tree) require explicit user
intent. Never perform these based on assumptions.

Use `--dry-run` to preview destructive operations without executing them.

## Common Commands

```bash
# Spaces
confcli space list
confcli space get MFS
confcli space pages MFS --tree

# Pages
confcli page list --space MFS --title "Overview"
confcli page get MFS:Overview                  # metadata (table)
confcli page get MFS:Overview -o json          # full JSON
confcli page body MFS:Overview                 # markdown content
confcli page body MFS:Overview --format storage
confcli page children MFS:Overview
confcli page children MFS:Overview --recursive
confcli page history MFS:Overview
confcli page open MFS:Overview                 # open in browser
confcli page edit MFS:Overview                 # edit in $EDITOR

# Search
confcli search "query"
confcli search "type=page AND title ~ Template"
confcli search "confluence" --space MFS

# Write
confcli page create --space MFS --title "Title" --body "<p>content</p>"
confcli page update MFS:Overview --body-file content.html
confcli page delete 12345

# Attachments
confcli attachment list --page MFS:Overview
confcli attachment upload MFS:Overview ./file.png
confcli attachment download att12345 --dest file.png

# Labels
confcli label add MFS:Overview "tag"
confcli label remove MFS:Overview "tag"
confcli label pages "tag"

# Comments
confcli comment list MFS:Overview
confcli comment add MFS:Overview --body "LGTM"
confcli comment delete 123456

# Export
confcli export MFS:Overview --dest ./exports --format md

# Copy Tree
confcli copy-tree MFS:Overview MFS:TargetParent
```

## Output Formats

Use `-o` flag: `json`, `table`, `md`

```bash
confcli space list -o json
confcli page get MFS:Overview -o json
```

## Pagination

Add `--all` to fetch all results, `-n` to set limit:

```bash
confcli space list --all
confcli search "query" --all -n 100
```
