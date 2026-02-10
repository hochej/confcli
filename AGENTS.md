# AGENTS.md

Project-specific guidance for AI coding agents working on **confcli**.

## Project overview

confcli is a Confluence CLI written in Rust. It wraps the Confluence Cloud REST API (v1 and v2) and provides commands for spaces, pages, search, attachments, labels, comments, export, and page-tree copying.

## Build & test

```bash
cargo fmt --all -- --check                                        # formatting
cargo clippy --all-targets --all-features -- -D warnings          # lints (all features)
cargo clippy --all-targets --no-default-features -- -D warnings   # lints (read-only build)
cargo test --all-features                                         # tests (all features)
cargo test --no-default-features                                  # tests (read-only build)
cargo audit --deny warnings                                       # security audit
```

The pre-commit hook (`.githooks/pre-commit`) runs all of the above automatically. CI (`.github/workflows/ci.yml`) runs the same steps. **All checks must pass before committing.**

### Pre-commit ↔ CI sync rule

The pre-commit hook and CI workflow **must always stay in sync**. If you add, remove, or change a check in one, apply the same change to the other. The hook is the local gate; CI is the remote gate — they must enforce identical invariants.

## Code layout

| Path | Purpose |
|---|---|
| `src/cli/` | Clap argument definitions (all command/arg structs), split by command domain |
| `src/main.rs` | Entry point, dispatches to command handlers |
| `src/commands/` | One module per top-level command (space, page, search, …); modules may be a single file or a directory |
| `src/client.rs` | HTTP client — auth, retries, `v1_url()` / `v2_url()` helpers |
| `src/resolve.rs` | Resolves `SPACE:Title` / space keys / URLs to numeric IDs |
| `src/download.rs` | Attachment download with retries and progress bars |
| `src/markdown.rs` | HTML → Markdown post-processing |
| `src/output.rs` | Table / JSON / KV output formatting (library side) |
| `src/helpers.rs` | Output wrappers that respect `--quiet`, plus misc utilities |
| `src/json_util.rs` | `json_str` helper for extracting fields from `serde_json::Value` |
| `src/config.rs` | Config file loading, env var fallback, migration |
| `src/context.rs` | `AppContext` (quiet, verbose, dry_run) and client construction |
| `tests/cli.rs` | Integration tests (assert_cmd) |

## Key conventions

- **Rust edition 2024**, stable toolchain.
- **Feature flag `write`** (default on) gates all mutating commands. Compile with `--no-default-features` for a read-only binary.
- **`#[cfg(feature = "write")]`** guards write-only arg structs, command variants, and handler functions.
- **Two API versions**: use `client.v1_url()` for legacy endpoints (space create, search, attachments) and `client.v2_url()` for everything else. Know which version the Confluence endpoint requires.
- **`Url::join` footgun**: absolute paths like `/download/...` resolve against the origin, dropping path prefixes like `/wiki`. Always use `attachment_download_url()` from `src/download.rs` for attachment URLs.
- **`json_str`** handles strings, numbers, and booleans — don't assume API responses use consistent types across v1/v2.
- **Error handling**: use `anyhow::Result` and `.context()` everywhere. User-facing errors should be clear and actionable.
- **Output**: all list/get commands support `-o json`, `-o table`, `-o md`. Table is default. `--quiet` suppresses all output.
- **`--dry-run`**: all write commands must check `ctx.dry_run` and print what *would* happen without making API calls.

## Versioning & releases

- Version lives in `Cargo.toml`.
- Update `CHANGELOG.md` with every version bump (follow existing format).
- Tag releases as `v<version>` (e.g. `v0.2.2`) and push the tag to trigger the release workflow.

## Adding a new command

1. Add the arg struct and subcommand variant to `src/cli.rs` (gate with `#[cfg(feature = "write")]` if it mutates).
2. Create or extend a handler file in `src/commands/`.
3. Wire it up in `src/main.rs` dispatch.
4. Add integration tests in `tests/cli.rs`.
5. Update `README.md` command table and `CHANGELOG.md`.

## Documentation hygiene

Before committing and pushing, always check whether your changes require updates to:

1. **`AGENTS.md`** (this file) — if you changed conventions, code layout, added pitfalls, or altered workflows.
2. **`skill/SKILL.md`** — if you added/removed/renamed commands, changed flags, or altered CLI behaviour. This file is used by AI agents to know how to operate confcli.
3. **`README.md`** — if user-facing commands or features changed.
4. **`CHANGELOG.md`** — always, for every version bump.

These files must never drift from the actual code. If you add a new command, it must appear in all four. If you rename a flag, update all references.

## Release verification

A release is **not complete** until all of the following are confirmed:

1. **Watch the CI run** — after pushing the tag, monitor the GitHub Actions workflow and verify it succeeds.
2. **Check the release page** — confirm the release appears on GitHub Releases with the correct tag and changelog.
3. **Download the binary** — download the released binary for the current platform.
4. **Smoke test** — run the downloaded binary (`confcli --version`, `confcli --help`, and at least one live command like `confcli space list`) to verify it works.

Only when all four steps pass is the release considered successful. If any step fails, fix the issue, bump the version, and release again.

## Common pitfalls

- Empty strings pass `str::chars().all(...)` checks (vacuous truth) — always guard with `!s.is_empty()`.
- Confluence v2 space create ignores `description` — use v1 for that.
- Attachment download links from the API are root-relative (`/download/...`) but need the site path prefix (`/wiki/download/...`) on Cloud instances.
