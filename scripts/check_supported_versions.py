#!/usr/bin/env python3
from __future__ import annotations

import argparse
import re
import subprocess
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
LEDGER = ROOT / "tests/certification/supported_versions.toml"


def load_toml(path: Path) -> dict[str, object]:
    return tomllib.loads(path.read_text(encoding="utf-8"))


def package_version(manifest: Path) -> str:
    package = load_toml(manifest).get("package")
    if not isinstance(package, dict) or not isinstance(package.get("version"), str):
        raise SystemExit(f"{manifest}: missing package.version")
    return package["version"]


def sifr_revision(manifest: Path) -> str:
    text = manifest.read_text(encoding="utf-8")
    match = re.search(r'sifr_runtime\s*=\s*\{[^\n]*rev\s*=\s*"([0-9a-f]{40})"', text)
    if match is None:
        raise SystemExit(f"{manifest}: missing exact sifr_runtime revision")
    return match.group(1)


def certified_combination() -> dict[str, str]:
    document = load_toml(LEDGER)
    if document.get("schema") != 1:
        raise SystemExit("supported version ledger schema must be 1")
    rows = document.get("combination")
    if not isinstance(rows, list) or len(rows) != 1 or not isinstance(rows[0], dict):
        raise SystemExit("supported version ledger must contain exactly one combination")
    row = rows[0]
    required = {
        "sifr_source_rev",
        "sifr_cli_version",
        "sifr_package_requirement",
        "package_version",
        "core_version",
        "status",
    }
    if set(row) != required or not all(isinstance(row[key], str) for key in required):
        raise SystemExit("supported version combination has an invalid shape")
    return {key: row[key] for key in required}


def check_manifests(row: dict[str, str]) -> None:
    root_manifest = ROOT / "Cargo.toml"
    core_manifest = ROOT / "backend/pydantic_sifr_core/Cargo.toml"
    sifr_manifest = load_toml(ROOT / "sifr.toml")
    sifr_package = sifr_manifest.get("package")
    if not isinstance(sifr_package, dict):
        raise SystemExit("sifr.toml: missing package table")

    actual = {
        "package_version": package_version(root_manifest),
        "core_version": package_version(core_manifest),
        "sifr_package_requirement": sifr_package.get("sifr-version"),
    }
    for key, value in actual.items():
        if value != row[key]:
            raise SystemExit(f"{key} is {value!r}, expected {row[key]!r}")

    for manifest in (root_manifest, core_manifest):
        revision = sifr_revision(manifest)
        if revision != row["sifr_source_rev"]:
            raise SystemExit(
                f"{manifest}: Sifr revision is {revision}, expected {row['sifr_source_rev']}"
            )
    if row["status"] != "certified":
        raise SystemExit("the only supported version combination is not certified")


def check_compiler(row: dict[str, str], sifr_bin: Path) -> None:
    result = subprocess.run(
        [str(sifr_bin), "--version"],
        check=True,
        capture_output=True,
        text=True,
    )
    actual = result.stdout.strip()
    expected = f"sifr {row['sifr_cli_version']}"
    if actual != expected:
        raise SystemExit(f"compiler reports {actual!r}, expected {expected!r}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest-only", action="store_true")
    parser.add_argument("--sifr-bin", type=Path)
    args = parser.parse_args()
    if not args.manifest_only and args.sifr_bin is None:
        parser.error("--sifr-bin is required unless --manifest-only is used")

    row = certified_combination()
    check_manifests(row)
    if not args.manifest_only and args.sifr_bin is not None:
        check_compiler(row, args.sifr_bin)
    print(
        "supported version certification passed: "
        f"sifr={row['sifr_cli_version']} package={row['package_version']} "
        f"core={row['core_version']} rev={row['sifr_source_rev']}"
    )


if __name__ == "__main__":
    main()
