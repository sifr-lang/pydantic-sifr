from __future__ import annotations

import tomllib
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
LEDGER = ROOT / "tests/compatibility/ps10.toml"
ANCHORS = ROOT / "tests/provenance/anchor_rules.toml"
PUBLIC_MATRIX = ROOT / "docs/compatibility.md"
STATUS_DOCUMENTS = (
    ROOT / "README.md",
    ROOT / "docs/quickstart.md",
    ROOT / "docs/migration.md",
)

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
    "excluded/assignment_validation",
    "excluded/create_model",
    "excluded/from_attributes",
    "excluded/frozen_models",
    "excluded/metaclasses",
    "excluded/mixed_adapter_providers",
    "excluded/model_construct",
    "excluded/model_copy_updates",
    "excluded/model_fields_rebuild",
    "excluded/multiple_data_inheritance",
    "excluded/private_attributes",
    "excluded/pydantic_dataclasses",
    "excluded/python_plugins",
    "excluded/runtime_schema",
    "excluded/runtime_types",
    "excluded/syntax_tree_macros",
    "excluded/unbound_generic_schema",
    "excluded/validate_call",
    "excluded/wildcard_field_validator",
    "excluded/wrap_handlers",
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
            self.assertIn(
                row["status"], {"same", "adapted", "blocked", "excluded"}
            )
            self.assertTrue(row["difference"].strip() or row["status"] == "same")
            if row["status"] == "blocked":
                self.assertEqual(row["evidence"], "")
                self.assertRegex(
                    row["blocker"],
                    r"^https://github\.com/sifr-lang/pydantic-sifr/issues/\d+$",
                )
            elif row["status"] == "excluded":
                self.assertEqual(row["evidence"], "")
                self.assertEqual(row["blocker"], "")
            else:
                self.assertTrue((ROOT / row["evidence"]).is_file())
                self.assertEqual(row["blocker"], "")

    def test_ps10_upstream_families_are_anchored(self) -> None:
        anchors = tomllib.loads(ANCHORS.read_text(encoding="utf-8"))["anchor"]
        fixtures = {row["fixture"] for row in anchors if row["milestone"] == "ps_10"}
        self.assertEqual(fixtures, PS10_ANCHORED)

    def test_public_matrix_matches_every_machine_surface_status(self) -> None:
        matrix = PUBLIC_MATRIX.read_text(encoding="utf-8")
        payload = tomllib.loads(LEDGER.read_text(encoding="utf-8"))
        documented: dict[str, tuple[str, str]] = {}
        for line in matrix.splitlines():
            cells = [cell.strip() for cell in line.strip().split("|")[1:-1]]
            if len(cells) != 5 or not cells[0].startswith("`"):
                continue
            documented[cells[0].strip("`")] = (cells[2], cells[4])
        expected = {row["name"]: row["status"] for row in payload["surface"]}
        self.assertEqual(
            {name: value[0] for name, value in documented.items()}, expected
        )

        serialization = next(
            row for row in payload["surface"] if row["name"] == "api/serialization"
        )
        self.assertEqual(
            documented["api/serialization"][1], f"`{serialization['evidence']}`"
        )

    def test_entry_docs_match_current_machine_status_set(self) -> None:
        payload = tomllib.loads(LEDGER.read_text(encoding="utf-8"))
        statuses = sorted({row["status"] for row in payload["surface"]})
        self.assertEqual(statuses, ["adapted", "excluded"])
        summary = "The current matrix uses `adapted` and `excluded` statuses."
        for path in STATUS_DOCUMENTS:
            document = " ".join(path.read_text(encoding="utf-8").split())
            self.assertIn(summary, document)


if __name__ == "__main__":
    unittest.main()
