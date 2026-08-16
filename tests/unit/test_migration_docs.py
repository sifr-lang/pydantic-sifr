from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
GUIDE = ROOT / "docs/migration.md"


class MigrationDocsTest(unittest.TestCase):
    def test_guide_names_the_supported_migration_contract(self) -> None:
        guide = GUIDE.read_text(encoding="utf-8")
        required = {
            '@const_specialize("pydantic_sifr.schema_contract", "verify_schema")',
            "@metadata",
            "model_validate_json",
            "model_validate_strings",
            "Result",
            "ValidationError",
            "RustPanicError",
            "compatibility matrix",
        }
        for marker in required:
            self.assertIn(marker, guide)

    def test_guide_names_each_blocked_surface_owner(self) -> None:
        guide = GUIDE.read_text(encoding="utf-8")
        for issue in (10, 14, 27):
            self.assertIn(
                f"https://github.com/sifr-lang/pydantic-sifr/issues/{issue}",
                guide,
            )

    def test_guide_does_not_introduce_a_versioned_or_legacy_api(self) -> None:
        guide = GUIDE.read_text(encoding="utf-8").lower()
        self.assertNotIn("v2", guide)
        self.assertNotIn("legacy", guide)
        self.assertNotIn("backward compatibility", guide)


if __name__ == "__main__":
    unittest.main()
