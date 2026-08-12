#!/usr/bin/env python3
"""Prove the Rust schema-kind enum matches the generated upstream ledger."""

from __future__ import annotations

import re
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LEDGER = ROOT / "tests/provenance/core_schema_kinds.toml"
KIND_SOURCE = ROOT / "backend/pydantic_sifr_core/src/schema/kind.rs"


def main() -> int:
    ledger = tomllib.loads(LEDGER.read_text(encoding="utf-8"))
    expected = {entry["name"] for entry in ledger["kind"]}
    source = KIND_SOURCE.read_text(encoding="utf-8")
    actual = set(re.findall(r'#\[serde\(rename = "([^"]+)"\)\]', source))
    if actual != expected:
        missing = sorted(expected - actual)
        extra = sorted(actual - expected)
        raise SystemExit(f"schema-kind binding mismatch: missing={missing}, extra={extra}")
    if len(actual) != len(ledger["kind"]):
        raise SystemExit("schema-kind binding contains duplicate serde names")
    print(f"schema-kind binding passed: {len(actual)} exact kinds")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
