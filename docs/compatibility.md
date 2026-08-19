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
| `api/base_model` | Model validation | adapted | Models derive from `BaseModel`. Attached validation methods return `Result` and construct the declared Sifr class. | `src/declarations.sifr`; `src/api.sifr`; `demos/milestone_m11_model_operations` |
| `api/field_configuration` | `Field` and model configuration | adapted | Typed `Field`, `ConfigDict`, and `Constraints` descriptors define aliases, constraints, defaults, extras, and schema annotations. | `src/declarations.sifr`; `src/schema_contract.sifr`; `demos/milestone_m8_fields_configuration` |
| `api/validators` | Validator declarations | adapted | `field_validator` supports before, after, and plain modes with explicit targets. `model_validator` supports before and after modes. Before handlers declare one static input type. Wrap mode and wildcard targets are excluded. | `src/declarations.sifr`; `src/schema_contract.sifr`; `demos/milestone_m9_validators` |
| `api/serialization` | Serializer and computed-field declarations | adapted | Checked field and model serializers support four `when_used` policies. Checked computed fields add serialization-only output. Attached dump methods use the same sealed static program and typed context. | `src/declarations.sifr`; `src/model.sifr`; `demos/milestone_m10_serializers` |
| `api/type_adapter` | `TypeAdapter[T]` | adapted | `TypeAdapter[T]` is a transparent type alias. Its attached validation, dump, and JSON Schema methods use the same sealed program as `T`. | `src/__init__.sifr`; `src/api.sifr`; `demos/milestone_m11_model_operations` |
| `api/root_model` | Root values | adapted | `RootModel[T]` declares one stored `root` field. Attached validation accepts the root value. Dump and JSON Schema operations expose the root field schema. | `src/declarations.sifr`; `backend/pydantic_sifr_core/tests/typed_model_construction.rs`; `demos/milestone_m11_model_operations` |
| `api/networks` | Network values | adapted | `Url` validates and constructs a mapped nominal Sifr value. | `src/special_values.sifr`; `src/bridges/special_values.rs` |
| `core/multi_host_url_serialization` | Multi-host URL serialization | adapted | `MultiHostUrl` validates and constructs a mapped nominal Sifr value. | `src/special_values.sifr`; `src/bridges/special_values.rs` |
| `api/pattern` | Compiled patterns | adapted | `Pattern` validates a bounded pattern and exposes its source and flags. | `src/special_values.sifr`; `src/bridges/special_values.rs` |
| `api/field_metadata` | Field metadata | adapted | Typed descriptors define constraints, aliases, checked error overrides, and bounded JSON Schema annotations. | `src/declarations.sifr`; `src/schema_contract.sifr` |
| `api/errors` | Validation errors | adapted | Validation failures are typed `Result` errors with stable codes, ordered locations, context, expected values, and truncation state. | `backend/pydantic_sifr_core/tests/validation_models.rs`; `src/errors.sifr` |
| `api/json_schema` | JSON Schema | adapted | The attached `model_json_schema` type method emits Draft 2020-12 from the sealed static schema. It accepts mode, alias, and integer-profile options without a dummy model value. | `src/api.sifr`; `backend/pydantic_sifr_core/tests/json_schema_dialect.rs`; `demos/milestone_m11_model_operations` |

The machine-readable source for this table is
`tests/compatibility/ps10.toml`. Earlier milestone ledgers remain the detailed
fixture-family record for delivered validation and serialization behavior.
