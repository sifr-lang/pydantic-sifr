# pydantic-sifr

`pydantic-sifr` provides native, statically linked validation and
serialization for Sifr programs. The package uses a Sifr frontend and a Rust
backend. Released artifacts do not load Python, CPython, extension modules, or
runtime plugins.

The repository is under active construction. The package now has static model
schemas, one shared validation engine, typed model construction, and public
JSON, structural, and strings entry points.

The [compatibility matrix](docs/compatibility.md) lists every selected public
surface as same, adapted, or blocked. Blocked entries are not implemented by a
fallback or compatibility shim.

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

General controls can select a lax or strict child and a JSON or structural
child. An explicit strictness setting overrides the schema default. JSON calls
select the JSON child. Native and strings calls select the structural child.
Both children must return the same declared Sifr type.

Typed chains pass each validated result to the next step through checked
arenas. Construction flattens nested chains, removes a one-step chain, and
rejects an empty chain. The handoff keeps the original input profile and does
not create a persistent dynamic object tree.

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

## Model validation

The package exports `model_validate`, `model_validate_json`, and
`model_validate_strings`. These functions construct an ordinary Sifr class.
They use one compiler-sealed schema program and one checked validation arena.

Native structural input uses compiler-generated visitation. The adapter writes
directly into the input arena and does not create a generic model tree. The
adapter sorts unordered mappings and sets before validation, which keeps error
order stable.

The milestone demo is a dependent package in
`demos/milestone_ps_6_demo`. It shows nested models, defaults, constraints,
aliases, alias paths, JSON input, strings input, structural input, and a stable
public validation error.

`demos/milestone_ps_7_demo` shows literals, payload-free enums, smart unions,
field-discriminated tagged unions, recursive models, and
labelled branch errors through the same public API.

Sum declarations use package-owned metadata. Literal keys are
`pydantic.literal.none|bool|int|str|bytes`. Enum fields use the corresponding
`pydantic.enum.*` keys in variant order. Integer values are canonical decimal
strings. Byte values are hexadecimal strings. Ordinary unions accept repeated
`pydantic.union.label` entries, `pydantic.union.mode`, and
`pydantic.union.auto_collapse`. A tagged union declares a field/index path,
then pairs each `pydantic.discriminator.choice` with one or more typed
`pydantic.discriminator.tag.*` entries.

Union labels use the compiler's canonical member order, not annotation order.
Supply one label for each member in that canonical order.
Static `left_to_right` unions also select the first successful member in
canonical order.

## Development gates

Set `SIFR_BIN` to the exact required Sifr compiler:

```sh
export SIFR_BIN=/path/to/sifr
```

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

The gate uses Sifr implementation commit
`4f5492531e81385dd28efe25adfdd57dd678d2a9`. CI builds that exact compiler
source. The runtime manifests and lockfiles pin the same commit.
