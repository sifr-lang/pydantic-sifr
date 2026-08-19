#!/usr/bin/env python3
"""Prove descriptor issues identify the exact invalid argument."""

from __future__ import annotations

import argparse
import shutil
import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
HARNESS = ROOT / "demos/milestone_m8_fields_configuration"
CASES = {
    "tests/sifr/invalid_field_descriptor_argument.sifr": (
        "invalid_multiple_of",
        "multiple_of=0",
    ),
    "tests/sifr/invalid_extra_allow_argument.sifr": (
        "extra_allow_requires_destination",
        'extra="allow"',
    ),
}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--sifr-bin", required=True)
    args = parser.parse_args()
    sifr_bin = str(Path(args.sifr_bin).resolve())
    for relative, (reason, source_argument) in CASES.items():
        with tempfile.TemporaryDirectory(
            prefix="pydantic-sifr-descriptor-", dir=ROOT / "demos"
        ) as raw:
            project = Path(raw)
            shutil.copyfile(HARNESS / "Cargo.toml", project / "Cargo.toml")
            shutil.copyfile(HARNESS / "Cargo.lock", project / "Cargo.lock")
            shutil.copyfile(HARNESS / "sifr.toml", project / "sifr.toml")
            (project / "src").mkdir()
            shutil.copyfile(HARNESS / "src/lib.rs", project / "src/lib.rs")
            shutil.copyfile(ROOT / relative, project / "src/main.sifr")
            result = subprocess.run(
                [sifr_bin, "check", "src/main.sifr"],
                cwd=project,
                capture_output=True,
                text=True,
            )
        output = result.stdout + result.stderr
        if result.returncode == 0:
            raise SystemExit(f"invalid descriptor unexpectedly passed: {relative}")
        if reason not in output or source_argument not in output:
            raise SystemExit(
                f"descriptor issue did not identify its argument: {relative}\n{output}"
            )
    print(f"descriptor argument checks passed: {len(CASES)} exact diagnostics")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
