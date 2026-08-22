#!/usr/bin/env python3
"""Prove invalid schemas fail during released-Sifr specialization."""

from __future__ import annotations

import argparse
import shutil
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
HARNESS = ROOT / "demos/fields_and_configuration"
CASES = {
    "tests/sifr/invalid_custom_error.sifr": (
        "custom error declaration is incomplete"
    ),
    "tests/sifr/invalid_builtin_error.sifr": (
        "built-in error message cannot change"
    ),
    "tests/sifr/invalid_collection_constraint.sifr": (
        "text constraints require a string field"
    ),
    "tests/sifr/invalid_merged_constraint.sifr": (
        "min_length cannot exceed max_length after descriptor merge"
    ),
    "tests/sifr/invalid_literal_constraint.sifr": (
        "field constraints are not supported for literal fields"
    ),
    "tests/sifr/invalid_recursive_constraint.sifr": (
        "field constraints are not supported for recursive references"
    ),
    "tests/sifr/invalid_union_constraint.sifr": (
        "field constraints are not supported for unions"
    ),
}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--sifr-bin", required=True)
    args = parser.parse_args()
    sifr_bin = str(Path(args.sifr_bin).resolve())
    for relative, expected in CASES.items():
        with tempfile.TemporaryDirectory(
            prefix="pydantic-sifr-negative-", dir=ROOT / "demos"
        ) as raw:
            probe = Path(raw)
            shutil.copyfile(HARNESS / "Cargo.toml", probe / "Cargo.toml")
            shutil.copyfile(HARNESS / "Cargo.lock", probe / "Cargo.lock")
            shutil.copyfile(HARNESS / "sifr.toml", probe / "sifr.toml")
            (probe / "src").mkdir()
            shutil.copyfile(HARNESS / "src/lib.rs", probe / "src/lib.rs")
            shutil.copyfile(ROOT / relative, probe / "src/main.sifr")
            result = subprocess.run(
                [sifr_bin, "check", "src/main.sifr"],
                cwd=probe,
                capture_output=True,
                text=True,
            )
        output = result.stdout + result.stderr
        if result.returncode == 0:
            raise SystemExit(f"invalid schema unexpectedly passed: {relative}")
        if "schema_invalid" not in output or expected not in output:
            raise SystemExit(
                f"invalid schema returned the wrong diagnostic: {relative}\n{output}"
            )
    print(f"Sifr schema failure checks passed: {len(CASES)} stable diagnostics")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
