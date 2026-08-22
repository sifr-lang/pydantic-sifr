from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
DEMO = ROOT / "demos/model_validation/src/main.sifr"


class CanonicalDemoTest(unittest.TestCase):
    def test_canonical_demo_and_snapshot_are_mandatory(self) -> None:
        gate = (ROOT / "scripts/run_all_tests.sh").read_text(encoding="utf-8")
        runner = (ROOT / "scripts/run_canonical_demo.py").read_text(encoding="utf-8")
        self.assertTrue(DEMO.is_file())
        self.assertIn(
            'python3 scripts/run_canonical_demo.py --sifr-bin "${sifr_bin}"',
            gate,
        )
        self.assertIn("tests/snapshots/model_validation.stdout", runner)
        self.assertIn("shutil.copy2(DEMO, project / \"src/main.sifr\")", runner)

    def test_demo_uses_the_public_api_and_checks_success_and_failure(self) -> None:
        source = DEMO.read_text(encoding="utf-8")
        for marker in (
            "BaseModel,",
            "ConfigDict,",
            "Field,",
            "JsonSchemaError,",
            "ValidationError,",
            "class User(BaseModel):",
            'model_config = ConfigDict(extra="forbid")',
            'Field(alias="user_id", gt=0)',
            "User.model_validate_json(",
            "user.model_dump_json()",
            "User.model_json_schema()",
            "assert user.active",
            "except ValidationError as error:",
            "assert error_seen",
        ):
            self.assertIn(marker, source)
        self.assertNotIn("@const_specialize", source)
        self.assertNotIn("@metadata", source)


if __name__ == "__main__":
    unittest.main()
