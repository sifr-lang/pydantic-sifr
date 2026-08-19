from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
DEMO = ROOT / "demos/milestone_ps_6_demo/src/main.sifr"


class SameEngineTest(unittest.TestCase):
    def test_production_api_has_only_selected_bridges(self) -> None:
        sources = "\n".join(
            path.read_text(encoding="utf-8")
            for path in sorted((ROOT / "src").rglob("*.sifr"))
        )
        bridges = re.findall(r"@rust\(([^)]+)\)", sources)
        self.assertEqual(
            bridges,
            [
                "bridge.model.validate",
                "bridge.model.validate_json",
                "bridge.model.validate_strings",
                "bridge.model.dump_json",
                "bridge.model.json_schema",
                "bridge.special_values.url_text",
                "bridge.special_values.multi_host_url_text",
                "bridge.special_values.pattern_source",
                "bridge.special_values.pattern_flags",
            ],
        )

    def test_each_class_facade_delegates_to_its_exported_function(self) -> None:
        source = DEMO.read_text(encoding="utf-8")
        user_class = source[
            source.index("class User(BaseModel):") : source.index("class PostalInput:")
        ]
        methods = re.findall(
            r"    @classmethod\n    def (model_validate_[a-z_]+)\(.*?"
            r"(?=\n    @classmethod|\n\nclass |\Z)",
            user_class,
            re.DOTALL,
        )
        self.assertEqual(methods, ["model_validate_json", "model_validate_strings"])
        for method_name in methods:
            function_name = method_name.replace("model_validate", "validate_model", 1)
            method_start = user_class.index(f"    def {method_name}(")
            next_method = user_class.find("\n    @classmethod", method_start)
            method_end = next_method if next_method >= 0 else len(user_class)
            method = user_class[method_start:method_end]
            self.assertIn(f"value: User = {function_name}(payload)", method)
            self.assertNotIn("@rust(", method)

    def test_demo_compares_functional_and_facade_results(self) -> None:
        source = DEMO.read_text(encoding="utf-8")
        self.assertIn("user: User = User.model_validate_json(payload)", source)
        self.assertIn("functional_user: User = validate_model_json(payload)", source)
        compared_fields = {
            "id",
            "name",
            "active",
            "address.city",
            "address.postal_code",
        }
        for field in compared_fields:
            self.assertIn(f"functional_user.{field} == user.{field}", source)
            self.assertIn(f"functional_strings.{field} == strings.{field}", source)


if __name__ == "__main__":
    unittest.main()
