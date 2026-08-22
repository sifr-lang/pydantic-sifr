# Pydantic compatibility

This matrix describes the supported `pydantic-sifr` surface. It is pinned to
Pydantic commit `f59e929c999e8b2efc7b12fd0bc1685c1a186be3`.

Status meanings:

- **same**: the selected behavior has the same observable contract.
- **adapted**: the behavior is available with the stated Sifr-specific API.
- **blocked**: the public API is not available. The linked issue names the
  package-neutral compiler capability that is required.
- **excluded**: the public API is intentionally unavailable. The terminal
  reason does not depend on a later milestone.

Blocked and excluded entries do not have a fallback, compatibility shim, or
second runtime.

| API key | Surface | Status | Sifr contract | Evidence or blocker |
| --- | --- | --- | --- | --- |
| `api/base_model` | Model validation | adapted | Models derive from `BaseModel`. Attached validation methods return `Result` and construct the declared Sifr class. | `src/declarations.sifr`; `src/api.sifr`; `demos/model_validation` |
| `api/field_configuration` | `Field` and model configuration | adapted | Typed `Field`, `ConfigDict`, and `Constraints` descriptors define aliases, constraints, defaults, extras, and schema annotations. | `src/declarations.sifr`; `src/schema_contract.sifr`; `demos/fields_and_configuration` |
| `api/validators` | Validator declarations | adapted | `field_validator` supports before, after, and plain modes with explicit targets. `model_validator` supports before and after modes. Before handlers declare one static input type. Wrap mode and wildcard targets are excluded. | `src/declarations.sifr`; `src/schema_contract.sifr`; `demos/validators` |
| `api/serialization` | Serializer and computed-field declarations | adapted | Checked field and model serializers support four `when_used` policies. Checked computed fields add serialization-only output. Attached dump methods use the same sealed static program and typed context. | `demos/serializers_and_computed_fields/src/main.sifr` |
| `api/type_adapter` | `TypeAdapter[T]` | adapted | `TypeAdapter[T]` is a transparent type alias. Its attached validation, dump, and JSON Schema methods use the same sealed program as `T`. | `src/__init__.sifr`; `src/api.sifr`; `demos/model_validation` |
| `api/root_model` | Root values | adapted | `RootModel[T]` declares one stored `root` field. Attached validation accepts the root value. Dump and JSON Schema operations expose the root field schema. | `src/declarations.sifr`; `backend/pydantic_sifr_core/tests/typed_model_construction.rs`; `demos/model_validation` |
| `api/networks` | Network values | adapted | `Url` validates and constructs a mapped nominal Sifr value. | `src/special_values.sifr`; `src/bridges/special_values.rs` |
| `core/multi_host_url_serialization` | Multi-host URL serialization | adapted | `MultiHostUrl` validates and constructs a mapped nominal Sifr value. | `src/special_values.sifr`; `src/bridges/special_values.rs` |
| `api/pattern` | Compiled patterns | adapted | `Pattern` validates a bounded pattern and exposes its source and flags. | `src/special_values.sifr`; `src/bridges/special_values.rs` |
| `api/field_metadata` | Field metadata | adapted | Typed descriptors define constraints, aliases, checked error overrides, and bounded JSON Schema annotations. | `src/declarations.sifr`; `src/schema_contract.sifr` |
| `api/errors` | Validation errors | adapted | Validation failures are typed `Result` errors with stable codes, ordered locations, context, expected values, and truncation state. | `backend/pydantic_sifr_core/tests/validation_models.rs`; `src/errors.sifr` |
| `api/json_schema` | JSON Schema | adapted | The attached `model_json_schema` type method emits Draft 2020-12 from the sealed static schema. It accepts mode, alias, and integer-profile options without a dummy model value. | `src/api.sifr`; `backend/pydantic_sifr_core/tests/json_schema_dialect.rs`; `demos/model_validation` |
| `excluded/metaclasses` | Python metaclasses and runtime class mutation | excluded | Classes and schema programs are finalized statically. Runtime mutation would invalidate checked layout and identity. | Terminal exclusion |
| `excluded/create_model` | Dynamic `create_model` | excluded | Runtime type and schema creation conflicts with concrete compile-time type identity and sealed programs. | Terminal exclusion |
| `excluded/syntax_tree_macros` | Arbitrary syntax-tree macros | excluded | Packages can use typed declarations and bounded plans. They cannot rewrite language semantics. | Terminal exclusion |
| `excluded/runtime_schema` | Runtime schema construction | excluded | The build-time canonicalizer and sealed static program are the only schema authority. | Terminal exclusion |
| `excluded/python_plugins` | Python plugins and custom Core Schema hooks | excluded | Released binaries contain no Python runtime. Open hooks would bypass the checked schema contract. | Terminal exclusion |
| `excluded/pydantic_dataclasses` | Pydantic dataclasses | excluded | Ordinary Sifr classes replace dataclass discovery and generated initialization. | Terminal exclusion |
| `excluded/private_attributes` | Private attributes | excluded | Models cannot add hidden storage outside their declared structural layout. | Terminal exclusion |
| `excluded/validate_call` | `validate_call` | excluded | Arbitrary function-call interception requires a separate function-adaptation mechanism. | Terminal exclusion |
| `excluded/model_construct` | `model_construct` | excluded | Construction cannot bypass the sealed validate-and-construct boundary. Use an ordinary Sifr constructor for trusted typed values. | Terminal exclusion |
| `excluded/model_copy_updates` | Dynamic `model_copy` updates | excluded | Construct a new Sifr value. Use explicit cloning first when the value supports cloning. | Terminal exclusion |
| `excluded/model_fields_rebuild` | Runtime `model_fields` and `model_rebuild` | excluded | Runtime reflection and schema rebuilding conflict with immutable compile-time shapes and programs. | Terminal exclusion |
| `excluded/from_attributes` | ORM `from_attributes` | excluded | Arbitrary attribute probing is outside the typed structural-input contract. | Terminal exclusion |
| `excluded/runtime_types` | Arbitrary runtime types | excluded | Every value needs a statically checked type and a structural or declared nominal mapping. | Terminal exclusion |
| `excluded/multiple_data_inheritance` | Multiple data inheritance | excluded | One data parent preserves deterministic layout, construction, and field identity. | Terminal exclusion |
| `excluded/mixed_adapter_providers` | Mixed class-adapter providers | excluded | Multiple providers would create ambiguous declaration, ordering, and cache authorities. | Terminal exclusion |
| `excluded/assignment_validation` | Assignment-validation interception | excluded | Field mutation keeps ordinary Sifr assignment and ownership semantics. | Terminal exclusion |
| `excluded/frozen_models` | Python-compatible frozen models | excluded | Use ordinary Sifr immutability and ownership contracts instead of a runtime flag. | Terminal exclusion |
| `excluded/wrap_handlers` | Public wrap-handler continuations | excluded | A public continuation needs ownership, lifetime, and effect contracts that this phase does not add. | Terminal exclusion |
| `excluded/wildcard_field_validator` | Wildcard `field_validator("*")` targets | excluded | Explicit field identities keep checking, diagnostics, inheritance, and ordering deterministic. | Terminal exclusion |
| `excluded/unbound_generic_schema` | Schema generation for an unbound generic model | excluded | A schema program requires a concrete owner type, substituted fields, and a complete cache identity. | Terminal exclusion |

The machine-readable source for this table is
`tests/compatibility/public_api.toml`. The feature ledgers remain the detailed
fixture-family record for delivered validation and serialization behavior.
