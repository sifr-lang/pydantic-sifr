from __future__ import annotations

import datetime as dt
import decimal
import unittest

from scripts.provenance.collect_upstream import _canonical, _fingerprint


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


if __name__ == "__main__":
    unittest.main()

