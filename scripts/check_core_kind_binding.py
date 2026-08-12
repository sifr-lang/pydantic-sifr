#!/usr/bin/env python3
"""Prove Rust schema kinds and unavailable dispositions match the ledger."""

from __future__ import annotations

import re
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LEDGER = ROOT / "tests/provenance/core_schema_kinds.toml"
KIND_SOURCE = ROOT / "backend/pydantic_sifr_core/src/schema/kind.rs"
ACCEPTED_CLASSES = {"same", "adapted", "not-applicable", "rejected"}


def main() -> int:
    entries = tomllib.loads(LEDGER.read_text(encoding="utf-8"))["kind"]
    expected = {entry["name"] for entry in entries}
    classes = {entry["class"] for entry in entries}
    if not classes <= ACCEPTED_CLASSES:
        raise SystemExit(f"unknown schema-kind classes: {sorted(classes - ACCEPTED_CLASSES)}")
    for entry in entries:
        if not entry["owner_milestone"] or not entry["evidence"]:
            raise SystemExit(f"schema kind has incomplete ownership evidence: {entry['name']}")

    source = KIND_SOURCE.read_text(encoding="utf-8")
    pairs = re.findall(
        r'#\[serde\(rename = "([^"]+)"\)\]\s+([A-Za-z0-9_]+),', source
    )
    variant_to_name = {variant: name for name, variant in pairs}
    actual = set(variant_to_name.values())
    if actual != expected or len(pairs) != len(entries):
        raise SystemExit(
            "schema-kind binding mismatch: "
            f"missing={sorted(expected - actual)}, extra={sorted(actual - expected)}"
        )

    unavailable_block = re.search(
        r"pub const UNAVAILABLE: \[Self; \d+\] = \[(.*?)\];", source, re.DOTALL
    )
    if unavailable_block is None:
        raise SystemExit("schema-kind binding has no UNAVAILABLE disposition set")
    unavailable_variants = re.findall(r"Self::([A-Za-z0-9_]+)", unavailable_block.group(1))
    try:
        actual_unavailable = {variant_to_name[variant] for variant in unavailable_variants}
    except KeyError as error:
        raise SystemExit(f"unknown unavailable SchemaKind variant: {error.args[0]}") from None
    expected_unavailable = {
        entry["name"]
        for entry in entries
        if entry["class"] in {"not-applicable", "rejected"}
    }
    if actual_unavailable != expected_unavailable:
        raise SystemExit(
            "schema-kind disposition mismatch: "
            f"missing={sorted(expected_unavailable - actual_unavailable)}, "
            f"extra={sorted(actual_unavailable - expected_unavailable)}"
        )
    print(
        "schema-kind binding passed: "
        f"{len(actual)} exact kinds, {len(actual_unavailable)} unavailable dispositions"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
