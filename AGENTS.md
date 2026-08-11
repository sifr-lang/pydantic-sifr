# Repository instructions

`pydantic-sifr` is the native validation and serialization package for Sifr.

- Do not add Python, CPython, extension-module, dynamic-plugin, fallback, or
  compatibility paths to production code.
- User input must return typed errors. It must not cause a panic.
- Keep hand-maintained source files below 900 lines.
- Treat `tests/provenance/upstream_manifest.toml` and
  `tests/provenance/core_schema_kinds.toml` as generated, exact-set ledgers.
- Run `scripts/run_all_tests.sh --profile create-pr` before a pull request.
- Run `scripts/run_all_tests.sh` once for the final merge candidate.
- Use six Cargo build jobs on the shared development host.

