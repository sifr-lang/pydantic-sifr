# Pydantic compatibility

This matrix describes the supported `pydantic-sifr` surface. It is pinned to
Pydantic commit `f59e929c999e8b2efc7b12fd0bc1685c1a186be3`.

Status meanings:

- **same**: the selected behavior has the same observable contract.
- **adapted**: the behavior is available with the stated Sifr-specific API.
- **blocked**: the public API is not available. The linked issue names the
  package-neutral compiler capability that is required.

Blocked entries do not have a fallback, compatibility shim, or second runtime.

| API key | Surface | Status | Sifr contract | Evidence or blocker |
| --- | --- | --- | --- | --- |
| `api/base_model` | Model validation | adapted | Models derive from `BaseModel`. Functional validation calls return `Result` and construct the declared Sifr class. | `src/declarations.sifr`; `src/model.sifr`; `demos/milestone_m8_fields_configuration` |
| `api/field_configuration` | `Field` and model configuration | adapted | Typed `Field`, `ConfigDict`, and `Constraints` descriptors define aliases, constraints, defaults, extras, and schema annotations. | `src/declarations.sifr`; `src/schema_contract.sifr`; `demos/milestone_m8_fields_configuration` |
| `api/validators` | Validator declarations | blocked | Before, after, plain, and wrap callbacks require typed handler-bearing method slots. | [#10](https://github.com/sifr-lang/pydantic-sifr/issues/10) |
| `api/serialization` | Serializer and computed-field declarations | blocked | Custom serialization and computed accessors require typed handler-bearing method slots. | [#14](https://github.com/sifr-lang/pydantic-sifr/issues/14) |
| `api/type_adapter` | `TypeAdapter[T]` | adapted | Sifr uses target-inferred validation, `model_dump_json`, and `model_json_schema` functions over one sealed schema. The Rust core also exposes a reusable typed adapter. | `src/model.sifr`; `backend/pydantic_sifr_core/tests/type_adapter.rs` |
| `api/root_model` | Root values | adapted | `RootModel[T]` declares one stored `root` field. M11 owns the complete operation facade. | `src/declarations.sifr`; `demos/milestone_m8_fields_configuration` |
| `api/networks` | Network values | adapted | `Url` validates and constructs a mapped nominal Sifr value. | `src/special_values.sifr`; `src/bridges/special_values.rs` |
| `core/multi_host_url_serialization` | Multi-host URL serialization | adapted | `MultiHostUrl` validates and constructs a mapped nominal Sifr value. | `src/special_values.sifr`; `src/bridges/special_values.rs` |
| `api/pattern` | Compiled patterns | adapted | `Pattern` validates a bounded pattern and exposes its source and flags. | `src/special_values.sifr`; `src/bridges/special_values.rs` |
| `api/field_metadata` | Field metadata | adapted | Typed descriptors define constraints, aliases, checked error overrides, and bounded JSON Schema annotations. | `src/declarations.sifr`; `src/schema_contract.sifr` |
| `api/errors` | Validation errors | adapted | Validation failures are typed `Result` errors with stable codes, ordered locations, context, expected values, and truncation state. | `backend/pydantic_sifr_core/tests/validation_models.rs`; `src/errors.sifr` |
| `api/json_schema` | JSON Schema | adapted | `model_json_schema` emits Draft 2020-12 from the sealed static schema with deterministic `$defs`, aliases, constraints, and modes. | `src/model.sifr`; `backend/pydantic_sifr_core/tests/json_schema_dialect.rs` |

The machine-readable source for this table is
`tests/compatibility/ps10.toml`. Earlier milestone ledgers remain the detailed
fixture-family record for delivered validation and serialization behavior.
