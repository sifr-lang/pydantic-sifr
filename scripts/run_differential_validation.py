#!/usr/bin/env python3
"""Compare native outcomes with the exact pinned Pydantic Core oracle."""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
UPSTREAM = ROOT / ".upstream/pydantic"
PIN = "f59e929c999e8b2efc7b12fd0bc1685c1a186be3"
CORE_VERSION = "2.47.0"


def run(command: list[str], *, env: dict[str, str] | None = None) -> bytes:
    return subprocess.run(
        command,
        cwd=ROOT,
        env=env,
        check=True,
        stdout=subprocess.PIPE,
    ).stdout.strip()


def main() -> int:
    revision = run(["git", "-C", str(UPSTREAM), "rev-parse", "HEAD"]).decode()
    if revision != PIN:
        raise SystemExit(f"oracle checkout is {revision}, expected {PIN}")
    version = run(
        [
            "uv",
            "run",
            "--project",
            str(UPSTREAM),
            "--locked",
            "--no-sync",
            "python",
            "-c",
            "import pydantic_core; print(pydantic_core.__version__)",
        ]
    ).decode()
    if version != CORE_VERSION:
        raise SystemExit(f"oracle core is {version}, expected {CORE_VERSION}")
    oracle = run(
        [
            "uv",
            "run",
            "--project",
            str(UPSTREAM),
            "--locked",
            "--no-sync",
            "python",
            "scripts/oracle/pydantic_differential.py",
        ]
    )
    native_env = os.environ.copy()
    native_env.setdefault("CARGO_BUILD_JOBS", "6")
    native = run(
        [
            "cargo",
            "run",
            "--quiet",
            "-p",
            "pydantic_sifr_core",
            "--example",
            "differential_probe",
        ],
        env=native_env,
    )
    oracle_value = json.loads(oracle)
    native_value = json.loads(native)
    if native_value != oracle_value:
        raise SystemExit(
            "differential validation failed:\n"
            f"oracle={json.dumps(oracle_value, sort_keys=True)}\n"
            f"native={json.dumps(native_value, sort_keys=True)}"
        )
    digest = hashlib.sha256(oracle).hexdigest()
    print(f"differential validation passed: cases={len(oracle_value)} sha256={digest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
