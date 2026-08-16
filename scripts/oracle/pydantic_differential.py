#!/usr/bin/env python3
"""Emit canonical outcomes from the pinned Pydantic Core oracle."""

from __future__ import annotations

import json

import pydantic_core
from pydantic_core import core_schema


CASES = (
    ("lax_int", core_schema.int_schema(), '"42"'),
    ("strict_int_error", core_schema.int_schema(strict=True), '"42"'),
    (
        "string_pipeline",
        core_schema.str_schema(
            strip_whitespace=True,
            to_upper=True,
            min_length=3,
            max_length=3,
            pattern="^[a-z]{3}$",
        ),
        '"  abc  "',
    ),
    ("list_int", core_schema.list_schema(core_schema.int_schema()), '[1,"2"]'),
    ("list_error", core_schema.list_schema(core_schema.int_schema()), '[1,"x"]'),
)


def main() -> None:
    outcomes: list[dict[str, object]] = []
    for name, schema, payload in CASES:
        validator = pydantic_core.SchemaValidator(schema)
        try:
            outcome: dict[str, object] = {"ok": validator.validate_json(payload)}
        except pydantic_core.ValidationError as error:
            first = error.errors(include_url=False)[0]
            outcome = {
                "error": {
                    "code": first["type"],
                    "location": list(first["loc"]),
                }
            }
        outcomes.append({"name": name, "outcome": outcome})
    print(json.dumps(outcomes, sort_keys=True, separators=(",", ":")))


if __name__ == "__main__":
    main()
