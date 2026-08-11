#!/usr/bin/env python3
"""Import approved portable anchor tables from the Sifr phase record."""

from __future__ import annotations

import argparse
import ast
import json
import re
from pathlib import Path


def quote(value: str) -> str:
    return json.dumps(value)


def split_cell(value: str) -> list[str]:
    return [item.strip().strip("`") for item in value.split(",")]


def expand_paths(value: str) -> list[str]:
    paths: list[str] = []
    directory = ""
    for item in split_cell(value):
        if "/" in item:
            path = item
            directory = str(Path(item).parent)
        elif directory:
            path = f"{directory}/{item}"
        else:
            path = item
        paths.append(path)
    return paths


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--phase-record", type=Path, required=True)
    parser.add_argument("--upstream", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    text = args.phase_record.read_text(encoding="utf-8")
    namespace = ""
    anchors: list[dict[str, str]] = []
    row_pattern = re.compile(
        r"^\| `(?P<milestone>ps_[0-9]+)` \| (?P<paths>.+?) \| "
        r"(?P<selectors>.+?) \| `(?P<fixture>[^`]+)` \|$"
    )
    for line in text.splitlines():
        if line == "#### Pydantic Core engine baseline":
            namespace = "core"
            continue
        if line == "#### Pydantic Sifr-API baseline":
            namespace = "api"
            continue
        match = row_pattern.match(line)
        if not match or not namespace:
            continue
        paths = expand_paths(match.group("paths"))
        selectors = split_cell(match.group("selectors"))
        if len(paths) == 1:
            pairs = [(paths[0], selector) for selector in selectors]
        else:
            pairs = []
            for selector in selectors:
                matches: list[str] = []
                for path in paths:
                    source_path = args.upstream / (
                        f"pydantic-core/{path}" if namespace == "core" else path
                    )
                    tree = ast.parse(source_path.read_text(encoding="utf-8"))
                    names = {
                        node.name
                        for node in ast.walk(tree)
                        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
                    }
                    if selector in names:
                        matches.append(path)
                if len(matches) != 1:
                    raise SystemExit(
                        f"anchor selector must resolve once in its row: "
                        f"{namespace} {selector} matches={matches}"
                    )
                pairs.append((matches[0], selector))
        for path, selector in pairs:
            manifest_path = f"pydantic-core/{path}" if namespace == "core" else path
            anchors.append(
                {
                    "namespace": namespace,
                    "path": manifest_path,
                    "selector": selector,
                    "class": "adapted",
                    "milestone": match.group("milestone"),
                    "fixture": match.group("fixture"),
                }
            )
    lines = [
        "# Imported from the approved Sifr native-Pydantic phase record.",
        "# Paths and selectors are resolved against the immutable upstream pin.",
        "",
    ]
    for anchor in anchors:
        lines.append("[[anchor]]")
        for key in ("namespace", "path", "selector", "class", "milestone", "fixture"):
            lines.append(f"{key} = {quote(anchor[key])}")
        lines.append("")
    args.output.write_text("\n".join(lines), encoding="utf-8")
    print(f"wrote {args.output}: candidate anchors={len(anchors)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
