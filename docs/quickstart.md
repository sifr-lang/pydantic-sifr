# Quick start

`pydantic_sifr` validates adapted Sifr classes. It returns typed `Result` values.
It does not create Python objects.

## Declare and validate a model

Derive the model from `BaseModel`. Use `Field` for field rules. Use
`ConfigDict` for model configuration.

```sifr
from pydantic_sifr import BaseModel, ConfigDict, Field, JsonSchemaError
from pydantic_sifr import SerializationError
from pydantic_sifr import ValidationError
class User(BaseModel):
    model_config = ConfigDict(extra="forbid")
    id: int64 = Field(gt=0)
    name: str


def main() -> Result[
    None,
    ValidationError | SerializationError | JsonSchemaError | RustPanicError,
]:
    try:
        user: User = User.model_validate_json(b'{"id":7,"name":"Ada"}')
        expected_id: int64 = 7
        assert user.id == expected_id
        output: bytes = user.model_dump_json()
        schema: bytes = User.model_json_schema()
    except ValidationError as error:
        raise error
    except SerializationError as error:
        raise error
    except JsonSchemaError as error:
        raise error
    except RustPanicError as error:
        raise error
    return None
```

The compiler creates and seals one schema program for `User`. The native core
parses the JSON, validates it, and constructs `User` directly. A validation
failure returns `ValidationError` with stable codes and locations.

`model_dump_json` serializes a typed value. The `model_json_schema` type method
returns a Draft 2020-12 document without a dummy model value. Both methods use
the same static program as validation.

## Run the end-to-end demos

Set `SIFR_BIN` to the certified compiler binary. Then run the dependent apps.

```bash
cd demos/model_validation
"$SIFR_BIN" fetch --locked
"$SIFR_BIN" run --locked
```

```bash
cd demos/fields_and_configuration
"$SIFR_BIN" fetch --locked
"$SIFR_BIN" run --locked
```

```bash
cd demos/validators
"$SIFR_BIN" fetch --locked
"$SIFR_BIN" run --locked
```

```bash
cd demos/serializers_and_computed_fields
"$SIFR_BIN" fetch --locked
"$SIFR_BIN" run --locked
```

The model-validation demo covers all input profiles, errors, dumps, JSON
Schema, root models, type adapters, and concrete generic models.

The fields-and-configuration demo covers aliases, constraints, sums, recursion,
schema annotations, and mapped special values. The remaining demos cover
validators, serializers, and computed fields.

All four demos are mandatory in the package gates.

## Select an input profile

Use `Model.model_validate_json` for JSON bytes. Use `Model.model_validate` for
typed structural input. Use `Model.model_validate_strings` for a bare string
or a structural value whose mapping keys and scalar leaves are strings. All
three methods use the same native validation engine.

See the [migration guide](migration.md) for field and configuration mappings.
See the [compatibility ledger](compatibility.md) for supported behavior and
terminal exclusions. The current matrix uses `adapted` and `excluded`
statuses. See [certification](certification.md) for the exact release tuple and
test procedure.

The canonical example is
[`demos/model_validation/src/main.sifr`](../demos/model_validation/src/main.sifr).
The gate runs that exact file and compares its output with the checked snapshot.
