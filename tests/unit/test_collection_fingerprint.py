from __future__ import annotations

import datetime as dt
import decimal
import sys
import types
import unittest

from scripts.provenance.collect_upstream import _canonical, _fingerprint, _source_hash


class FingerprintTests(unittest.TestCase):
    def test_mapping_order_does_not_change_fingerprint(self) -> None:
        left = _canonical(_fingerprint({"b": 2, "a": 1}))
        right = _canonical(_fingerprint({"a": 1, "b": 2}))
        self.assertEqual(left, right)

    def test_value_changes_do_change_fingerprint(self) -> None:
        self.assertNotEqual(
            _canonical(_fingerprint(decimal.Decimal("1.20"))),
            _canonical(_fingerprint(decimal.Decimal("1.2"))),
        )
        self.assertNotEqual(
            _canonical(_fingerprint(dt.timedelta(seconds=1))),
            _canonical(_fingerprint(dt.timedelta(seconds=2))),
        )

    def test_cycles_are_rejected(self) -> None:
        value: list[object] = []
        value.append(value)
        with self.assertRaises(TypeError):
            _fingerprint(value)

    def test_module_fingerprint_excludes_runtime_source(self) -> None:
        module = types.ModuleType("compiled_example")
        module.__file__ = "/different/platform/compiled_example.so"
        self.assertEqual(
            _fingerprint(module),
            ["module", "compiled_example"],
        )

    def test_runtime_symbol_fingerprint_excludes_interpreter_source(self) -> None:
        self.assertEqual(_source_hash(dt.datetime), "external-symbol")

    def test_runtime_import_path_excludes_environment_paths(self) -> None:
        self.assertEqual(_fingerprint(sys.path), ["runtime-import-path"])


if __name__ == "__main__":
    unittest.main()
