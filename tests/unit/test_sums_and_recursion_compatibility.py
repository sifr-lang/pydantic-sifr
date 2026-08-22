from __future__ import annotations

import tomllib
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
LEDGER = ROOT / "tests/compatibility/sums_and_recursion.toml"
ANCHORS = ROOT / "tests/provenance/anchor_rules.toml"

REQUIRED = {
    "validators/control_composition",
    "validators/definitions_recursion",
    "validators/enum",
    "validators/literal",
    "validators/nullable_union",
    "validators/tagged_unions",
    "validators/unions",
    "core/recursion_limit",
    "core/smart_union_ranking",
}
UPSTREAM_REQUIRED = {
    family for family in REQUIRED if not family.startswith("core/")
}


class SumsAndRecursionCompatibilityTest(unittest.TestCase):
    def test_ledger_is_total_for_delivered_families(self) -> None:
        payload = tomllib.loads(LEDGER.read_text(encoding="utf-8"))
        self.assertEqual(payload["scope"], "sums_and_recursion")
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

    def test_delivered_families_have_upstream_anchors(self) -> None:
        anchors = tomllib.loads(ANCHORS.read_text(encoding="utf-8"))["anchor"]
        fixtures = {
            row["fixture"]
            for row in anchors
            if row["fixture"] in UPSTREAM_REQUIRED
        }
        self.assertTrue(UPSTREAM_REQUIRED.issubset(fixtures))


if __name__ == "__main__":
    unittest.main()
