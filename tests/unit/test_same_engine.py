from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
DEMO = ROOT / "demos/model_validation/src/main.sifr"


class SameEngineTest(unittest.TestCase):
    def test_production_api_has_only_selected_bridges(self) -> None:
        sources = "\n".join(
            path.read_text(encoding="utf-8")
            for path in sorted((ROOT / "src").rglob("*.sifr"))
        )
        bridges = re.findall(r"@rust\(([^)]+)\)", sources)
        self.assertCountEqual(
            bridges,
            [
                "bridge.model.json_schema",
                "bridge.model.json_schema",
                "bridge.model.validate",
                "bridge.model.validate_json",
                "bridge.model.validate_strings",
                "bridge.model.validate_with_validators",
                "bridge.model.validate_json_with_validators",
                "bridge.model.validate_strings_with_validators",
                "bridge.model.dump_json",
                "bridge.model.dump_json_with_serializers",
                "bridge.model.dump",
                "bridge.model.dump_with_serializers",
                "bridge.special_values.url_text",
                "bridge.special_values.multi_host_url_text",
                "bridge.special_values.pattern_source",
                "bridge.special_values.pattern_flags",
            ],
        )

    def test_model_demo_uses_the_attached_validation_surface(self) -> None:
        source = DEMO.read_text(encoding="utf-8")
        self.assertIn("User.model_validate_json(payload)", source)
        self.assertIn("User.model_validate_strings(strings_input)", source)
        self.assertIn("User.model_validate(", source)
        self.assertNotIn("@classmethod", source)
        self.assertNotIn("model_validate as", source)

    def test_demo_checks_all_three_input_profiles(self) -> None:
        source = DEMO.read_text(encoding="utf-8")
        self.assertIn("user: User = User.model_validate_json(payload)", source)
        self.assertIn("strings: User = User.model_validate_strings(strings_input)", source)
        self.assertIn("structural: User = User.model_validate(", source)


if __name__ == "__main__":
    unittest.main()
