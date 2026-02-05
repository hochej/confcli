# Changelog

## [0.2.3] - 2026-02-05

### Fixed

- **Empty search query** no longer causes a server 500 — now validated client-side with a clear error message
- **`comment list`** now includes reply (child) comments, not just top-level comments
- **`page get --body-format`** now shows the body content in table output (previously only visible with `-o json`)

### Added

- **`space delete`** command — deletes a space via v1 API with confirmation prompt and `--dry-run` support
- **`label add` accepts multiple labels** — e.g. `confcli label add PAGE tag1 tag2 tag3`
- **`label remove` accepts multiple labels** — e.g. `confcli label remove PAGE tag1 tag2`
- **`attachment upload` accepts multiple files** — e.g. `confcli attachment upload PAGE a.txt b.png`
- Integration tests for empty/whitespace search, multi-label, multi-file upload, and space delete

## [0.2.2] - 2026-02-05

### Fixed

- **Attachment downloads broken on Confluence Cloud** — download URLs were missing the `/wiki` path prefix, causing 404s in both `export` and `attachment download` commands
- **`attachment download`** now uses the shared `attachment_download_url` helper instead of duplicated inline logic
- **Empty page/space references** (`page get ""`) no longer return empty results; they now error with a helpful message

### Added

- Unit tests for attachment download URL construction (with/without `/wiki` prefix, absolute URLs)

## [0.2.1] - 2026-02-05

### Fixed

- **`space create --description`** now works — switched to v1 API since v2 silently ignores the description field
- **`json_str` helper** now handles numeric and boolean JSON values (fixes empty ID display with v1 API responses)

## [0.2.0] - 2026-02-05

### Added

- **Comments**: `comment list`, `comment add`, `comment delete` commands
- **Export**: `export` command to save a page and its attachments to a folder
- **Copy Tree**: `copy-tree` command to duplicate a page hierarchy
- **Page Edit**: `page edit` opens page body in `$EDITOR` with version conflict detection
- **Page Open**: `page open` launches a page in the browser
- **Page History**: `page history` shows version history
- **Retry logic**: Automatic retries with `Retry-After` header support on API requests
- **Bearer auth**: OAuth bearer token support (`--bearer` / `CONFLUENCE_BEARER_TOKEN`)
- **API path overrides**: `--api-path` and `--api-v2-path` for Server/DC and proxied instances
- **Progress bars**: Download and bulk operation progress indicators
- **`page get` table output** now shows Version and URL fields
- **Search `--all`**: Paginate through all v1 search results
- **`md` alias**: `--output md` works everywhere as shorthand for `--output markdown`
- **Markdown improvements**: Panel-to-blockquote conversion, image alt text, table fixes

### Changed

- **`page get` defaults to table output** instead of markdown (use `-o md` or `page body` for content)
- **`attachment download --dest`** replaces `-o/--output` to avoid collision with output format flag
- **Help text cleaned up**: Removed internal API version details (v1/v2) from all command descriptions
- **Command descriptions**: Replaced vague "Work with X" with specific summaries (e.g. "List, view, create, and manage pages")
- **`--limit` help text**: Changed from "Page size for pagination" to "Maximum number of results"
- **`comment list`**: Added `-a`/`-n` short flags, removed `--start` offset parameter
- **Config**: Migrated from single `base_url` to separate `site_url`, `api_base_v1`, `api_base_v2` (old configs auto-migrate)
- **About text**: "A scrappy little Confluence CLI for you and your clanker"

### Fixed

- Space keys starting with `~` (personal spaces) now display the space name instead

## [0.1.0] - 2025-01-01

Initial release.

- `auth login`, `auth status`, `auth logout`
- `space list`, `space get`, `space pages` (with `--tree`)
- `page list`, `page get`, `page body`, `page create`, `page update`, `page delete`, `page children`
- `search` with automatic CQL detection
- `attachment list`, `attachment get`, `attachment download`, `attachment upload`, `attachment delete`
- `label list`, `label add`, `label remove`, `label pages`
- Shell completions (bash, zsh, fish, powershell)
- Global `--quiet`, `--verbose`, `--dry-run` flags
- `SPACE:Title` page resolution
