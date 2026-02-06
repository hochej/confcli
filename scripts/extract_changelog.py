#!/usr/bin/env python3
"""Extract a single version section from CHANGELOG.md.

Usage:
  scripts/extract_changelog.py 0.2.4 > RELEASE_NOTES.md

It extracts the block starting at:
  ## [0.2.4] - YYYY-MM-DD

until the next "## [" heading (or EOF), excluding the heading line itself.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path


def main() -> int:
    if len(sys.argv) != 2:
        print("Usage: extract_changelog.py <version>", file=sys.stderr)
        return 2

    version = sys.argv[1].strip()
    if not version:
        print("Version cannot be empty", file=sys.stderr)
        return 2

    path = Path("CHANGELOG.md")
    text = path.read_text(encoding="utf-8").splitlines()

    header_re = re.compile(rf"^## \[{re.escape(version)}\](?:\s|$)")

    start = None
    for i, line in enumerate(text):
        if header_re.match(line):
            start = i
            break

    if start is None:
        print(f"Could not find CHANGELOG section for version {version}", file=sys.stderr)
        return 1

    end = len(text)
    for j in range(start + 1, len(text)):
        if text[j].startswith("## ["):
            end = j
            break

    # Skip the heading line itself.
    body_lines = text[start + 1 : end]

    # Trim blank lines at start/end.
    while body_lines and body_lines[0].strip() == "":
        body_lines.pop(0)
    while body_lines and body_lines[-1].strip() == "":
        body_lines.pop()

    out = "\n".join(body_lines).rstrip() + "\n"
    sys.stdout.write(out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
