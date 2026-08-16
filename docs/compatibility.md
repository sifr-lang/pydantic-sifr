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
| `api/base_model` | Model validation | adapted | `model_validate`, `model_validate_json`, and `model_validate_strings` return `Result` and construct ordinary Sifr classes. | `src/model.sifr`; `backend/pydantic_sifr_core/tests/typed_model_construction.rs` |
| `api/field_configuration` | `Field` and model configuration | adapted | Static `@metadata` declarations encode aliases, constraints, extras, name population, and error locations. Runtime `Field(...)` objects do not exist. | `src/schema_contract.sifr`; `backend/pydantic_sifr_core/tests/validation_models.rs` |
| `api/validators` | Validator declarations | blocked | Before, after, plain, and wrap callbacks require typed handler-bearing method slots. | [#10](https://github.com/sifr-lang/pydantic-sifr/issues/10) |
| `api/serialization` | Serializer and computed-field declarations | blocked | Custom serialization and computed accessors require typed handler-bearing method slots. | [#14](https://github.com/sifr-lang/pydantic-sifr/issues/14) |
| `api/type_adapter` | `TypeAdapter[T]` | adapted | Sifr uses target-inferred validation, `model_dump_json`, and `model_json_schema` functions over one sealed schema. The Rust core also exposes a reusable typed adapter. | `src/model.sifr`; `backend/pydantic_sifr_core/tests/type_adapter.rs` |
| `api/root_model` | Root values | blocked | The adapter engine validates non-model roots, but the Pydantic-familiar `RootModel[T]` facade depends on the blocked facade/callback surface. | [#10](https://github.com/sifr-lang/pydantic-sifr/issues/10) |
| `api/networks` | Network values | blocked | URL validation exists in the shared core. Sifr-visible nominal network values require package-defined structural mappings. | [#27](https://github.com/sifr-lang/pydantic-sifr/issues/27) |
| `core/multi_host_url_serialization` | Multi-host URL serialization | blocked | The core has no Sifr-visible nominal multi-host value to serialize until package-defined structural mappings exist. | [#27](https://github.com/sifr-lang/pydantic-sifr/issues/27) |
| `api/pattern` | Compiled patterns | blocked | Bounded compilation and matching exist in the shared core. The Sifr-visible nominal value requires a package-defined structural mapping. | [#27](https://github.com/sifr-lang/pydantic-sifr/issues/27) |
| `api/field_metadata` | Field metadata | adapted | Constraints and aliases are compile-time declaration metadata. | `src/schema_contract.sifr` |
| `api/errors` | Validation errors | adapted | Validation failures are typed `Result` errors with stable codes, ordered locations, context, expected values, and truncation state. | `backend/pydantic_sifr_core/tests/validation_models.rs`; `src/errors.sifr` |
| `api/json_schema` | JSON Schema | adapted | `model_json_schema` emits Draft 2020-12 from the sealed static schema with deterministic `$defs`, aliases, constraints, and modes. | `src/model.sifr`; `backend/pydantic_sifr_core/tests/json_schema_dialect.rs` |

The machine-readable source for this table is
`tests/compatibility/ps10.toml`. Earlier milestone ledgers remain the detailed
fixture-family record for delivered validation and serialization behavior.
