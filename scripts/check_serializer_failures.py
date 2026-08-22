#!/usr/bin/env python3
"""Prove serializer and computed-field declarations reject invalid signatures."""

from __future__ import annotations

import argparse
import shutil
import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
HARNESS = ROOT / "demos/serializers_and_computed_fields"
HEADER = (
    "from pydantic_sifr import "
    "BaseModel, computed_field, field_serializer, model_serializer\n\n"
)

CASES = {
    "wildcard": (
        "wildcard_serializer_target",
        '''class Wildcard(BaseModel):
    value: str

    @staticmethod
    @field_serializer("*")
    def reject_wildcard(own value: str) -> str:
        return value
''',
    ),
    "unknown_target": (
        "field serializer target does not name a declared field",
        '''class UnknownTarget(BaseModel):
    value: str

    @staticmethod
    @field_serializer("missing")
    def reject_unknown_target(own value: str) -> str:
        return value
''',
    ),
    "field_receiver": (
        "field serializer receiver must be an immutable instance borrow",
        '''class FieldReceiver(BaseModel):
    value: str

    @field_serializer("value")
    def reject_receiver(mut self, own value: str) -> str:
        return value
''',
    ),
    "field_input": (
        "field serializer input does not match its field type",
        '''class FieldInput(BaseModel):
    value: str

    @staticmethod
    @field_serializer("value")
    def reject_input(own value: bool) -> str:
        return str(value)
''',
    ),
    "field_borrowed_input": (
        "field serializer value input must be owned",
        '''class FieldBorrowedInput(BaseModel):
    value: str

    @staticmethod
    @field_serializer("value")
    def reject_borrowed_input(value: str) -> str:
        return ""
''',
    ),
    "field_when_used": (
        "invalid_serializer_when_used",
        '''class FieldWhenUsed(BaseModel):
    value: str

    @staticmethod
    @field_serializer("value", when_used="sometimes")
    def reject_when_used(own value: str) -> str:
        return value
''',
    ),
    "model_receiver": (
        "model serializer must use an immutable instance receiver",
        '''class ModelReceiver(BaseModel):
    value: str

    @model_serializer()
    @classmethod
    def reject_receiver(cls) -> str:
        return ""
''',
    ),
    "duplicate_model": (
        "a model can declare at most one model serializer",
        '''class DuplicateModel(BaseModel):
    value: str

    @model_serializer()
    def first(self) -> str:
        return self.value

    @model_serializer()
    def second(self) -> str:
        return self.value
''',
    ),
    "computed_receiver": (
        "computed field must use an immutable instance receiver",
        '''class ComputedReceiver(BaseModel):
    value: str

    @staticmethod
    @computed_field()
    def reject_receiver() -> str:
        return ""
''',
    ),
    "computed_argument": (
        "computed field must be a zero-argument instance method",
        '''class ComputedArgument(BaseModel):
    value: str

    @computed_field()
    def reject_argument(self, own extra: str) -> str:
        return extra
''',
    ),
    "computed_output": (
        "computed field must return a value",
        '''class ComputedOutput(BaseModel):
    value: str

    @computed_field()
    def reject_output(self) -> None:
        return None
''',
    ),
    "computed_alias": (
        "computed field alias must not be empty",
        '''class ComputedAlias(BaseModel):
    value: str

    @computed_field(alias="")
    def reject_alias(self) -> str:
        return self.value
''',
    ),
    "async_serializer": (
        "serializer and computed-field methods cannot be async",
        '''class AsyncSerializer(BaseModel):
    value: str

    @staticmethod
    @field_serializer("value")
    async def reject_async(own value: str) -> str:
        return value
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
            prefix="pydantic-sifr-serializer-", dir=ROOT / "demos"
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
            raise SystemExit(f"invalid serializer unexpectedly passed: {name}")
        if expected not in output:
            raise SystemExit(
                f"serializer diagnostic mismatch for {name}; expected {expected!r}\n"
                f"{output}"
            )
    print(f"serializer failure checks passed: {len(CASES)} diagnostics")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
