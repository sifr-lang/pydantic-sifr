from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
PACKAGE_ROOT = ROOT / "src/__init__.sifr"


def package_root_exports(source: str) -> list[str]:
    exports: list[str] = []
    for line in source.splitlines():
        if not line:
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
                "JsonSchemaError",
                "SerializationError",
                "ValidationError",
                "model_dump_json",
                "model_json_schema",
                "model_validate",
                "model_validate_json",
                "model_validate_strings",
                "verify_schema",
                "MultiHostUrl",
                "Pattern",
                "SpecialValueError",
                "Url",
            ],
        )

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
