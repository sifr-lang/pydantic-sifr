#!/usr/bin/env python3
"""Require one exact Sifr revision in CI, manifests, locks, and records."""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PIN_SOURCES = (
    ".github/workflows/ci.yml",
    "Cargo.toml",
    "backend/pydantic_sifr_core/Cargo.toml",
    "Cargo.lock",
    "fuzz/Cargo.lock",
    "demos/milestone_ps_6_demo/Cargo.lock",
    "demos/milestone_ps_7_demo/Cargo.lock",
    "README.md",
    "docs/architecture.md",
    "THIRD_PARTY_LICENSES.md",
)
COMMIT = re.compile(r"\b[0-9a-f]{40}\b")
SIFR_REV = re.compile(r"^\s*SIFR_REV:\s*([0-9a-f]{40})\s*$", re.MULTILINE)


def main() -> int:
    workflow = (ROOT / PIN_SOURCES[0]).read_text(encoding="utf-8")
    match = SIFR_REV.search(workflow)
    if match is None:
        raise SystemExit("CI must declare one exact SIFR_REV")
    expected = match.group(1)
    failures: list[str] = []
    for relative in PIN_SOURCES[1:]:
        text = (ROOT / relative).read_text(encoding="utf-8")
        sifr_lines = "\n".join(
            line for line in text.splitlines() if "sifr" in line.lower() or expected in line
        )
        revisions = set(COMMIT.findall(sifr_lines))
        if expected not in revisions:
            failures.append(f"{relative}: missing {expected}")
        unexpected = revisions - {expected}
        if unexpected:
            failures.append(f"{relative}: stale Sifr revisions {sorted(unexpected)}")
    if failures:
        raise SystemExit("Sifr pin mismatch:\n" + "\n".join(failures))
    print(f"Sifr pin check passed: {expected}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
