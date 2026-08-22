#!/usr/bin/env python3
"""Prove validator declarations reject invalid modes, targets, and signatures."""

from __future__ import annotations

import argparse
import shutil
import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
HARNESS = ROOT / "demos/validators"
HEADER = "from pydantic_sifr import BaseModel, field_validator, model_validator\n\n"

CASES = {
    "wildcard": (
        "wildcard_validator_target",
        '''class Wildcard(BaseModel):
    value: str

    @field_validator("*")
    @classmethod
    def reject_wildcard(cls, own value: str) -> str:
        return value
''',
    ),
    "unknown_target": (
        "field validator target does not name a declared field",
        '''class UnknownTarget(BaseModel):
    value: str

    @field_validator("missing")
    @classmethod
    def reject_unknown_target(cls, own value: str) -> str:
        return value
''',
    ),
    "field_receiver": (
        "field validator cannot declare an instance receiver",
        '''class FieldReceiver(BaseModel):
    value: str

    @field_validator("value")
    def reject_field_receiver(own self, own value: str) -> str:
        return value
''',
    ),
    "field_mode": (
        "invalid_field_validator_mode",
        '''class FieldMode(BaseModel):
    value: str

    @field_validator("value", mode="around")
    @classmethod
    def reject_field_mode(cls, own value: str) -> str:
        return value
''',
    ),
    "field_after_input": (
        "field after validator input does not match its field type",
        '''class FieldAfterInput(BaseModel):
    value: str

    @field_validator("value", mode="after")
    @classmethod
    def reject_field_after_input(cls, own value: bool) -> str:
        return str(value)
''',
    ),
    "field_output": (
        "field validator output does not match its field type",
        '''class FieldOutput(BaseModel):
    value: str

    @field_validator("value")
    @classmethod
    def reject_field_output(cls, own value: str) -> bool:
        return True
''',
    ),
    "field_borrowed_input": (
        "field validator value input must be owned",
        '''class FieldBorrowedInput(BaseModel):
    value: str

    @field_validator("value")
    @classmethod
    def reject_field_borrowed_input(cls, value: str) -> str:
        return ""
''',
    ),
    "model_before_input": (
        "model before validator input must be a concrete structural type",
        '''class ModelBeforeInput(BaseModel):
    value: str

    @model_validator(mode="before")
    @classmethod
    def reject_model_before_input(cls, own value: str) -> str:
        return value
''',
    ),
    "model_after_receiver": (
        "model after validator must consume its model receiver",
        '''class ModelAfterReceiver(BaseModel):
    value: str

    @model_validator(mode="after")
    def reject_model_after_receiver(self) -> Self:
        return ModelAfterReceiver("")
''',
    ),
    "model_mode": (
        "invalid_model_validator_mode",
        '''class ModelMode(BaseModel):
    value: str

    @model_validator(mode="wrap")
    def reject_model_mode(own self) -> Self:
        return self
''',
    ),
    "model_after_output": (
        "model after validator output must be its model type",
        '''class ModelAfterOutput(BaseModel):
    value: str

    @model_validator(mode="after")
    def reject_model_after_output(own self) -> str:
        return self.value
''',
    ),
    "async_validator": (
        "validator methods cannot be async",
        '''class AsyncValidator(BaseModel):
    value: str

    @field_validator("value")
    @classmethod
    async def reject_async_validator(cls, own value: str) -> str:
        return value
''',
    ),
    "nested_validator": (
        "nested models with validators require their own validation boundary",
        '''class Inner(BaseModel):
    value: str

    @field_validator("value")
    @classmethod
    def inner_step(cls, own value: str) -> str:
        return value


class Outer(BaseModel):
    inner: Inner
''',
    ),
}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--sifr-bin", required=True)
    args = parser.parse_args()
    sifr_bin = str(Path(args.sifr_bin).resolve())
    for name, (expected, body) in CASES.items():
        with tempfile.TemporaryDirectory(
            prefix="pydantic-sifr-validator-", dir=ROOT / "demos"
        ) as raw:
            project = Path(raw)
            for filename in ("Cargo.toml", "Cargo.lock", "sifr.toml"):
                shutil.copyfile(HARNESS / filename, project / filename)
            (project / "src").mkdir()
            shutil.copyfile(HARNESS / "src/lib.rs", project / "src/lib.rs")
            (project / "src/main.sifr").write_text(HEADER + body)
            result = subprocess.run(
                [sifr_bin, "check", "src/main.sifr"],
                cwd=project,
                capture_output=True,
                text=True,
            )
        output = result.stdout + result.stderr
        if result.returncode == 0:
            raise SystemExit(f"invalid validator unexpectedly passed: {name}")
        if expected not in output:
            raise SystemExit(
                f"validator diagnostic mismatch for {name}; expected {expected!r}\n"
                f"{output}"
            )
    print(f"validator failure checks passed: {len(CASES)} diagnostics")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
