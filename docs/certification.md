# Certification

This page records the inputs and procedures for the release certification.
The checks fail if a source revision or generated ledger changes.

## Pinned sources

The conformance oracle is Pydantic commit
`f59e929c999e8b2efc7b12fd0bc1685c1a186be3`. Its locked environment contains
Pydantic Core 2.47.0 and pytest 9.1.1.

The package compiler and runtime source is Sifr commit
`4f5492531e81385dd28efe25adfdd57dd678d2a9`.

The total-set manifest contains 310 tracked upstream files and 12,754 collected
test nodes. Its generated file SHA-256 is
`4dfdfed840829f0fd439b42ebba859f22c9c491f8f7e62595fe2fc4f19fedf0e`.
The Core Schema ledger contains 53 schema kinds and 4 field kinds. Its universe
SHA-256 is
`b221bebe7c78f5ac2eeac3c47e51f9097fc5ca068e25f4d3e7a380d243faff49`.
No compatibility row is deferred to PS11.

## Differential validation

`scripts/run_differential_validation.py` executes five shared cases against
the pinned Pydantic Core 2.47.0 environment and the native core. The cases
cover lax integer coercion, strict integer rejection, ordered string
transforms and constraints, list item coercion, and indexed list errors. The
gate compares canonical success values or stable error code and location. It
does not compare implementation-specific messages.

The oracle is a development and certification input only. Python and Pydantic
are absent from the production dependency graph.

## Robustness testing

The merge gate runs 4,096 property cases for the schema envelope, JSON input,
scalar validation, collection validation, and special-value validation suites.
Those suites include bounded-depth, bounded-size, malformed-input, and
panic-free properties.

The gate also compiles and executes six fuzz targets. Each target gets 1,000
bounded randomized inputs: JSON foundations, schema envelopes, scalar
validation, collection validation, special-value validation, and typed
construction. Seed corpora cover representative scalar, collection, and
special inputs. These are not sanitizer-guided fuzz campaigns.

## Update the Pydantic pin

Use one clean checkout at the proposed exact commit. Do not combine revisions.

1. Change `PIN` in both provenance generators.
2. Regenerate `tests/provenance/upstream_manifest.toml` with
   `scripts/provenance/generate_upstream_manifest.py`.
3. Re-audit every Core Schema disposition. Then update
   `tests/provenance/core_schema_kinds.toml` with the approved exact universe.
   Verify it with `scripts/provenance/generate_core_schema_kinds.py`.
4. Import changed anchor rules with
   `scripts/provenance/import_anchor_rules.py`. Do not retain a stale selector.
5. Run both provenance generators with `--check` against the same clean
   checkout.
6. Review every added, removed, or changed file, node, kind, and anchor. Assign
   each changed surface to executable evidence or an owning issue before merge.

The update is invalid if the checkout has tracked changes or its revision does
not equal both generator pins.

## Update the Sifr pin

Use one exact Sifr commit for the compiler and runtime.

1. Change `SIFR_REV` in `.github/workflows/ci.yml`.
2. Change the Sifr revision in both Cargo manifests.
3. Regenerate every affected lockfile.
4. Change the revision in `README.md`, `docs/architecture.md`, and
   `THIRD_PARTY_LICENSES.md`.
5. Run `python3 scripts/check_sifr_pin.py`.
6. Run the companion create-PR gate. Run the merge gate once on the exact
   reviewed candidate.

Do not add a compatibility pin, fallback compiler, or alternate runtime.
