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
python3 -m unittest discover -s tests/unit -p 'test_*.py'

if [[ -d .upstream/pydantic ]]; then
  python3 scripts/provenance/generate_upstream_manifest.py \
    --upstream .upstream/pydantic --check
  python3 scripts/provenance/generate_core_schema_kinds.py \
    --upstream .upstream/pydantic --check
  python3 scripts/check_core_kind_binding.py
else
  echo "missing .upstream/pydantic; clone the pinned source before running gates" >&2
  exit 2
fi

sifr_bin="${SIFR_BIN:-sifr}"
if [[ "$("${sifr_bin}" --version)" != "sifr 0.1.0-beta.16" ]]; then
  echo "PS4 requires released sifr 0.1.0-beta.16" >&2
  exit 2
fi
"${sifr_bin}" fmt --check src
"${sifr_bin}" check src/__init__.sifr
"${sifr_bin}" test src

if [[ -f Cargo.toml ]]; then
  cargo fmt --check
  if cargo tree -p pydantic_sifr_core --edges normal --format '{p}' \
    | grep -Eq '^(pyo3|pythonize) '; then
    echo "production dependency graph contains Python bindings" >&2
    exit 1
  fi
  CARGO_BUILD_JOBS=6 cargo test --workspace --all-targets
  CARGO_BUILD_JOBS=6 cargo clippy --workspace --all-targets -- -D warnings
fi

if [[ "${profile}" == "merge" && -x scripts/run_merge_only_tests.sh ]]; then
  scripts/run_merge_only_tests.sh
fi

echo "${profile} gate passed"
