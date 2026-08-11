# pydantic-sifr

`pydantic-sifr` provides native, statically linked validation and
serialization for Sifr programs. The package uses a Sifr frontend and a Rust
backend. Released artifacts do not load Python, CPython, extension modules, or
runtime plugins.

The repository is under active construction. Its first milestone establishes
the versioned schema-program contract, JSON foundation, typed errors, and
exact upstream provenance.

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

