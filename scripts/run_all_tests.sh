#!/usr/bin/env bash

set -euo pipefail

profile="merge"
if [[ "${1:-}" == "--profile" ]]; then
  profile="${2:-}"
  shift 2
fi
if [[ $# -ne 0 || ! "${profile}" =~ ^(create-pr|merge)$ ]]; then
  echo "usage: scripts/run_all_tests.sh [--profile create-pr]" >&2
  exit 2
fi

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${root}"

python3 scripts/check_file_size.py
python3 scripts/check_sifr_pin.py
python3 -m unittest discover -s tests/unit -p 'test_*.py'

if [[ -d .upstream/pydantic ]]; then
  python3 scripts/provenance/generate_upstream_manifest.py \
    --upstream .upstream/pydantic --check
  python3 scripts/provenance/generate_core_schema_kinds.py \
    --upstream .upstream/pydantic --check
  python3 scripts/run_differential_validation.py
  python3 scripts/check_core_kind_binding.py
else
  echo "missing .upstream/pydantic; clone the pinned source before running gates" >&2
  exit 2
fi

sifr_bin="${SIFR_BIN:?set SIFR_BIN to the exact required Sifr compiler}"
"${sifr_bin}" --version
python3 scripts/check_supported_versions.py --sifr-bin "${sifr_bin}"
python3 scripts/check_sifr_union_order.py --sifr-bin "${sifr_bin}"
"${sifr_bin}" fmt --check src
"${sifr_bin}" check src/__init__.sifr
"${sifr_bin}" test src
python3 scripts/check_sifr_schema_failures.py --sifr-bin "${sifr_bin}"
python3 scripts/check_descriptor_failures.py --sifr-bin "${sifr_bin}"
python3 scripts/check_validator_failures.py --sifr-bin "${sifr_bin}"
python3 scripts/check_serializer_failures.py --sifr-bin "${sifr_bin}"
python3 scripts/check_model_operation_failures.py --sifr-bin "${sifr_bin}"
python3 scripts/check_static_program_roundtrip.py --sifr-bin "${sifr_bin}"
(
  cd demos/model_validation
  "${sifr_bin}" fetch --locked
  "${sifr_bin}" fmt --check src
  "${sifr_bin}" run --locked
)
(
  cd demos/fields_and_configuration
  "${sifr_bin}" fetch --locked
  "${sifr_bin}" fmt --check src
  "${sifr_bin}" run --locked
)
(
  cd demos/validators
  "${sifr_bin}" fetch --locked
  "${sifr_bin}" fmt --check src
  "${sifr_bin}" run --locked
)
(
  cd demos/serializers_and_computed_fields
  "${sifr_bin}" fetch --locked
  "${sifr_bin}" fmt --check src
  "${sifr_bin}" run --locked
)
python3 scripts/check_model_operation_identity.py --sifr-bin "${sifr_bin}"
python3 scripts/run_canonical_demo.py --sifr-bin "${sifr_bin}"

if [[ -f Cargo.toml ]]; then
  cargo fmt --check
  python3 scripts/check_python_free_graph.py
  CARGO_BUILD_JOBS=6 cargo test --workspace --all-targets
  CARGO_BUILD_JOBS=6 cargo clippy --workspace --all-targets -- -D warnings
fi

if [[ "${profile}" == "merge" && -x scripts/run_merge_only_tests.sh ]]; then
  scripts/run_merge_only_tests.sh
fi

echo "${profile} gate passed"
