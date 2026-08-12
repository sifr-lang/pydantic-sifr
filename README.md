# pydantic-sifr

`pydantic-sifr` provides native, statically linked validation and
serialization for Sifr programs. The package uses a Sifr frontend and a Rust
backend. Released artifacts do not load Python, CPython, extension modules, or
runtime plugins.

The repository is under active construction. The package now has its static
schema contract and its shared scalar and collection validation engine.

## Foundation contract

Schema programs use format version 1. The Sifr frontend const function produces
one deterministic program record from the compiler's structural shape. The
compiler seals that record for the concrete target type. Runtime package code
cannot construct or change the sealed program.

The Sifr const function verifies node indices, schema kinds, references, and
error overrides. Invalid schemas fail during checking. The compiler then seals
the canonical bytes, program identity, and concrete shape identity.

The Python-free Rust backend checks only that sealed envelope before it accepts
input. It does not parse or verify the schema graph again. JSON parsing uses
jiter 0.16.0 without its Python feature. The parser stores exact integers as
decimal text in a checked move-owned arena. Syntax errors and resource limits
return typed errors.

## Validation engine

One engine validates native, JSON, and strings-profile inputs. Strict mode
controls native conversions. JSON strict mode still accepts the JSON form of a
declared scalar, such as a UUID string.

Exact integers, decimals, and fractions do not pass through a float. Fixed
integer targets report typed overflow errors. String processing has one fixed
order: conversion, trimming, ASCII checks, Unicode length, pattern matching,
and case conversion.

The engine validates lists, tuples, mappings, sets, and frozen sets. It keeps
their native input kinds distinct. `ValidatedIterator` validates each item
when the caller requests it. Its next item contains either a value or a stable
indexed error.

Temporal checks use one clock value supplied for the validation call. UUID,
URL, and regular expression values use Python-free Rust libraries. URL length
and scheme policies are explicit. JSON bytes can use UTF-8 or URL-safe base64.

Depth, item, string-byte, numeric-digit, decimal-exponent, and error-count
limits apply before unbounded work. The engine has no Python path, legacy
format, or runtime fallback.

## Development gates

Run the pull-request gate:

```sh
scripts/run_all_tests.sh --profile create-pr
```

Run the merge gate once on the final reviewed candidate:

```sh
scripts/run_all_tests.sh
```

The provenance gate uses Pydantic commit
`f59e929c999e8b2efc7b12fd0bc1685c1a186be3`. It collects the API and in-tree
Core suites in isolated pytest processes from that commit's root `uv.lock`.
The historical standalone Pydantic Core checkout is not a conformance source.

The gate requires the released `sifr 0.1.0-beta.16` compiler. Locked external
package checking currently has an upstream authority-policy defect tracked in
[sifr#3145](https://github.com/sifr-lang/sifr/issues/3145). The gate uses the
normal released-compiler package check. It does not change or bypass schema
behavior.
