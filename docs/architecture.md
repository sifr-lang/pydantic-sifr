# Foundation architecture

The package has two release-synchronized components.

- Sifr source owns deterministic schema derivation and static specialization.
- The `pydantic_sifr_core` Rust crate owns envelope checks, input arenas, JSON
  parsing, the runtime error registry, and execution-plan foundations.

## Static program boundary

`verify_schema` is a deterministic `@const_eval` function. It consumes the
compiler's canonical structural shape. It builds a compact, deterministic node
array. It checks node ranges, arity, definitions, references, and error
overrides. It returns the contract tuple, feature bitmap, shape identity, root,
and nodes.

`@const_specialize` runs that function in check, test, build, and editor modes.
It reports invalid schemas before runtime. The compiler seals the successful
result for one concrete type.

The native core accepts only `VerifiedSchemaProgram`. Its fields are private.
It compares all contract versions, the feature bitmap, program identity, shape
identity, and payload size with the generated bridge. It does not decode,
traverse, compile, or semantically verify the schema graph. No older format or
runtime fallback exists.

The round-trip gate emits a representative specialization with the released
Sifr compiler. It compares the exact bytes and identity with checked-in
fixtures. A Rust test accepts the same fixture through the envelope checker.

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

Malformed schemas return stable package diagnostics during specialization.
Malformed JSON returns typed runtime errors with stable codes and source
locations. Separate property and fuzz targets exercise JSON and program
envelopes under explicit resource limits.
