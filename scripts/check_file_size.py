#!/usr/bin/env python3
"""Reject oversized hand-maintained first-party source files."""

from __future__ import annotations

import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LIMIT = 900
EXCLUDED_SUFFIXES = {".md", ".mdx", ".lock"}
EXCLUDED_PREFIXES = ("target/", "tests/provenance/")


def main() -> int:
    result = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    failures: list[str] = []
    for relative in result.stdout.splitlines():
        if relative.startswith(EXCLUDED_PREFIXES):
            continue
        path = ROOT / relative
        if path.suffix in EXCLUDED_SUFFIXES or not path.is_file():
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        lines = text.count("\n") + (0 if not text or text.endswith("\n") else 1)
        if lines > LIMIT:
            failures.append(f"{relative}: {lines} lines (limit {LIMIT})")
    if failures:
        raise SystemExit("oversized hand-maintained files:\n" + "\n".join(failures))
    print(f"file-size guard passed: limit={LIMIT}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
