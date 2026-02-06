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

### 1. `--quiet` semantics are inconsistent and documentation disagrees with the CLI
- **File:** `src/cli.rs:L36` vs `AGENTS.md:L46` and various commands
- **Fix:** Decide contract and enforce via helpers.

### 2. Missing error handling: page body extraction silently returns empty string
- **File:** `src/download.rs:L36-L37`
- **Fix:** Treat missing body as an error with context.

### 3. Config file writes are not atomic
- **File:** `src/config.rs:L99-L106`
- **Fix:** Write temp + fsync + rename.

### 4. Env-based config silently does nothing when partially provided
- **File:** `src/config.rs:L48-L87`
- **Fix:** If base URL present, validate required env vars and return a clear error.

### 5. Config permissions are only enforced on Unix, but docs claim 0600 generally
- **File:** `src/config.rs:L101-L107`
- **Fix:** Update docs or harden Windows too.

### 6. URL parsing in `extract_page_id_from_url()` allocates unnecessarily
- **File:** `src/resolve.rs:L145`
- **Fix:** Iterate segments without allocating.

### 7. Installer and release pipeline have no integrity verification
- **File:** `install.sh`, `.github/workflows/release.yml`
- **Fix:** Publish/verify SHA256; safer tar extraction.

### 8. Tests are mostly help-text checks
- **File:** `tests/cli.rs`
- **Fix:** Add unit tests for URL encoding, pagination loop safety, retries, etc.

### 9. Deprecated API used in tests
- **File:** `tests/cli.rs:L5-L7`
- **Fix:** Update to current `assert_cmd` idioms.

---

## ⚪ Configuration & Infrastructure

### 1. CI does not run any dependency/security scanning
- **Fix:** Add `cargo audit` or `cargo deny`.

### 2. CI only runs on Ubuntu
- **Fix:** Add Windows/macOS to the matrix.

### 3. Release artifacts are packaged without checksums
- **Fix:** Generate and upload `sha256sum` files.

### 4. Dependency bloat: `http` crate exists just for `HeaderMap`
- **Fix:** Use `reqwest::header` and drop `http`.

### 5. Installer has no strategy for GitHub API rate limiting
- **Fix:** Support `GITHUB_TOKEN` and detect rate-limit errors.

### 6. No toolchain pinning for reproducible builds
- **Fix:** Add `rust-toolchain.toml`.
