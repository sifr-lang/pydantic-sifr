from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
DEMOS = (
    "model_validation",
    "fields_and_configuration",
    "validators",
    "serializers_and_computed_fields",
)


class EndToEndDocsTest(unittest.TestCase):
    def test_each_documented_demo_is_a_mandatory_gate_app(self) -> None:
        readme = (ROOT / "README.md").read_text(encoding="utf-8")
        quickstart = (ROOT / "docs/quickstart.md").read_text(encoding="utf-8")
        gate = (ROOT / "scripts/run_all_tests.sh").read_text(encoding="utf-8")
        for demo in DEMOS:
            self.assertIn(f"demos/{demo}", readme)
            self.assertIn(f"demos/{demo}", quickstart)
            self.assertIn(f"cd demos/{demo}", gate)
            self.assertTrue((ROOT / f"demos/{demo}/README.md").is_file())
            self.assertTrue((ROOT / f"demos/{demo}/src/main.sifr").is_file())

        self.assertEqual(gate.count('"${sifr_bin}" run --locked'), len(DEMOS))

    def test_quickstart_uses_only_the_public_validation_surface(self) -> None:
        quickstart = (ROOT / "docs/quickstart.md").read_text(encoding="utf-8")
        for name in (
            "ValidationError",
            "SerializationError",
            "JsonSchemaError",
            "model_dump_json",
            "model_json_schema",
            "model_validate",
            "model_validate_json",
            "model_validate_strings",
            "BaseModel",
            "ConfigDict",
            "Field",
        ):
            self.assertIn(name, quickstart)
        self.assertNotIn("@const_specialize", quickstart)
        self.assertNotIn("@metadata", quickstart)
        self.assertNotIn("import pydantic\n", quickstart)


if __name__ == "__main__":
    unittest.main()
