from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
BENCHMARK = ROOT / "backend/pydantic_sifr_core/benches/foundations.rs"
REPORT = ROOT / "docs/benchmarks.md"
OPERATIONS = ("parse", "validate", "construct", "serialize")


class BenchmarkTest(unittest.TestCase):
    def test_each_required_operation_has_a_published_result(self) -> None:
        source = BENCHMARK.read_text(encoding="utf-8")
        report = REPORT.read_text(encoding="utf-8")
        for operation in OPERATIONS:
            benchmark = f'"{operation}/representative_'
            self.assertIn(benchmark, source)
            self.assertIn(f"| `{operation}/", report)

        self.assertIn("cargo bench -p pydantic_sifr_core --bench foundations", report)
        self.assertIn("measured implementation", report)


if __name__ == "__main__":
    unittest.main()
