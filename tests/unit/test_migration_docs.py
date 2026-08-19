from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
GUIDE = ROOT / "docs/migration.md"


class MigrationDocsTest(unittest.TestCase):
    def test_guide_names_the_supported_migration_contract(self) -> None:
        guide = GUIDE.read_text(encoding="utf-8")
        required = {
            "BaseModel",
            "ConfigDict",
            "Field",
            "AliasPath",
            "model_validate_json",
            "model_validate_strings",
            "Result",
            "ValidationError",
            "RustPanicError",
            "compatibility matrix",
        }
        for marker in required:
            self.assertIn(marker, guide)
        self.assertNotIn("@const_specialize", guide)
        self.assertNotIn("@metadata", guide)

    def test_guide_names_the_remaining_blocked_surfaces(self) -> None:
        guide = GUIDE.read_text(encoding="utf-8")
        for surface in ("validator decorators", "serializer decorators", "computed fields"):
            self.assertIn(surface, guide)

    def test_guide_does_not_introduce_a_versioned_or_legacy_api(self) -> None:
        guide = GUIDE.read_text(encoding="utf-8").lower()
        self.assertNotIn("v2", guide)
        self.assertNotIn("legacy", guide)
        self.assertNotIn("backward compatibility", guide)


if __name__ == "__main__":
    unittest.main()
