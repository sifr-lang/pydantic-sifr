from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
PACKAGE_ROOT = ROOT / "src/__init__.sifr"
ARCHITECTURE = ROOT / "docs/architecture.md"


def package_root_exports(source: str) -> list[str]:
    exports: list[str] = []
    for line in source.splitlines():
        if not line:
            continue
        alias = re.fullmatch(
            r"type ([A-Za-z_][A-Za-z0-9_]*)\[[A-Za-z_][A-Za-z0-9_]*\] = "
            r"[A-Za-z_][A-Za-z0-9_]*",
            line,
        )
        if alias is not None:
            exports.append(alias.group(1))
            continue
        statement = re.fullmatch(r"from [A-Za-z0-9_.]+ import (.+)", line)
        if statement is None:
            raise ValueError(f"unsupported package-root statement: {line}")
        for imported in statement.group(1).split(","):
            binding = re.fullmatch(
                r"\s*([A-Za-z_][A-Za-z0-9_]*)"
                r"(?:\s+as\s+([A-Za-z_][A-Za-z0-9_]*))?\s*",
                imported,
            )
            if binding is None:
                raise ValueError(f"unsupported import binding: {imported}")
            exports.append(binding.group(2) or binding.group(1))
    return exports


class PublicApiTest(unittest.TestCase):
    def test_package_root_exports_only_the_supported_surface(self) -> None:
        source = PACKAGE_ROOT.read_text(encoding="utf-8")
        self.assertEqual(
            package_root_exports(source),
            [
                "AliasChoices",
                "AliasPath",
                "BaseModel",
                "ConfigDict",
                "Constraints",
                "Field",
                "RootModel",
                "field_validator",
                "field_serializer",
                "computed_field",
                "model_validator",
                "model_serializer",
                "JsonSchemaError",
                "SerializationError",
                "ValidationError",
                "model_dump",
                "model_dump_json",
                "model_dump_json_with_serializers",
                "model_dump_with_serializers",
                "model_validate",
                "model_validate_json",
                "model_validate_json_with_validators",
                "model_validate_strings",
                "model_validate_strings_with_validators",
                "model_validate_with_validators",
                "verify_schema",
                "MultiHostUrl",
                "Pattern",
                "SpecialValueError",
                "Url",
                "TypeAdapter",
            ],
        )

    def test_architecture_names_every_package_root_export(self) -> None:
        exports = package_root_exports(PACKAGE_ROOT.read_text(encoding="utf-8"))
        architecture = ARCHITECTURE.read_text(encoding="utf-8")
        for name in exports:
            self.assertIn(f"`{name}`", architecture)

    def test_export_parser_exposes_aliases_and_fails_closed(self) -> None:
        source = "from sifr.meta import StaticProgram as VerifiedSchemaProgram\n"
        self.assertEqual(package_root_exports(source), ["VerifiedSchemaProgram"])
        with self.assertRaises(ValueError):
            package_root_exports("export hidden_construction_api\n")

    def test_construction_time_version_constants_are_removed(self) -> None:
        sources = "\n".join(
            path.read_text(encoding="utf-8")
            for path in sorted((ROOT / "src").rglob("*.sifr"))
        )
        for name in (
            "SCHEMA_PROGRAM_FORMAT_VERSION",
            "STRUCTURAL_CONTRACT_VERSION",
            "STRUCTURAL_CALL_VERSION",
            "CALLBACK_ABI_VERSION",
        ):
            self.assertNotIn(name, sources)


if __name__ == "__main__":
    unittest.main()
