#!/usr/bin/env python3
"""Match package sum categories to the exact Sifr compiler source."""

from __future__ import annotations

import argparse
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PACKAGE_SOURCE = ROOT / "backend/pydantic_sifr_core/src/validation/sum_schema.rs"


def category(source: str, arm: str, label: str) -> int:
    match = re.search(rf"{arm}\s*=>\s*\((\d+),", source)
    if match is None:
        raise SystemExit(f"missing canonical union category for {label}")
    return int(match.group(1))


def block_category(source: str, arm: str, label: str) -> int:
    match = re.search(rf"{arm}\s*=>\s*\{{\s*\((\d+),", source)
    if match is None:
        raise SystemExit(f"missing canonical union category for {label}")
    return int(match.group(1))


def secondary_expression(source: str, arm: str, label: str) -> str:
    match = re.search(rf"^{arm}\s*=>\s*\(\d+,\s*(.+)\),$", source, re.MULTILINE)
    if match is None:
        raise SystemExit(f"missing canonical union secondary key for {label}")
    return match.group(1).strip()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--sifr-bin", required=True)
    args = parser.parse_args()

    sifr_root = Path(args.sifr_bin).resolve().parents[2]
    compiler_path = sifr_root / "crates/sifr_type_system/src/union.rs"
    if not compiler_path.is_file():
        raise SystemExit(f"cannot find Sifr union source beside SIFR_BIN: {compiler_path}")

    compiler = compiler_path.read_text(encoding="utf-8")
    package = PACKAGE_SOURCE.read_text(encoding="utf-8")
    compiler_categories = {
        "none": category(compiler, r"Type::None", "Sifr None"),
        "bool": category(compiler, r"Type::Bool", "Sifr bool"),
        "int": category(compiler, r"Type::Int", "Sifr int"),
        "fixed_int": category(compiler, r"Type::FixedInt\([^)]*\)", "Sifr fixed int"),
        "float": category(compiler, r"Type::Float", "Sifr float"),
        "string": category(compiler, r"Type::Str", "Sifr str"),
        "bytes": category(compiler, r"Type::Bytes", "Sifr bytes"),
        "list": category(compiler, r"Type::List\([^)]*\)", "Sifr list"),
        "mapping": category(compiler, r"Type::Dict\([^)]*\)", "Sifr dict"),
        "set": category(compiler, r"Type::Set\([^)]*\)", "Sifr set"),
        "tuple": category(compiler, r"Type::Tuple\([^)]*\)", "Sifr tuple"),
        "class": category(compiler, r"Type::Class\s*\{[^}]*\}", "Sifr class"),
        "newtype": category(compiler, r"Type::Newtype\s*\{[^}]*\}", "Sifr newtype"),
        "enum": category(compiler, r"Type::Enum\s*\{[^}]*\}", "Sifr enum"),
        "bigdecimal": category(compiler, r"Type::BigDecimal", "Sifr bigdecimal"),
    }
    package_categories = {
        "none": category(package, r"Schema::None", "package None"),
        "bool": category(package, r"Schema::Bool", "package bool"),
        "int": block_category(
            package,
            r"Schema::Integer\s*\{\s*target,\s*\.\.\s*\}\s*if[^=]*==[^=]*Exact",
            "package exact int",
        ),
        "fixed_int": category(
            package,
            r"Schema::Integer\s*\{\s*target,\s*\.\.\s*\}",
            "package fixed int",
        ),
        "float": category(package, r"Schema::Float\([^)]*\)", "package float"),
        "string": category(
            package,
            r"Schema::String\([^)]*\)\s*\|\s*Schema::Url\([^)]*\)",
            "package string",
        ),
        "bytes": category(
            package,
            r"Schema::Bytes\([^)]*\)\s*\|\s*Schema::Uuid\s*\{[^}]*\}",
            "package bytes",
        ),
        "list": category(
            package,
            r"Schema::List\s*\{[^}]*\}\s*\|\s*Schema::Generator\s*\{[^}]*\}",
            "package list",
        ),
        "mapping": category(package, r"Schema::Mapping\s*\{[^}]*\}", "package mapping"),
        "set": category(package, r"Schema::Set\s*\{[^}]*\}", "package set"),
        "tuple": category(package, r"Schema::Tuple\([^)]*\)", "package tuple"),
        "class": category(package, r"Schema::Model\([^)]*\)", "package model"),
        "newtype": category(package, r"Schema::Temporal\([^)]*\)", "package newtype"),
        "enum": category(package, r"Schema::Enum\([^)]*\)", "package enum"),
        "bigdecimal": category(package, r"Schema::Decimal\([^)]*\)", "package bigdecimal"),
    }
    if package_categories != compiler_categories:
        raise SystemExit(
            "Sifr/package canonical union categories differ:\n"
            f"compiler={compiler_categories}\npackage={package_categories}"
        )
    secondary_keys = {
        "compiler_class": secondary_expression(
            compiler, r"\s*Type::Class\s*\{\s*\.\.\s*\}", "Sifr class"
        ),
        "package_model": secondary_expression(
            package, r"\s*Schema::Model\(model\)", "package model"
        ),
        "package_frozen_set": secondary_expression(
            package, r"\s*Schema::FrozenSet\s*\{\s*\.\.\s*\}", "package frozen set"
        ),
    }
    expected_secondary_keys = {
        "compiler_class": "ty.display_name()",
        "package_model": "bare_class_name(model.name).to_owned()",
        "package_frozen_set": '"frozenset".to_owned()',
    }
    if secondary_keys != expected_secondary_keys:
        raise SystemExit(
            "Sifr/package canonical class secondary keys differ:\n"
            f"actual={secondary_keys}\nexpected={expected_secondary_keys}"
        )
    if "name.rfind('.').map_or(0, |index| index + 1)" not in package:
        raise SystemExit("package model ordering does not use the bare class name")
    print(f"Sifr union order check passed: {compiler_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
