# pydantic-sifr

`pydantic-sifr` provides native, statically linked validation and
serialization for Sifr programs. The package uses a Sifr frontend and a Rust
backend. Released artifacts do not load Python, CPython, extension modules, or
runtime plugins.

The repository is under active construction. Its first milestone establishes
the versioned schema-program contract, JSON foundation, typed errors, and
exact upstream provenance.

## Foundation contract

Schema programs use format version 1. The Sifr frontend const function produces
one deterministic program record from the compiler's structural shape. The
compiler seals that record for the concrete target type. Runtime package code
cannot construct or change the sealed program.

The Python-free Rust backend checks the contract tuple, compact node indices,
schema kinds, definition references, error overrides, and the canonical
payload hash before it accepts input. JSON parsing uses jiter 0.16.0 without
its Python feature. The parser stores exact integers as decimal text in a
checked move-owned arena. Syntax errors and resource limits return typed
errors.

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
