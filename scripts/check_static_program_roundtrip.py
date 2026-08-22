#!/usr/bin/env python3
"""Match released-Sifr static bytes to the Rust core envelope fixture."""

from __future__ import annotations

import argparse
import re
import shutil
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "tests/sifr/static_schema_contract.sifr"
HARNESS = ROOT / "demos/fields_and_configuration"
PROGRAM_FIXTURE = ROOT / "tests/static_program/schema_contract_program.txt"
IDENTITY_FIXTURE = ROOT / "tests/static_program/schema_contract_program.identity"

BYTE_PATTERN = re.compile(
    r"__SIFR_STATIC_PROGRAM_BYTES_[A-Z0-9_]+: &\[u8\] = &\[([^]]+)\];"
)
IDENTITY_PATTERN = re.compile(
    r"__SIFR_STATIC_PROGRAM_IDENTITY_[A-Z0-9_]+: \[u8; 32\] = \[([^]]+)\];"
)


def parse_numbers(value: str) -> bytes:
    return bytes(int(part.strip()) for part in value.split(",") if part.strip())


def one_match(pattern: re.Pattern[str], emitted: str, label: str) -> str:
    matches = pattern.findall(emitted)
    if len(matches) != 1:
        raise SystemExit(f"expected one emitted {label}; found {len(matches)}")
    return matches[0]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--sifr-bin", required=True)
    parser.add_argument("--update", action="store_true")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(
        prefix="pydantic-sifr-static-program-", dir=ROOT / "demos"
    ) as raw:
        project = Path(raw)
        shutil.copyfile(HARNESS / "Cargo.toml", project / "Cargo.toml")
        shutil.copyfile(HARNESS / "Cargo.lock", project / "Cargo.lock")
        shutil.copyfile(HARNESS / "sifr.toml", project / "sifr.toml")
        (project / "src").mkdir()
        shutil.copyfile(HARNESS / "src/lib.rs", project / "src/lib.rs")
        shutil.copyfile(SOURCE, project / "src/main.sifr")
        subprocess.run(
            [args.sifr_bin, "build", "--locked", "src/main.sifr"],
            cwd=project,
            check=True,
            capture_output=True,
            text=True,
        )
        emitted = (project / "sifr_output/src/main.rs").read_text(encoding="utf-8")
    actual_program = parse_numbers(one_match(BYTE_PATTERN, emitted, "program"))
    actual_identity = parse_numbers(one_match(IDENTITY_PATTERN, emitted, "identity"))
    if args.update:
        PROGRAM_FIXTURE.write_bytes(actual_program + b"\n")
        IDENTITY_FIXTURE.write_text(actual_identity.hex() + "\n", encoding="utf-8")
    expected_program = PROGRAM_FIXTURE.read_bytes().removesuffix(b"\n")
    expected_identity = bytes.fromhex(IDENTITY_FIXTURE.read_text().strip())
    if actual_program != expected_program:
        raise SystemExit("released Sifr emitted different canonical schema-program bytes")
    if actual_identity != expected_identity:
        raise SystemExit("released Sifr emitted a different schema-program identity")
    if len(actual_identity) != 32:
        raise SystemExit("schema-program identity is not 32 bytes")
    print(
        "static-program round trip passed: "
        f"bytes={len(actual_program)} identity={actual_identity.hex()}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
