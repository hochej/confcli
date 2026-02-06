# Repository Audit — 2026-02-06

## Status
- **Critical issues (10)**: resolved (secrets removed from history, dotenv gated, N+1 removed, URL/CQL injection fixed, retries+atomic download, test env isolation, bounded cache)

## Summary (remaining)
- **Total remaining issues:** 28
- **Critical:** 0 | **Bad Implementation:** 0 | **Inefficiency:** 10 | **Code Quality:** 10 | **Config/Infra:** 8

---

## ✅ Fixed (was “Bad Implementations”)

The 10 issues previously tracked under **🟠 Bad Implementations** have been fixed:

- Factored `send()` / `send_with_json_body()` into a shared `send_impl()` (single retry/error path)
- Request-error retries now use the same backoff logic with jitter as response retries
- `upload_attachment()` now retries by re-opening the file and rebuilding the multipart form per attempt
- `page edit` now uses `tempfile::TempDir` so temp files are cleaned up automatically
- `page edit` now parses `$EDITOR` via `shell-words` (supports `code --wait`) and prints diff in-process
- URL normalization for auth + config is now shared (`normalize_site_url_and_origin()` in `src/config.rs`)
- Glob-to-regex conversion is now shared (`confcli::pattern::glob_to_regex_ci`)
- `copy-tree` exclusion propagation is now O(N) via BFS over a precomputed adjacency map
- Pagination now detects looping `next` links and caps maximum pages
- `install.sh` no longer parses GitHub JSON with `grep|sed` (uses redirect-based resolution)

## ✅ Fixed (follow-up)

Additional high-impact fixes applied after this audit:

- Markdown hot path: lifted all regex compilations into `static LazyLock<Regex>` (`src/markdown.rs`)
- Download temp files: tmp name now includes `time+pid+counter` to avoid collisions (`src/download.rs`)
- Download atomic replace: replaced blocking `dest.exists()` with async `tokio::fs::try_exists()` (`src/download.rs`)
- Export: attachment downloads now use `FuturesUnordered` instead of storing all `JoinHandle`s (`src/commands/export.rs`)
- Copy-tree: concurrent body fetch now uses `FuturesUnordered` (`src/commands/copy_tree.rs`)
- Resolve: tree rendering no longer clones whole JSON blobs and no longer recurses (`src/resolve.rs`)
- Resolve: URL page-id parsing no longer allocates a `Vec` of segments (`src/resolve.rs`)
- Errors: `friendly_error()` now streams whitespace cleanup without intermediate allocations (`src/client.rs`)
- Config: `Config::save()` now writes atomically (temp + fsync + rename) (`src/config.rs`)
- Config: `Config::from_env()` now errors when base URL is set but auth is incomplete (`src/config.rs`)
- CLI: `--quiet` help text now matches actual behaviour (“Suppress all output”) (`src/cli.rs`)
- Release/install: release workflow publishes SHA256, installer verifies checksum and defensively extracts (`.github/workflows/release.yml`, `install.sh`)
- Quiet: `--quiet` now truly suppresses *all* output (stdout + stderr) in command handlers, diff output, completions, and top-level error handling (`src/main.rs`, `src/commands/{page,space}.rs`)
- Dependencies: removed direct dependency on the `http` crate (use `reqwest::header::HeaderMap` instead) (`Cargo.toml`, `src/{client,download,pagination}.rs`)
- Toolchain pinning: added `rust-toolchain.toml` (Rust 1.93.0) for reproducible builds
- CI: added Windows + macOS to the matrix and added a `cargo audit` job (`.github/workflows/ci.yml`)
- Docs: clarified Windows config-file permission semantics in `README.md`
- Tests: removed deprecated `assert_cmd` API usage and added coverage for `--quiet` + URL/query helpers (`tests/cli.rs`, `src/helpers.rs`)

---

## 🟡 Inefficiencies & Performance

### 1. Regexes are compiled on every markdown conversion (hot path)
- **File:** `src/markdown.rs:L50-L69`, `L81-L83`, `L178-L195`
- **Fix:** Lift into `static LazyLock<Regex>` / `OnceLock`.

### 2. Blocking filesystem checks in async code paths
- **File:** `src/download.rs:L197-L224`, etc.
- **Fix:** Use `tokio::fs` or offload to blocking threads.

### 3. Tree rendering clones full JSON blobs and uses recursion
- **File:** `src/resolve.rs:L168-L218`
- **Fix:** Use lightweight structs and iterative traversal.

### 4. Temp download filename can collide under concurrent downloads
- **File:** `src/download.rs:L226-L242`
- **Fix:** Add random suffix / PID / atomic counter.

### 5. Export/download stores every JoinHandle
- **File:** `src/commands/export.rs:L132-L167`
- **Fix:** Use `FuturesUnordered`.

### 6. Copy-tree body fetching stores every JoinHandle
- **File:** `src/commands/copy_tree.rs:L176-L214`
- **Fix:** Use `FuturesUnordered`.

### 7. `friendly_error()` does unnecessary allocations for cleanup
- **File:** `src/client.rs:L374`
- **Fix:** Stream into `String` without `Vec`.

(Other perf items remain as originally noted.)

---

## 🔵 Code Quality & Maintainability

### 1. ✅ Fixed — `--quiet` semantics are consistent
- Enforced `--quiet` suppression across commands, completions, and top-level error handling.

### 2. Missing error handling: page body extraction silently returns empty string
- **File:** `src/download.rs:L36-L37`
- **Fix:** Treat missing body as an error with context.

### 3. Config file writes are not atomic
- **File:** `src/config.rs:L99-L106`
- **Fix:** Write temp + fsync + rename.

### 4. Env-based config silently does nothing when partially provided
- **File:** `src/config.rs:L48-L87`
- **Fix:** If base URL present, validate required env vars and return a clear error.

### 5. ✅ Fixed — Docs clarify config permissions on Windows
- Updated `README.md` to clarify Unix `0600` vs Windows ACL semantics.

### 6. URL parsing in `extract_page_id_from_url()` allocates unnecessarily
- **File:** `src/resolve.rs:L145`
- **Fix:** Iterate segments without allocating.

### 7. Installer and release pipeline have no integrity verification
- **File:** `install.sh`, `.github/workflows/release.yml`
- **Fix:** Publish/verify SHA256; safer tar extraction.

### 8. 🟡 Partially fixed — Tests now cover some non-help behaviour
- Added unit tests for pagination link parsing and URL query encoding, plus an integration test for `--quiet`.
- Remaining: add coverage for pagination loop safety, retries, and more edge cases.

### 9. ✅ Fixed — Tests use current `assert_cmd` idioms
- Updated `tests/cli.rs` to use `assert_cmd::cargo::cargo_bin!`.

---

## ⚪ Configuration & Infrastructure

### 1. ✅ Fixed — CI runs dependency/security scanning
- Added a `cargo audit --deny warnings` job in `.github/workflows/ci.yml`.

### 2. ✅ Fixed — CI runs on multiple OSes
- Added Windows + macOS to the CI matrix in `.github/workflows/ci.yml`.

### 3. ✅ Fixed — Release artifacts have integrity verification
- Release workflow uploads `.sha256` checksums and `install.sh` verifies them.

### 4. ✅ Fixed — Removed `http` dependency
- Switched to `reqwest::header::HeaderMap` and dropped the direct `http` crate dependency.

### 5. Installer has no strategy for GitHub API rate limiting
- **Fix:** Support `GITHUB_TOKEN` and detect rate-limit errors.

### 6. ✅ Fixed — Toolchain pinning for reproducible builds
- Added `rust-toolchain.toml`.
