# Foundation architecture

The package has two release-synchronized components.

- Sifr source owns deterministic schema derivation and static specialization.
- The `pydantic_sifr_core` Rust crate owns verified program loading, input
  arenas, JSON parsing, error registries, and execution-plan foundations.

## Static program boundary

`verify_schema` is a deterministic `@const_eval` function. It consumes the
compiler's canonical structural shape. It returns the current contract tuple,
feature bitmap, and canonical shape identity. `@const_specialize` causes the
compiler to seal that value for one concrete type in check, test, build, and
editor modes.

The native core accepts only `VerifiedSchemaProgram`. Its fields are private.
The loader checks format version 1, all other contract versions, the shape
identity, the feature bitmap, node indices, node arity, definition identities,
error declarations, direct cycles, and the SHA-256 payload identity. It does
not interpret an older format.

## Input boundary

The JSON adapter uses jiter with `default-features = false` and only the
`num-bigint` feature. Production dependencies do not include Python or PyO3.
The adapter rejects non-finite numbers, duplicate object keys, malformed UTF-8,
and incomplete or trailing input.

The adapter checks input bytes, nesting depth, node count, and total string
bytes. It moves parsed values into a compact checked arena. Integer text is
preserved exactly. A user-controlled input path does not use `unwrap`,
`expect`, or an assertion.

## Error boundary

Built-in errors have fixed codes, messages, and context keys. A custom code
must be package-qualified. A custom declaration must register one exact
message and context set. An override cannot change a built-in declaration.

Malformed schemas and JSON return typed errors with stable codes and source
locations. Property tests and the fuzz target exercise arbitrary bytes under
explicit resource limits.
