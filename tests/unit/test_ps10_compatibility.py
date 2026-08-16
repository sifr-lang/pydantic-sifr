from __future__ import annotations

import tomllib
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
LEDGER = ROOT / "tests/compatibility/ps10.toml"
ANCHORS = ROOT / "tests/provenance/anchor_rules.toml"
PUBLIC_MATRIX = ROOT / "docs/compatibility.md"

REQUIRED = {
    "api/base_model",
    "api/errors",
    "api/field_configuration",
    "api/field_metadata",
    "api/json_schema",
    "api/networks",
    "api/pattern",
    "api/root_model",
    "api/serialization",
    "api/type_adapter",
    "api/validators",
    "core/multi_host_url_serialization",
}
PS10_ANCHORED = {
    "api/field_metadata",
    "api/networks",
    "api/pattern",
    "api/root_model",
    "core/multi_host_url_serialization",
}


class Ps10CompatibilityTest(unittest.TestCase):
    def test_machine_matrix_is_total_and_evidence_backed(self) -> None:
        payload = tomllib.loads(LEDGER.read_text(encoding="utf-8"))
        self.assertEqual(payload["milestone"], "ps_10")
        rows = payload["surface"]
        names = [row["name"] for row in rows]
        self.assertEqual(set(names), REQUIRED)
        self.assertEqual(len(names), len(set(names)))

        for row in rows:
            self.assertIn(row["status"], {"same", "adapted", "blocked"})
            self.assertTrue(row["difference"].strip() or row["status"] == "same")
            if row["status"] == "blocked":
                self.assertEqual(row["evidence"], "")
                self.assertRegex(
                    row["blocker"],
                    r"^https://github\.com/sifr-lang/pydantic-sifr/issues/\d+$",
                )
            else:
                self.assertTrue((ROOT / row["evidence"]).is_file())
                self.assertEqual(row["blocker"], "")

    def test_ps10_upstream_families_are_anchored(self) -> None:
        anchors = tomllib.loads(ANCHORS.read_text(encoding="utf-8"))["anchor"]
        fixtures = {row["fixture"] for row in anchors if row["milestone"] == "ps_10"}
        self.assertEqual(fixtures, PS10_ANCHORED)

    def test_public_matrix_names_every_machine_surface(self) -> None:
        matrix = PUBLIC_MATRIX.read_text(encoding="utf-8")
        payload = tomllib.loads(LEDGER.read_text(encoding="utf-8"))
        for row in payload["surface"]:
            self.assertIn(f"`{row['name']}`", matrix)


if __name__ == "__main__":
    unittest.main()
