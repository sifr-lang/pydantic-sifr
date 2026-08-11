#!/usr/bin/env python3
"""Import approved Core Schema dispositions and bind them to pinned literals."""

from __future__ import annotations

import argparse
import ast
import hashlib
import json
import re
from pathlib import Path

PIN = "f59e929c999e8b2efc7b12fd0bc1685c1a186be3"
SOURCE = Path("pydantic-core/python/pydantic_core/core_schema.py")


def quote(value: str) -> str:
    return json.dumps(value)


def literal_values(tree: ast.Module, name: str) -> list[str]:
    for node in tree.body:
        if not isinstance(node, ast.AnnAssign):
            continue
        if not isinstance(node.target, ast.Name) or node.target.id != name:
            continue
        if not isinstance(node.value, ast.Subscript):
            break
        value = node.value.slice
        items = value.elts if isinstance(value, ast.Tuple) else [value]
        return [str(ast.literal_eval(item)) for item in items]
    raise SystemExit(f"could not find {name}")


def clean(value: str) -> str:
    return value.replace("`", "").strip()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--phase-record", type=Path, required=True)
    parser.add_argument("--upstream", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    source = (args.upstream / SOURCE).read_bytes()
    tree = ast.parse(source.decode("utf-8"))
    schema = literal_values(tree, "CoreSchemaType")
    field = literal_values(tree, "CoreSchemaFieldType")
    universe = {("schema", item) for item in schema} | {
        ("field", item) for item in field
    }
    rows: list[dict[str, object]] = []
    family = ""
    for line in args.phase_record.read_text(encoding="utf-8").splitlines():
        if line.startswith("| Pydantic Core kind |"):
            family = "schema"
            continue
        if line.startswith("| Pydantic Core field kind |"):
            family = "field"
            continue
        if family and not line.startswith("|"):
            family = ""
            continue
        if not family or line.startswith("| ---"):
            continue
        cells = [item.strip() for item in line.split("|")[1:-1]]
        if len(cells) != 4:
            continue
        name = clean(cells[0])
        compatibility_class = clean(cells[1])
        normal_form = clean(cells[2])
        owner_cell = cells[3]
        milestone_match = re.search(r"ps_[0-9]+", owner_cell)
        if milestone_match is None:
            raise SystemExit(f"missing owner milestone for {family}:{name}")
        milestone = milestone_match.group(0)
        tokens = re.findall(r"`([^`]+)`", owner_cell)
        evidence = [token for token in tokens if token != milestone]
        if not evidence:
            evidence = ["ps_0/disposition_audit"]
        rows.append(
            {
                "family": family,
                "name": name,
                "class": compatibility_class,
                "normal_form": normal_form,
                "owner_milestone": milestone,
                "evidence": evidence,
            }
        )
    identities = {(str(row["family"]), str(row["name"])) for row in rows}
    if identities != universe:
        raise SystemExit(
            f"phase disposition differs from pinned universe: "
            f"missing={sorted(universe - identities)} extra={sorted(identities - universe)}"
        )
    universe_sha256 = hashlib.sha256(
        b"\n".join(f"{left}\0{right}".encode() for left, right in sorted(universe))
    ).hexdigest()
    lines = [
        "# Generated from the pinned Core Schema literals and approved dispositions.",
        "# Do not edit by hand.",
        "",
        "[meta]",
        "schema_version = 1",
        f"pydantic_commit = {quote(PIN)}",
        f"source_path = {quote(SOURCE.as_posix())}",
        f"source_sha256 = {quote(hashlib.sha256(source).hexdigest())}",
        f"universe_sha256 = {quote(universe_sha256)}",
        f"schema_kind_count = {len(schema)}",
        f"field_kind_count = {len(field)}",
        "",
    ]
    for row in sorted(rows, key=lambda item: (str(item["family"]), str(item["name"]))):
        lines.append("[[kind]]")
        for key in ("family", "name", "class", "normal_form", "owner_milestone"):
            lines.append(f"{key} = {quote(str(row[key]))}")
        evidence = ", ".join(quote(str(item)) for item in row["evidence"])
        lines.append(f"evidence = [{evidence}]")
        lines.append("")
    args.output.write_text("\n".join(lines), encoding="utf-8")
    print(
        f"wrote {args.output}: schema={len(schema)} fields={len(field)} "
        f"universe_sha256={universe_sha256}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

