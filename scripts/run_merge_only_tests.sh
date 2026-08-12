#!/usr/bin/env bash

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${root}"

PROPTEST_CASES=4096 CARGO_BUILD_JOBS=6 \
  cargo test --release -p pydantic_sifr_core \
  --test schema_contract --test json_foundation \
  --test validation_scalars --test validation_collections \
  --test validation_special
CARGO_BUILD_JOBS=6 \
  cargo check --manifest-path fuzz/Cargo.toml --bin json_foundation
CARGO_BUILD_JOBS=6 \
  cargo check --manifest-path fuzz/Cargo.toml --bin schema_envelope
CARGO_BUILD_JOBS=6 \
  cargo check --manifest-path fuzz/Cargo.toml --bin scalar_validation
CARGO_BUILD_JOBS=6 \
  cargo check --manifest-path fuzz/Cargo.toml --bin collection_validation
CARGO_BUILD_JOBS=6 \
  cargo check --manifest-path fuzz/Cargo.toml --bin special_validation
CARGO_BUILD_JOBS=6 \
  cargo check --manifest-path fuzz/Cargo.toml --bin typed_construction
CARGO_BUILD_JOBS=6 \
  cargo run --manifest-path fuzz/Cargo.toml --bin scalar_validation -- \
  -seed_inputs=fuzz/corpus/scalar_validation/integer.json,fuzz/corpus/scalar_validation/string.json \
  -runs=1000
CARGO_BUILD_JOBS=6 \
  cargo run --manifest-path fuzz/Cargo.toml --bin collection_validation -- \
  -seed_inputs=fuzz/corpus/collection_validation/list.json,fuzz/corpus/collection_validation/object.json \
  -runs=1000
CARGO_BUILD_JOBS=6 \
  cargo run --manifest-path fuzz/Cargo.toml --bin special_validation -- \
  -seed_inputs=fuzz/corpus/special_validation/date.json,fuzz/corpus/special_validation/url.json \
  -runs=1000
CARGO_BUILD_JOBS=6 \
  cargo run --manifest-path fuzz/Cargo.toml --bin typed_construction -- \
  -runs=1000

echo "merge-only foundation tests passed"
