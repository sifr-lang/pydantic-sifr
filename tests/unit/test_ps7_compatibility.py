from __future__ import annotations

import tomllib
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
LEDGER = ROOT / "tests/compatibility/ps7.toml"
ANCHORS = ROOT / "tests/provenance/anchor_rules.toml"

REQUIRED = {"validators/control_composition"}


class Ps7CompatibilityTest(unittest.TestCase):
    def test_ledger_is_total_for_delivered_families(self) -> None:
        payload = tomllib.loads(LEDGER.read_text(encoding="utf-8"))
        self.assertEqual(payload["milestone"], "ps_7")
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

    def test_delivered_families_have_ps7_upstream_anchors(self) -> None:
        anchors = tomllib.loads(ANCHORS.read_text(encoding="utf-8"))["anchor"]
        fixtures = {
            row["fixture"] for row in anchors if row["milestone"] == "ps_7"
        }
        self.assertTrue(REQUIRED.issubset(fixtures))


if __name__ == "__main__":
    unittest.main()
