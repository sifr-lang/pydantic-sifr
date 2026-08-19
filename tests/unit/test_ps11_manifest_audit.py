from __future__ import annotations

import re
import tomllib
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
PYDANTIC_PIN = "f59e929c999e8b2efc7b12fd0bc1685c1a186be3"
SIFR_PIN = "c1b41a6078cff5bb678bd94df5bbf4e8c7e0ec6c"


class Ps11ManifestAuditTest(unittest.TestCase):
    def test_provenance_ledgers_match_the_generator_pin(self) -> None:
        manifest = tomllib.loads(
            (ROOT / "tests/provenance/upstream_manifest.toml").read_text(
                encoding="utf-8"
            )
        )["meta"]
        kinds = tomllib.loads(
            (ROOT / "tests/provenance/core_schema_kinds.toml").read_text(
                encoding="utf-8"
            )
        )["meta"]
        self.assertEqual(manifest["pydantic_commit"], PYDANTIC_PIN)
        self.assertEqual(manifest["file_count"], 310)
        self.assertEqual(manifest["node_count"], 12754)
        self.assertEqual(kinds["pydantic_commit"], PYDANTIC_PIN)
        self.assertEqual(kinds["schema_kind_count"], 53)
        self.assertEqual(kinds["field_kind_count"], 4)
        for relative in (
            "scripts/provenance/generate_upstream_manifest.py",
            "scripts/provenance/generate_core_schema_kinds.py",
        ):
            source = (ROOT / relative).read_text(encoding="utf-8")
            pin = re.search(r'^PIN = "([0-9a-f]{40})"$', source, re.MULTILINE)
            self.assertIsNotNone(pin)
            assert pin is not None
            self.assertEqual(pin.group(1), PYDANTIC_PIN)

    def test_certification_names_pins_evidence_and_update_steps(self) -> None:
        guide = (ROOT / "docs/certification.md").read_text(encoding="utf-8")
        for marker in (
            PYDANTIC_PIN,
            SIFR_PIN,
            "4dfdfed840829f0fd439b42ebba859f22c9c491f8f7e62595fe2fc4f19fedf0e",
            "b221bebe7c78f5ac2eeac3c47e51f9097fc5ca068e25f4d3e7a380d243faff49",
            "generate_upstream_manifest.py",
            "generate_core_schema_kinds.py",
            "import_anchor_rules.py",
            "check_sifr_pin.py",
            "--check",
        ):
            self.assertIn(marker, guide)
        self.assertIn("No compatibility row is deferred to PS11", guide)


if __name__ == "__main__":
    unittest.main()
