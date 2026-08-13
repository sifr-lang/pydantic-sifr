from __future__ import annotations

import tomllib
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
LEDGER = ROOT / "tests/compatibility/ps6.toml"
ANCHORS = ROOT / "tests/provenance/anchor_rules.toml"

REQUIRED = {
    "api/aliases",
    "api/base_model",
    "api/config_fields",
    "api/constraints",
    "core/json_models",
    "core/model_error_locations",
    "core/strings_profile_models",
    "core/structural_models",
    "validators/dataclasses",
    "validators/defaults",
    "validators/model_fields",
    "validators/nullable",
    "validators/typed_dict",
}


class Ps6CompatibilityTest(unittest.TestCase):
    def test_ledger_is_total_and_has_existing_evidence(self) -> None:
        payload = tomllib.loads(LEDGER.read_text(encoding="utf-8"))
        self.assertEqual(payload["milestone"], "ps_6")
        rows = payload["family"]
        names = [row["name"] for row in rows]
        self.assertEqual(set(names), REQUIRED)
        self.assertEqual(len(names), len(set(names)))
        for row in rows:
            self.assertIn(row["class"], {"same", "adapted"})
            self.assertTrue((ROOT / row["evidence"]).is_file())
            if row["class"] == "adapted":
                self.assertTrue(row["difference"].strip())
            else:
                self.assertEqual(row["difference"], "")

    def test_every_ps6_upstream_anchor_maps_to_a_ledger_family(self) -> None:
        anchors = tomllib.loads(ANCHORS.read_text(encoding="utf-8"))["anchor"]
        fixtures = {
            row["fixture"] for row in anchors if row["milestone"] == "ps_6"
        }
        self.assertTrue(fixtures)
        self.assertTrue(fixtures.issubset(REQUIRED))


if __name__ == "__main__":
    unittest.main()
