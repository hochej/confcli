# TODO — Repository Audit Follow-ups

Last updated: 2026-02-06

This file tracks **remaining** work items from the repository audit. All previously listed *critical* issues and the initial batch of “bad implementations” have been resolved.

## Remaining

### Config / Infra

1. **Installer: handle GitHub rate limiting**
   - **File:** `install.sh`
   - **Problem:** The installer avoids the GitHub API for version resolution, but still has no explicit strategy for rate limiting / enterprise proxies.
   - **TODO:**
     - Support `GITHUB_TOKEN` when GitHub API calls are introduced (or as a fallback).
     - Detect rate-limit / auth failures and print an actionable error (including how to set `VERSION=…`).

### Tests

2. **Add coverage for pagination loop safety + retry edge cases**
   - **Files:** `src/pagination.rs`, `src/client.rs`, `src/download.rs`
   - **TODO:**
     - Tests that exercise loop detection / max-pages behavior for pagination.
     - Tests around retry backoff behavior (request errors vs response errors) and “give up” conditions.

---

## Notes

- Read-only builds (`--no-default-features`) are now expected to compile and test cleanly.
- CI should continue to validate both `--all-features` and `--no-default-features` configurations.
