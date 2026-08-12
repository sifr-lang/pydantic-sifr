#!/usr/bin/env bash

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${root}"

PROPTEST_CASES=4096 CARGO_BUILD_JOBS=6 \
  cargo test --release -p pydantic_sifr_core \
  --test schema_contract --test json_foundation
CARGO_BUILD_JOBS=6 \
  cargo check --manifest-path fuzz/Cargo.toml --bin json_foundation
CARGO_BUILD_JOBS=6 \
  cargo check --manifest-path fuzz/Cargo.toml --bin schema_envelope

echo "merge-only foundation tests passed"
