from __future__ import annotations

import subprocess
import sys
import tomllib
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
LEDGER = ROOT / "tests/certification/supported_versions.toml"
GATE = ROOT / "scripts/run_all_tests.sh"
GUIDE = ROOT / "docs/certification.md"


class SupportedVersionsTest(unittest.TestCase):
    def test_certified_tuple_matches_every_manifest(self) -> None:
        result = subprocess.run(
            [sys.executable, "scripts/check_supported_versions.py", "--manifest-only"],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        )
        self.assertIn("supported version certification passed", result.stdout)

    def test_gate_and_guide_are_bound_to_the_certified_tuple(self) -> None:
        gate = GATE.read_text(encoding="utf-8")
        self.assertIn(
            'python3 scripts/check_supported_versions.py --sifr-bin "${sifr_bin}"',
            gate,
        )

        document = tomllib.loads(LEDGER.read_text(encoding="utf-8"))
        self.assertEqual(document["schema"], 1)
        self.assertEqual(len(document["combination"]), 1)
        guide = GUIDE.read_text(encoding="utf-8")
        for key, value in document["combination"][0].items():
            if key == "status":
                continue
            self.assertIn(f"`{value}`", guide)


if __name__ == "__main__":
    unittest.main()
