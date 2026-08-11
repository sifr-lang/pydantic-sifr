#!/usr/bin/env python3
"""Verify the exact pinned Core Schema and field-kind disposition universe."""

from __future__ import annotations

import argparse
import ast
import hashlib
import subprocess
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PIN = "f59e929c999e8b2efc7b12fd0bc1685c1a186be3"
OUTPUT = ROOT / "tests/provenance/core_schema_kinds.toml"
SOURCE = Path("pydantic-core/python/pydantic_core/core_schema.py")


def fail(message: str) -> None:
    raise SystemExit(f"core-schema-kinds: {message}")


def literal_values(tree: ast.Module, name: str) -> list[str]:
    for node in tree.body:
        if not isinstance(node, ast.AnnAssign):
            continue
        if not isinstance(node.target, ast.Name) or node.target.id != name:
            continue
        value = node.value
        if not isinstance(value, ast.Subscript):
            break
        items = value.slice.elts if isinstance(value.slice, ast.Tuple) else [value.slice]
        result: list[str] = []
        for item in items:
            if not isinstance(item, ast.Constant) or not isinstance(item.value, str):
                fail(f"{name} contains a non-string literal")
            result.append(item.value)
        return result
    fail(f"could not find {name}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=OUTPUT)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    upstream = args.upstream.resolve()
    commit = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=upstream,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    if commit != PIN:
        fail(f"checkout is {commit}, expected {PIN}")
    source = (upstream / SOURCE).read_bytes()
    tree = ast.parse(source.decode("utf-8"))
    universe = {
        ("schema", value) for value in literal_values(tree, "CoreSchemaType")
    } | {
        ("field", value) for value in literal_values(tree, "CoreSchemaFieldType")
    }
    try:
        payload = tomllib.loads(args.output.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        fail(f"invalid disposition file: {error}")
    meta = payload.get("meta")
    rows = payload.get("kind")
    if not isinstance(meta, dict) or not isinstance(rows, list):
        fail("disposition file must contain [meta] and [[kind]] rows")
    expected_source_sha = hashlib.sha256(source).hexdigest()
    expected_universe_sha = hashlib.sha256(
        b"\n".join(f"{family}\0{kind}".encode() for family, kind in sorted(universe))
    ).hexdigest()
    expected_meta = {
        "schema_version": 1,
        "pydantic_commit": PIN,
        "source_path": SOURCE.as_posix(),
        "source_sha256": expected_source_sha,
        "universe_sha256": expected_universe_sha,
        "schema_kind_count": 53,
        "field_kind_count": 4,
    }
    if meta != expected_meta:
        fail("metadata differs from the pinned source universe")
    identities: list[tuple[str, str]] = []
    required = {
        "family",
        "name",
        "class",
        "normal_form",
        "owner_milestone",
        "evidence",
    }
    for index, row in enumerate(rows):
        if not isinstance(row, dict) or set(row) != required:
            fail(f"row {index} has invalid fields")
        family = row["family"]
        name = row["name"]
        if family not in {"schema", "field"} or not isinstance(name, str):
            fail(f"row {index} has invalid identity")
        if row["class"] not in {"same", "adapted", "rejected", "not-applicable"}:
            fail(f"row {index} has invalid class")
        for key in ("normal_form", "owner_milestone"):
            if not isinstance(row[key], str) or not row[key]:
                fail(f"row {index} has empty {key}")
        if not isinstance(row["evidence"], list) or not row["evidence"]:
            fail(f"row {index} has no evidence or disposition audit")
        identities.append((family, name))
    if len(identities) != len(set(identities)):
        fail("disposition rows contain duplicate identities")
    actual = set(identities)
    if actual != universe:
        missing = sorted(universe - actual)
        extra = sorted(actual - universe)
        fail(f"kind universe differs: missing={missing} extra={extra}")
    print(
        "core schema kind exact-set audit passed: "
        f"schema=53 fields=4 universe_sha256={expected_universe_sha}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

