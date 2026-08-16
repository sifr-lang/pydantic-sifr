#!/usr/bin/env python3
"""Prove invalid schemas fail during released-Sifr specialization."""

from __future__ import annotations

import argparse
import shutil
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CASES = {
    "tests/sifr/invalid_custom_error.sifr": "custom error declaration is incomplete",
    "tests/sifr/invalid_builtin_error.sifr": "built-in error message cannot change",
    "tests/sifr/invalid_collection_constraint.sifr": (
        "field constraints are not supported for collections"
    ),
}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--sifr-bin", required=True)
    args = parser.parse_args()
    sifr_bin = str(Path(args.sifr_bin).resolve())
    for relative, expected in CASES.items():
        with tempfile.TemporaryDirectory(prefix="pydantic-sifr-negative-") as temp:
            probe = Path(temp)
            shutil.copyfile(ROOT / "src/errors.sifr", probe / "errors.sifr")
            shutil.copyfile(ROOT / "src/schema_types.sifr", probe / "schema_types.sifr")
            contract = (ROOT / "src/schema_contract.sifr").read_text(
                encoding="utf-8"
            ).replace("pydantic_sifr.schema_contract", "schema_contract")
            (probe / "schema_contract.sifr").write_text(contract, encoding="utf-8")
            source = (ROOT / relative).read_text(encoding="utf-8").replace(
                "pydantic_sifr.schema_contract", "schema_contract"
            )
            (probe / "main.sifr").write_text(source, encoding="utf-8")
            (probe / "sifr.toml").write_text(
                "\n".join(
                    [
                        "[package]",
                        'name = "schema_failure_probe"',
                        'version = "0.0.0"',
                        'edition = "2026"',
                        'sifr-version = ">=0.3,<0.4"',
                        "",
                        "[source]",
                        'roots = ["."]',
                        "",
                    ]
                ),
                encoding="utf-8",
            )
            (probe / "marker.rs").write_text(
                "// Native marker for the temporary Sifr package.\n", encoding="utf-8"
            )
            (probe / "Cargo.toml").write_text(
                "\n".join(
                    [
                        "[package]",
                        'name = "schema-failure-probe"',
                        'version = "0.0.0"',
                        'edition = "2024"',
                        "",
                        "[lib]",
                        'path = "marker.rs"',
                        "",
                        "[workspace]",
                        "",
                    ]
                ),
                encoding="utf-8",
            )
            result = subprocess.run(
                [sifr_bin, "check", "main.sifr"],
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
