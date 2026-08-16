from __future__ import annotations

import ast
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
ORACLE = ROOT / "scripts/oracle/pydantic_differential.py"
NATIVE = ROOT / "backend/pydantic_sifr_core/examples/differential_probe.rs"
RUNNER = ROOT / "scripts/run_differential_validation.py"


class Ps11DifferentialTest(unittest.TestCase):
    def test_oracle_and_native_case_sets_are_identical(self) -> None:
        tree = ast.parse(ORACLE.read_text(encoding="utf-8"))
        assignment = next(
            node
            for node in tree.body
            if isinstance(node, ast.Assign)
            and any(isinstance(target, ast.Name) and target.id == "CASES" for target in node.targets)
        )
        assert isinstance(assignment.value, ast.Tuple)
        oracle_names = [ast.literal_eval(item.elts[0]) for item in assignment.value.elts]
        native_source = NATIVE.read_text(encoding="utf-8")
        native_names = re.findall(r'outcome\(\s*"([a-z_]+)"', native_source)
        self.assertEqual(native_names, oracle_names)
        self.assertEqual(len(oracle_names), 5)

    def test_runner_is_pinned_and_gated(self) -> None:
        runner = RUNNER.read_text(encoding="utf-8")
        gate = (ROOT / "scripts/run_all_tests.sh").read_text(encoding="utf-8")
        self.assertIn('PIN = "f59e929c999e8b2efc7b12fd0bc1685c1a186be3"', runner)
        self.assertIn('CORE_VERSION = "2.47.0"', runner)
        self.assertIn("native_value != oracle_value", runner)
        self.assertIn("python3 scripts/run_differential_validation.py", gate)


if __name__ == "__main__":
    unittest.main()
