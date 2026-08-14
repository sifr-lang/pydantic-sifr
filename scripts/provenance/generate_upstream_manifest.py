#!/usr/bin/env python3
"""Generate the total-set Pydantic conformance provenance manifest."""

from __future__ import annotations

import argparse
import hashlib
import itertools
import json
import os
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
PIN = "f59e929c999e8b2efc7b12fd0bc1685c1a186be3"
CORE_VERSION = "2.47.0"
PYTEST_VERSION = "9.1.1"
OUTPUT = ROOT / "tests/provenance/upstream_manifest.toml"
RULES = ROOT / "tests/provenance/anchor_rules.toml"
HISTORICAL_CORE = "383eb95a19433754c0cecf7025b50c26b6d97a36"


def fail(message: str) -> None:
    raise SystemExit(f"upstream-manifest: {message}")


def run(
    args: list[str],
    *,
    cwd: Path,
    env: dict[str, str] | None = None,
    capture: bool = False,
    silent: bool = False,
) -> str:
    result = subprocess.run(
        args,
        cwd=cwd,
        env=env,
        check=False,
        text=True,
        stdout=subprocess.PIPE if capture or silent else None,
        stderr=None,
    )
    if result.returncode != 0:
        if silent and result.stdout:
            sys.stderr.write("\n".join(result.stdout.splitlines()[-100:]) + "\n")
        fail(f"command failed ({result.returncode}): {' '.join(args)}")
    return result.stdout if capture and result.stdout is not None else ""


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def canonical(value: object) -> bytes:
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=True
    ).encode()


def quote(value: str) -> str:
    return json.dumps(value, ensure_ascii=True)


def verify_checkout(upstream: Path) -> None:
    if not (upstream / "uv.lock").is_file():
        fail(f"missing pinned checkout at {upstream}")
    commit = run(["git", "rev-parse", "HEAD"], cwd=upstream, capture=True).strip()
    if commit != PIN:
        fail(f"checkout is {commit}, expected {PIN}")
    dirty = run(
        ["git", "status", "--porcelain", "--untracked-files=no"],
        cwd=upstream,
        capture=True,
    ).strip()
    if dirty:
        fail("pinned checkout has tracked modifications")


def prepare_environment(upstream: Path) -> dict[str, str]:
    env = os.environ.copy()
    audit_environment = str(upstream / ".venv-pydantic-sifr-audit")
    env["UV_PROJECT_ENVIRONMENT"] = audit_environment
    env.setdefault("CARGO_BUILD_JOBS", "6")
    run(
        ["uv", "sync", "--project", ".", "--locked", "--group", "dev"],
        cwd=upstream,
        env=env,
    )
    run(
        [
            "uv",
            "sync",
            "--project",
            ".",
            "--locked",
            "--package",
            "pydantic-core",
            "--group",
            "testing-extra",
            "--inexact",
        ],
        cwd=upstream,
        env=env,
    )
    versions = run(
        [
            "uv",
            "run",
            "--project",
            ".",
            "--locked",
            "--no-sync",
            "python",
            "-c",
            (
                "import pydantic_core,pytest; "
                "print(pytest.__version__,pydantic_core.__version__)"
            ),
        ],
        cwd=upstream,
        env=env,
        capture=True,
    ).strip()
    if versions != f"{PYTEST_VERSION} {CORE_VERSION}":
        fail(f"locked environment version drift: {versions}")
    return env


def collect_nodes(upstream: Path, env: dict[str, str]) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    with tempfile.TemporaryDirectory(prefix="pydantic-sifr-collection-") as temporary:
        directory = Path(temporary)
        commands = (
            (
                "api",
                ["tests", "--ignore=tests/pydantic_core"],
            ),
            ("core", ["pydantic-core/tests"]),
        )
        for namespace, paths in commands:
            output = directory / f"{namespace}.json"
            collection_env = env.copy()
            collection_env["PYTHONPATH"] = (
                f"{ROOT}{os.pathsep}{collection_env.get('PYTHONPATH', '')}"
            )
            collection_env["PYDANTIC_SIFR_COLLECTION_OUT"] = str(output)
            collection_env["PYDANTIC_SIFR_COLLECTION_NAMESPACE"] = namespace
            collection_env["PYDANTIC_SIFR_UPSTREAM_ROOT"] = str(upstream)
            run(
                [
                    "uv",
                    "run",
                    "--project",
                    ".",
                    "--locked",
                    "--no-sync",
                    "pytest",
                    *paths,
                    "--collect-only",
                    "-q",
                    "--disable-warnings",
                    "-p",
                    "scripts.provenance.collect_upstream",
                ],
                cwd=upstream,
                env=collection_env,
                silent=True,
            )
            try:
                payload = json.loads(output.read_text(encoding="utf-8"))
            except (OSError, json.JSONDecodeError) as error:
                fail(f"invalid {namespace} collection output: {error}")
            if not isinstance(payload, list):
                fail(f"{namespace} collection output is not a list")
            records.extend(payload)
    identities = [
        f"{item['namespace']}::{item['path']}::{item['selector']}#{item['collected_ordinal']}"
        for item in records
    ]
    if len(identities) != len(set(identities)):
        fail("collection produced duplicate normalized identities")
    records.sort(
        key=lambda item: (
            item["namespace"],
            item["path"],
            item["selector"],
            item["collected_ordinal"],
        )
    )
    return records


def tracked_files(upstream: Path) -> list[dict[str, str]]:
    raw = subprocess.run(
        [
            "git",
            "ls-tree",
            "-r",
            "-z",
            PIN,
            "tests",
            "pydantic-core/tests",
        ],
        cwd=upstream,
        check=True,
        capture_output=True,
    ).stdout
    records: list[dict[str, str]] = []
    for entry in raw.split(b"\0"):
        if not entry:
            continue
        metadata, path_bytes = entry.split(b"\t", 1)
        mode, kind, object_id = metadata.decode().split()
        path = path_bytes.decode()
        blob = subprocess.run(
            ["git", "cat-file", "blob", object_id],
            cwd=upstream,
            check=True,
            capture_output=True,
        ).stdout
        records.append(
            {
                "path": path,
                "git_mode": mode,
                "git_object": object_id,
                "sha256": digest(blob),
            }
        )
    records.sort(key=lambda item: item["path"])
    return records


def load_rules() -> list[dict[str, str]]:
    try:
        payload = tomllib.loads(RULES.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        fail(f"invalid anchor rules: {error}")
    rules = payload.get("anchor")
    if not isinstance(rules, list) or not rules:
        fail("anchor rules must contain at least one [[anchor]] row")
    required = {"namespace", "path", "selector", "class", "milestone", "fixture"}
    normalized: list[dict[str, str]] = []
    for index, row in enumerate(rules):
        if not isinstance(row, dict) or set(row) != required:
            fail(f"anchor row {index} has invalid fields")
        item = {key: str(value) for key, value in row.items()}
        if item["namespace"] not in {"api", "core"}:
            fail(f"anchor row {index} has invalid namespace")
        if item["class"] not in {"same", "adapted", "rejected"}:
            fail(f"anchor row {index} has invalid compatibility class")
        normalized.append(item)
    unique = {
        (item["namespace"], item["path"], item["selector"]) for item in normalized
    }
    if len(unique) != len(normalized):
        fail("anchor rules contain duplicate identities")
    return normalized


def classify_nodes(
    records: list[dict[str, Any]], rules: list[dict[str, str]]
) -> list[dict[str, Any]]:
    counts = [0] * len(rules)
    classified: list[dict[str, Any]] = []
    for record in records:
        leaf = str(record["selector"]).split("::")[-1]
        matches = [
            index
            for index, rule in enumerate(rules)
            if rule["namespace"] == record["namespace"]
            and rule["path"] == record["path"]
            and rule["selector"] == leaf
        ]
        if len(matches) > 1:
            fail(f"node has multiple anchor owners: {record['path']}::{record['selector']}")
        item = dict(record)
        item["identity"] = (
            f"{record['namespace']}::{record['path']}::{record['selector']}"
            f"#{record['collected_ordinal']}"
        )
        if matches:
            index = matches[0]
            rule = rules[index]
            counts[index] += 1
            item.update(
                {
                    "compatibility_class": rule["class"],
                    "owner_milestone": rule["milestone"],
                    "fixture_family": rule["fixture"],
                    "disposition_reason": (
                        "mandatory portable selector anchor; normalized by the "
                        "named Sifr fixture family"
                    ),
                }
            )
        else:
            item.update(
                {
                    "compatibility_class": "not-applicable",
                    "owner_milestone": "ps_0",
                    "fixture_family": "disposition/audit",
                    "disposition_reason": (
                        "not selected by the approved portable selector baseline; "
                        "no Python runtime behavior is inherited implicitly"
                    ),
                }
            )
        classified.append(item)
    missing = [
        f"{rule['namespace']}::{rule['path']}::{rule['selector']}"
        for rule, count in zip(rules, counts, strict=True)
        if count == 0
    ]
    if missing:
        fail("mandatory anchors did not collect:\n" + "\n".join(missing))
    return classified


def classify_files(
    files: list[dict[str, str]], nodes: list[dict[str, Any]]
) -> list[dict[str, str]]:
    collected_paths = {str(node["path"]) for node in nodes}
    tracked_paths = {item["path"] for item in files}
    missing = sorted(collected_paths - tracked_paths)
    if missing:
        fail("collected nodes came from untracked paths:\n" + "\n".join(missing))
    for item in files:
        path = item["path"]
        if "benchmark" in path or path.endswith("benchmarks.py"):
            role = "benchmark"
        elif path in collected_paths:
            role = "collected-conformance"
        elif item["git_mode"] == "120000":
            role = "infrastructure"
        elif path.endswith(("conftest.py", "__init__.py", ".ini", ".toml")):
            role = "infrastructure"
        elif path.endswith(".py"):
            role = "not-applicable"
        else:
            role = "fixture"
        item["role"] = role
    return files


def render(files: list[dict[str, str]], nodes: list[dict[str, Any]]) -> bytes:
    ledger = {"files": files, "nodes": nodes}
    ledger_sha256 = digest(canonical(ledger))
    lines = [
        "# Generated by scripts/provenance/generate_upstream_manifest.py.",
        "# Do not edit by hand.",
        "",
        "[meta]",
        "schema_version = 1",
        f"pydantic_commit = {quote(PIN)}",
        f"pydantic_core_version = {quote(CORE_VERSION)}",
        f"pytest_version = {quote(PYTEST_VERSION)}",
        f"historical_standalone_core_commit = {quote(HISTORICAL_CORE)}",
        'historical_standalone_core_disposition = "excluded-research-source"',
        f"file_count = {len(files)}",
        f"node_count = {len(nodes)}",
        f"ledger_sha256 = {quote(ledger_sha256)}",
        "",
    ]
    for item in files:
        lines.extend(
            [
                "[[file]]",
                f"path = {quote(item['path'])}",
                f"git_mode = {quote(item['git_mode'])}",
                f"git_object = {quote(item['git_object'])}",
                f"sha256 = {quote(item['sha256'])}",
                f"role = {quote(item['role'])}",
                "",
            ]
        )
    node_keys = (
        "identity",
        "namespace",
        "path",
        "selector",
        "collected_ordinal",
        "parameter_identity",
        "parameter_value_sha256",
        "source_closure_sha256",
        "compatibility_class",
        "owner_milestone",
        "fixture_family",
        "disposition_reason",
    )
    for item in nodes:
        lines.append("[[node]]")
        for key in node_keys:
            if key == "collected_ordinal":
                lines.append(f"{key} = {int(item[key])}")
            else:
                lines.append(f"{key} = {quote(str(item[key]))}")
        lines.append("")
    return "\n".join(lines).encode()


def first_difference(current: bytes, generated: bytes) -> str:
    current_lines = current.decode("utf-8", errors="replace").splitlines()
    generated_lines = generated.decode("utf-8", errors="replace").splitlines()
    for line_number, (current_line, generated_line) in enumerate(
        itertools.zip_longest(current_lines, generated_lines, fillvalue="<missing>"),
        start=1,
    ):
        if current_line != generated_line:
            return (
                f"first difference at line {line_number}: "
                f"committed={current_line!r}, generated={generated_line!r}"
            )
    return "byte content differs after identical decoded lines"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=OUTPUT)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    upstream = args.upstream.resolve()
    verify_checkout(upstream)
    env = prepare_environment(upstream)
    nodes = classify_nodes(collect_nodes(upstream, env), load_rules())
    files = classify_files(tracked_files(upstream), nodes)
    generated = render(files, nodes)
    output = args.output.resolve()
    if args.check:
        try:
            current = output.read_bytes()
        except OSError as error:
            fail(f"could not read {output}: {error}")
        if current != generated:
            fail(
                f"generated manifest differs from {output}; "
                f"committed_sha256={digest(current)}; "
                f"generated_sha256={digest(generated)}; "
                f"{first_difference(current, generated)}"
            )
        print(
            f"upstream manifest exact-set audit passed: files={len(files)} "
            f"nodes={len(nodes)} sha256={digest(generated)}"
        )
        return 0
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(generated)
    print(
        f"wrote {output}: files={len(files)} nodes={len(nodes)} "
        f"sha256={digest(generated)}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
