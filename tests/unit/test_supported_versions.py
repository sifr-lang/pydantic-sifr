from __future__ import annotations

import subprocess
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


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


if __name__ == "__main__":
    unittest.main()
