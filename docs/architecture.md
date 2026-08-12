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

## Shared validation engine

The backend validates all scalar and collection schemas through one recursive
engine. `ValidationOptions` selects native, JSON, or strings input. The same
options carry strict mode, resource limits, and one clock snapshot. A schema
does not select a second runtime or a fallback path.

The scalar layer keeps exact integers as `BigInt`, decimals as `BigDecimal`,
and fractions as normalized `BigRational` values. Fixed integer schemas check
their declared signed or unsigned range. Complex values use two finite `f64`
components unless the schema allows non-finite components.

String validation applies conversion, trimming, ASCII policy, Unicode length,
pattern matching, and case conversion in that order. Pattern compilation uses
bounded Rust regex settings. Bytes use their exact native value. JSON text can
use explicit UTF-8 or URL-safe base64 decoding.

Temporal values preserve their components and declared offset. Past and future
checks compare with the clock snapshot from the validation call. UUID schemas
can require a version. URL schemas can limit source length and allowed schemes
before they return one canonical absolute URL.

The collection layer keeps lists, tuples, sets, frozen sets, objects, and
generic mappings distinct at the native boundary. It validates children into
one move-owned output arena. It records stable field, item, and mapping-key
locations. Aggregate errors stop at the configured limit and mark truncation.

`ValidatedIterator` keeps the input arena borrowed. It validates one item for
each call to `next`. A deferred failure includes the original item index. The
iterator applies length and resource checks without collecting values in
silence.

## PS5 compatibility ledger

`tests/compatibility/ps5.toml` classifies every required PS5 fixture family as
the same or adapted. Each row names executable local evidence and describes
every adapted semantic difference. A unit gate checks total coverage against
the pinned upstream anchor ledger.
