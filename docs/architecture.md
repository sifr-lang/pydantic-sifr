# Package architecture

The package has two release-synchronized components.

- Sifr source owns deterministic schema derivation and static specialization.
- The `pydantic_sifr_core` Rust crate owns sealed-program access, input arenas,
  JSON parsing, validation, serialization plans, and JSON Schema generation.

## Static program boundary

`verify_schema` is a deterministic `@const_eval` function. It consumes the
compiler's canonical structural shape. It builds a compact, deterministic node
array. It checks node ranges, arity, definitions, references, and error
overrides. It returns the contract tuple, feature bitmap, shape identity, root,
and nodes.

`@const_specialize` runs that function in check, test, build, and editor modes.
It reports invalid schemas before runtime. The compiler seals the successful
result for one concrete type.

The native core accepts the compiler's sealed static program through
`PreparedSchema::from_static`. The compiler-owned envelope verifies contract
versions, program identity, and shape identity before the core reads a node.
The package has no second envelope type, older format, or runtime fallback.

The Sifr package root exports typed validation, serialization, and JSON Schema
errors; `verify_schema`; three validation functions; `model_dump_json`; and
`model_json_schema`. Schema payload classes and contract values are internal.

The round-trip gate emits a representative specialization with the released
Sifr compiler. It compares the exact bytes and identity with checked-in
fixtures. A Rust test accepts the same fixture through `PreparedSchema`.

## Input boundary

The JSON adapter uses jiter with `default-features = false` and only the
`num-bigint` feature. Production dependencies do not include Python or PyO3.
The adapter rejects non-finite numbers, duplicate object keys, malformed UTF-8,
and incomplete or trailing input.

The adapter checks input bytes, nesting depth, node count, and total string
bytes. It moves parsed values into a compact checked arena. Integer text is
preserved exactly. A user-controlled input path does not use `unwrap`,
`expect`, or an assertion.

The native structural adapter consumes compiler-generated visitation events.
It writes scalar values and aggregate edges directly into the same input
arena. It does not build a generic model value first. The adapter applies the
same depth, node, item, string, and integer limits. It sorts unordered mapping
and set projections before validation, which keeps aggregate error order
stable.

## Error boundary

Built-in errors have fixed codes, messages, and context keys. A custom code
must be package-qualified. A custom declaration must register one exact
message and context set. An override cannot change a built-in declaration.
Typed `Field` arguments carry these override values into the sealed schema.

Validator callback failures use the same structured error type. A failure
keeps its checked message in the public context map. Field callbacks add the
field location. Model callbacks add the model root location.

Malformed schemas return stable package diagnostics during specialization.
Malformed JSON returns typed runtime errors with stable codes and source
locations. Separate property and fuzz targets exercise JSON and prepared
schemas under explicit resource limits.

## Shared validation engine

The backend validates all scalar and collection schemas through one recursive
engine. `ValidationOptions` selects native, JSON, or strings input. The same
options carry the default strict mode, an optional call override, resource
limits, and one clock snapshot. A schema does not select a second runtime or a
fallback path.

Strictness and input-profile controls select one declared child. A call
override has authority over the default strict mode. A JSON call selects the
JSON child. Native and strings calls select the structural child. Both children
must produce the same structural type.

`Schema::chain` rejects an empty chain. It removes a one-step chain and flattens
nested chains. Each step validates the typed output of the previous step. A
checked arena handoff converts only that output into the next input arena. The
handoff preserves the original input profile and enforces the same limits. It
does not keep a third dynamic value tree.

A definition scope owns one exact identity-to-schema table. A reference must
match the structural identity and canonical kind of its target. A reference
cannot target a flattened wrapper or another definition scope. Flattened
wrappers include literals, nullables, unions, tagged unions, and embedded JSON.

Validation checks every definition before it accepts the scope. It resolves
references inside that scope. Fresh parsed inputs, defaults, mapping keys, and
lazy generator items keep the scope. Each fresh input starts a new recursion
trace.

Validation tracks active input and reference pairs. A repeated reference can
reuse its target. A repeated active pair returns `recursion_loop`. A finite
value that exceeds the depth limit returns `recursion_limit`.

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

## Typed construction boundary

`PreparedSchema` derives the structural shape identity once from the static
schema. A model schema stores the exact compiler-owned identity. It does not
copy the target identity at runtime. Typed construction accepts only this
prepared object. Construction fails before node access if the schema and target
identities differ.

Ordinary validation does not build structural edge arrays or move-state
tables. Typed construction prepares those views only after validation succeeds.
Specialized values add crate-neutral component nodes to the same validated
arena. They do not create another model tree.

`ModelSchema::new` rejects duplicate fields and invalid non-input fields before
the schema can validate user data. An allowed-extra destination must be one
declared, non-input mapping field. Its key type is `str`, and its value schema
must match the extra value schema.

The backend pins the Sifr structural runtime to exact commit
`0ab08c47605b694e396a4c30c621fd1ae79b07e1`. The package contains this Rust
source and its lockfile. The backend is not a separate crates.io product
because the Sifr runtime crates are not crates.io packages. This rule avoids a
duplicate private copy of the compiler-owned structural contract.

## Model API boundary

The Sifr package exports `model_validate`, `model_validate_json`, and
`model_validate_strings`. Each function returns the requested ordinary Sifr
class or a typed error. The JSON and strings functions accept bytes. The native
function accepts a separate structural input type.

Each call borrows one compiler-sealed static schema program. The Rust bridge
prepares a schema view over those static values. It does not parse or clone a
schema graph. Successful validation prepares structural construction over the
validated arena and moves the result into the target class.

Validator declarations become ordered callable identities in the sealed
program. Function-before, function-after, and function-plain nodes contain the
exact method-slot number. The bridge checks the slot-table identity before it
accepts input. It then invokes only the generated slot table. Typed callback
context stays borrowed for the validation call.

Applications can add thin class methods for familiar call syntax. Such a
method calls one of the exported functional entry points. It does not declare
another Rust bridge or own another schema. The PS6 demo validates one payload
through both forms and compares every constructed field. A source contract
test also requires the facade method to call the exported function and keeps
the production bridge set limited to the three functional entry points.

`ValidationError.message` contains one stable JSON object. The object contains
ordered details, typed locations, expected values, and the truncation fact.
The bridge escapes all text before it writes this object. A contained Rust
panic remains a distinct `RustPanicError`.

## PS5 compatibility ledger

`tests/compatibility/ps5.toml` classifies every required PS5 fixture family as
the same or adapted. Each row names executable local evidence and describes
every adapted semantic difference. A unit gate checks total coverage against
the pinned upstream anchor ledger.

## PS6 compatibility ledger

`tests/compatibility/ps6.toml` classifies each required PS6 fixture family.
The ledger covers models, fields, defaults, nullable fields, aliases,
configuration, constraints, structural input, and the public model API. A unit
gate checks total coverage against the pinned upstream anchor ledger.

## PS7 compatibility ledger

`tests/compatibility/ps7.toml` records each delivered PS7 family. The current
rows cover literals, enums, nullable and ordinary unions, field/path tagged
unions, definitions and recursion, strictness controls, input-profile controls,
typed chains, recursion limits, and smart-union ranking. Each adapted row states
the Sifr type and ownership rules. A unit gate binds each upstream-derived row
to the pinned anchor ledger. The Sifr-native recursion-limit and smart-ranking
families have direct local evidence and do not claim upstream anchors. Typed
callback discriminators, validator callbacks, and caller-owned context remain
assigned to the later PS7 callback waves. The PS7 validator API,
discriminated-union API, and generic-recursion API families also remain
pending.

## PS10 compatibility matrix

`tests/compatibility/ps10.toml` is the machine-readable source for the public
matrix in `docs/compatibility.md`. It distinguishes delivered adaptations from
compiler-blocked APIs. A blocked row must name an owning issue and cannot name
local evidence as though the API were available. The unit gate also binds the
selected PS10 upstream families to the pinned anchor ledger.
