#!/usr/bin/env python3
"""Prove that model-operation type bounds reject invalid input."""

from __future__ import annotations

import argparse
import shutil
import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
HARNESS = ROOT / "demos/model_validation"
SOURCE = """from pydantic_sifr import BaseModel


class Model(BaseModel):
    value: int64


class InvalidStrings:
    value: int64


def main():
    Model.model_validate_strings(InvalidStrings(1))
"""


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--sifr-bin", required=True)
    args = parser.parse_args()
    sifr_bin = str(Path(args.sifr_bin).resolve())
    with tempfile.TemporaryDirectory(
        prefix="pydantic-sifr-model-operation-", dir=ROOT / "demos"
    ) as raw:
        project = Path(raw)
        for filename in ("Cargo.toml", "Cargo.lock", "sifr.toml"):
            shutil.copyfile(HARNESS / filename, project / filename)
        (project / "src").mkdir()
        shutil.copyfile(HARNESS / "src/lib.rs", project / "src/lib.rs")
        (project / "src/main.sifr").write_text(SOURCE, encoding="utf-8")
        result = subprocess.run(
            [sifr_bin, "check", "src/main.sifr"],
            cwd=project,
            capture_output=True,
            text=True,
        )
    output = result.stdout + result.stderr
    if result.returncode == 0:
        raise SystemExit("non-string model-operation input unexpectedly passed")
    if "SIFR-PROTO-0001" not in output:
        raise SystemExit(
            "model-operation diagnostic mismatch; expected 'SIFR-PROTO-0001'\n"
            + output
        )
    print("model-operation failure checks passed: 1 diagnostic")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
