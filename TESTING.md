# End-to-End CLI Testing (Playground Confluence Org)

Date: 2026-02-06

confcli version: `0.2.3`

> Notes on privacy: this report redacts the specific Atlassian domain and any user identifiers returned by the API.

## 1) CLI surface area (discovered)

Top-level commands:

- `auth` (`login`, `status`, `logout`)
- `space` (`list`, `get`, `pages`, `create`, `delete`)
- `page` (`list`, `get`, `body`, `edit`, `create`, `update`, `delete`, `children`, `history`, `open`)
- `search` (query-only command)
- `attachment` (`list`, `get`, `download`, `upload`, `delete`)
- `label` (`list`, `add`, `remove`, `pages`)
- `comment` (`list`, `add`, `delete`)
- `export`
- `copy-tree`
- `completions`

Global flags:

- `-q/--quiet`
- `-v/--verbose` (repeatable)
- `--dry-run`
- `-o/--output` on most read/list commands (`table` default; supports `json` and usually `markdown`)

## 2) What was tested (systematically)

### Authentication

- `confcli auth status`
  - Verified the CLI can run purely from environment variables.

### Space workflows

- Create space (real + `--dry-run`)
- List spaces with pagination (`--all`, `-n 1`)
- List pages within a space and render `--tree`
- Delete space (`--dry-run` + real)

### Page workflows (CRUD + navigation)

Tested against a dedicated temporary space created for this run.

- Create pages
  - Unicode titles (e.g. en dash + Greek + CJK)
  - Titles containing `:` (colon)
  - Bodies via `--body` and via `--body-file -` (stdin)
  - Very long bodies (multi-kilobyte)
- Read pages
  - Resolve by `id`, by URL, and by `SPACE:Title`
  - `page body` in `markdown` and `storage`
- Update pages
  - Update title + body + version message
  - Edge case: update with no fields (see UX issue below)
- Page hierarchy
  - `page children` direct children
  - `page children --recursive` descendants
- History
  - `page history` and verifying version increments
- Open
  - `page open --dry-run` (non-destructive)
- Delete / purge
  - `page delete` (trash)
  - `page delete --purge --force`

### Search

- Plain text searches + `--space` scoping
- Raw CQL queries
- Output formats: `table` and `markdown`

### Attachments

- Upload multiple files in one invocation
  - Unicode filename upload
  - Random binary file
- List attachments on a page
- Get attachment metadata (`attachment get`)
- Download attachment
  - Default destination uses the attachment title in the current directory
  - Explicit `--dest` path
- Delete attachment (`--dry-run` + real)

### Labels

- Add labels, including unicode in label names
- List labels on a page
- List pages by label
- Remove label

### Comments

- Add a footer comment using `--body-format markdown`
- List comments
- Delete comment

### Export

- Export page + attachments to a destination directory
- `--pattern` filtering to export only matching attachments

### Copy-tree

- `copy-tree --dry-run` (with `--exclude`, `--max-depth`, `--delay-ms`)
- Real copy of a page tree including grandchildren
- Verified copied children titles get suffix ` (Copy)`

### Output + exit codes

- `--quiet` produced no output and still returned exit code 0.
- Missing required args caused Clap usage errors (exit code 2).
- Invalid IDs generally returned a helpful error message and exit code 1.

## 3) Bugs found (with reproduction steps)

### BUG-1: `completions` panics on broken pipe when stdout closes early

**Repro**:

```bash
confcli completions bash | head
```

**Observed**: process panics with a Rust backtrace indicating “Broken pipe”.

**Expected**: graceful exit (typically exit 0) when stdout is closed by the downstream command (common UNIX behavior).

**Impact**: makes it annoying to preview completions output or pipe it through tools.

---

### BUG-2 (docs/skill mismatch): `attachment list --page` example does not match actual CLI

The skill doc contains:

```bash
confcli attachment list --page MFS:Overview
```

But the CLI uses an optional positional argument:

```bash
confcli attachment list [PAGE]
```

**Impact**: users following the built-in agent skill docs will hit an “unexpected argument” error.

(Strictly speaking this is a documentation bug, but it will be experienced as a CLI UX bug.)

## 4) UX issues / rough edges

### UX-1: `page update` with no changes is treated as a “success” without explaining it was a no-op

**Repro**:

```bash
confcli page update <page-id>
```

**Observed**: prints a normal-looking success table, but does not change the version.

**Expected**: either (a) a validation error like “nothing to update”, or (b) explicit messaging like “No fields provided; nothing changed.”

---

### UX-2: Some errors surface raw API payloads (inconsistent friendliness)

Example: uploading to a non-existent page returns a large error blob including JSON.

**Expected**: a concise top-line message (e.g. “Page not found”) plus `-v/-vv` to show full HTTP payload.

---

### UX-3: Authentication failures can appear as generic 404s

With an invalid `CONFLUENCE_TOKEN`, some requests returned:

- `404 Not Found: Not Found`

This is likely an Atlassian behavior for unauthorized access in some cases, but it’s still confusing.

**Suggestion**: detect common auth-failure patterns and add a hint like “This may be an auth/permission issue; run `confcli auth status`.”

---

### UX-4: `copy-tree --dry-run` output includes confusing placeholder parent IDs

Dry-run log lines included strings like `under dry-run:<id>`.

**Suggestion**: print something like `under (dry-run) <source-id>` or `under <target-parent>`.

---

### UX-5: Help text minor inconsistencies

Example: some `--help` strings mention only `json/table` while Clap lists `markdown` as a possible value.

## 5) What works well (solid flows)

- Page addressing is robust:
  - Works by numeric ID, full URL, and `SPACE:Title`.
  - Handles unicode in titles and titles containing `:`.
- Markdown rendering for `page body` works well for simple storage bodies.
- `--dry-run` is widely supported and reliable for destructive commands.
- Pagination:
  - `--all` works correctly even with very small `-n` values.
- Attachments:
  - Upload/download/list/delete all worked.
  - Unicode filenames round-tripped correctly on upload + download + export.
- Export:
  - Produces a predictable folder structure with `page.md`, `meta.json`, and `attachments/`.
  - Attachment filtering via `--pattern` behaves as expected.
- Copy-tree:
  - Copies multi-level page hierarchies successfully.
  - Default suffixing prevents title collisions.

## 6) Missing features vs Confluence REST API (gap analysis)

Confluence’s REST API surface is very large; confcli currently covers a practical subset (spaces/pages/search/attachments/labels/comments/export/copy).

Notable gaps that users commonly want (grouped by area):

### Space management

- Update space details (name/description) after creation
- Space permissions / roles administration
- Space settings (look and feel, theme, homepage management)
- Space properties

### Content / pages

- Blog posts (create/list/update)
- Page restrictions (read/update restrictions) and permission auditing
- Watching / unwatching content, notifications
- Likes / reactions
- Move pages (reorder / change parent) as a first-class “move” command (vs only via update parent)
- Trash management (list trashed pages, restore)
- Content properties / page properties reports (commonly used for automation)
- Content states / workflows (where applicable)
- Richer ADF support (beyond fetch/edit raw ADF): conversion helpers, validation, etc.

### Comments

- Edit/update existing comments
- Resolve/unresolve inline comments as first-class operations
- Better inline comment creation ergonomics (currently “best-effort” JSON properties)

### Attachments

- Attachment version history and versioned downloads
- Update attachment metadata/comment
- Move attachments between pages

### Search

- Richer CQL helpers (saved searches, more filter flags)
- Searching across additional content types beyond pages (where API supports it)

### Admin / user tooling

- User and group lookup commands
- Audit log access (where permissions allow)

## 7) Suggested next test iterations

If you want to push further “try to break it” testing:

- Run copy-tree + attachment uploads concurrently (multiple shells) to stress rate limits and retry logic.
- Introduce network turbulence (e.g. via a local proxy that injects latency / 429 / timeouts) to validate retry/backoff behavior.
- Create very large pages and hundreds of attachments to validate progress bars and pagination performance.
