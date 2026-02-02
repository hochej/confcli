---
name: confcli
description:
  Interact with Confluence Cloud from the command line. Use when reading,
  creating, updating, or searching Confluence pages, managing attachments, or
  working with labels.
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
- `CONFLUENCE_TOKEN`

## Page References

Pages can be referenced by:

- ID: `12345`
- URL: `https://company.atlassian.net/wiki/spaces/MFS/pages/12345/Title`
- Space:Title: `MFS:Overview`

## Important

Write operations (create, update, delete, purge, label add/remove, attachment
upload/delete) require explicit user intent. Never perform these based on
assumptions.

## Common Commands

```bash
# Read
confcli space list
confcli space pages MFS --tree
confcli page list --space MFS --title "Overview"
confcli page get MFS:Overview
confcli page body MFS:Overview              # markdown output
confcli page children MFS:Overview          # direct children
confcli page children MFS:Overview --recursive
confcli search "query"

# Write
confcli page create --space MFS --title "Title" --body "<p>content</p>"
confcli page update MFS:Overview --body-file content.html
confcli page delete 12345

# Attachments
confcli attachment list --page MFS:Overview
confcli attachment upload MFS:Overview ./file.png
confcli attachment download att12345 -o file.png

# Labels
confcli label add MFS:Overview "tag"
confcli label remove MFS:Overview "tag"
confcli label pages "tag"
```

## Output Formats

Use `-o` flag: `json`, `table`, `md`

```bash
confcli page get MFS:Overview -o json
```

## Pagination

Add `--all` to fetch all results:

```bash
confcli space list --all
confcli search "query" --all
```
