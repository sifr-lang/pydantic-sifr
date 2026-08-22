from __future__ import annotations

import re
import tomllib
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MERGE_GATE = ROOT / "scripts/run_merge_only_tests.sh"
EXPECTED_TARGETS = {
    "json_foundation",
    "prepared_schema",
    "scalar_validation",
    "collection_validation",
    "special_validation",
    "typed_construction",
}


class RobustnessTest(unittest.TestCase):
    def test_every_fuzz_target_is_compiled_and_executed(self) -> None:
        gate = MERGE_GATE.read_text(encoding="utf-8")
        manifest = tomllib.loads((ROOT / "fuzz/Cargo.toml").read_text(encoding="utf-8"))
        declared = {item["name"] for item in manifest["bin"]}
        source_targets = {
            path.stem for path in (ROOT / "fuzz/fuzz_targets").glob("*.rs")
        }
        self.assertEqual(declared, source_targets)
        self.assertEqual(declared, EXPECTED_TARGETS)
        checked = set(re.findall(r"cargo check .* --bin ([a-z_]+)", gate))
        executed = set(re.findall(r"cargo run .* --bin ([a-z_]+)", gate))
        self.assertEqual(checked, declared)
        self.assertEqual(executed, declared)
        self.assertEqual(gate.count("-runs=1000"), len(declared))

    def test_property_and_resource_guards_are_mandatory(self) -> None:
        gate = MERGE_GATE.read_text(encoding="utf-8")
        self.assertIn("PROPTEST_CASES=4096", gate)
        test_sources = "\n".join(
            path.read_text(encoding="utf-8")
            for path in sorted((ROOT / "backend/pydantic_sifr_core/tests").glob("*.rs"))
        )
        for marker in (
            "arbitrary_json_bytes_never_panic",
            "arbitrary_prepared_schema_fields_never_panic",
            "arbitrary_collection_json_never_panics",
            "arbitrary_scalar_json_never_panics",
            "arbitrary_special_json_never_panics",
            "input_limit_exceeded",
            "resource_limit",
            "recursion_limit",
        ):
            self.assertIn(marker, test_sources)


if __name__ == "__main__":
    unittest.main()
