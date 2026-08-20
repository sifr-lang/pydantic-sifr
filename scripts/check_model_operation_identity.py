#!/usr/bin/env python3
"""Record one static-program identity for all M11 model operations."""

from __future__ import annotations

import argparse
import re
import shutil
import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DEMO = ROOT / "demos/milestone_m11_model_operations"
SOURCE = DEMO / "src/main.sifr"
IDENTITY = re.compile(
    r"__SIFR_STATIC_PROGRAM_IDENTITY_USER_[A-Z0-9_]+: \[u8; 32\] = \[([^]]+)\];"
)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--sifr-bin", required=True)
    args = parser.parse_args()
    sifr_bin = str(Path(args.sifr_bin).resolve())
    source = SOURCE.read_text(encoding="utf-8")
    for operation in (
        "User.model_validate_json(",
        "from_json.model_dump_json(",
        "User.model_json_schema(",
    ):
        if operation not in source:
            raise SystemExit(f"M11 demo is missing {operation}")
    with tempfile.TemporaryDirectory(
        prefix="pydantic-sifr-model-operation-identity-", dir=ROOT / "demos"
    ) as raw:
        project = Path(raw)
        for filename in ("Cargo.toml", "Cargo.lock", "sifr.toml"):
            shutil.copyfile(DEMO / filename, project / filename)
        (project / "src").mkdir()
        shutil.copyfile(DEMO / "src/lib.rs", project / "src/lib.rs")
        shutil.copyfile(SOURCE, project / "src/main.sifr")
        subprocess.run(
            [sifr_bin, "build", "--locked", "src/main.sifr"],
            cwd=project,
            check=True,
            capture_output=True,
            text=True,
        )
        emitted = (project / "sifr_output/src/main.rs").read_text(encoding="utf-8")
    matches = IDENTITY.findall(emitted)
    if len(matches) != 1:
        raise SystemExit(
            "expected one User static-program identity for validation, dump, and schema; "
            f"found {len(matches)}"
        )
    identity = bytes(
        int(part.strip()) for part in matches[0].split(",") if part.strip()
    )
    if len(identity) != 32:
        raise SystemExit("User static-program identity is not 32 bytes")
    print(
        "model-operation static-program identity passed: "
        f"validation=dump=json_schema={identity.hex()}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
