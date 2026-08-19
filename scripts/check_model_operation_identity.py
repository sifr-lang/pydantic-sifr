#!/usr/bin/env python3
"""Record one static-program identity for all M11 model operations."""

from __future__ import annotations

import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DEMO = ROOT / "demos/milestone_m11_model_operations"
SOURCE = DEMO / "src/main.sifr"
EMITTED = DEMO / "sifr_output/src/main.rs"
IDENTITY = re.compile(
    r"__SIFR_STATIC_PROGRAM_IDENTITY_USER_[A-Z0-9_]+: \[u8; 32\] = \[([^]]+)\];"
)


def main() -> int:
    source = SOURCE.read_text(encoding="utf-8")
    for operation in (
        "User.model_validate_json(",
        "from_json.model_dump_json(",
        "User.model_json_schema(",
    ):
        if operation not in source:
            raise SystemExit(f"M11 demo is missing {operation}")
    emitted = EMITTED.read_text(encoding="utf-8")
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
