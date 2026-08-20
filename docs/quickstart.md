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
cd demos/milestone_ps_6_demo
"$SIFR_BIN" fetch --locked
"$SIFR_BIN" run --locked
```

```bash
cd demos/milestone_ps_7_demo
"$SIFR_BIN" fetch --locked
"$SIFR_BIN" run --locked
```

```bash
cd demos/milestone_m8_fields_configuration
"$SIFR_BIN" fetch --locked
"$SIFR_BIN" run --locked
```

```bash
cd demos/milestone_m9_validators
"$SIFR_BIN" fetch --locked
"$SIFR_BIN" run --locked
```

```bash
cd demos/milestone_m10_serializers
"$SIFR_BIN" fetch --locked
"$SIFR_BIN" run --locked
```

```bash
cd demos/milestone_m11_model_operations
"$SIFR_BIN" fetch --locked
"$SIFR_BIN" run --locked
```

The PS6 demo covers inputs, constraints, aliases, defaults, and errors. The PS7
demo covers sums and recursion. The M8 demo covers the declaration facade. The
M9 demo covers checked field and model validators. The M10 demo covers attached
dump methods, serializers, computed fields, selections, and typed context. The
M11 demo covers attached model operations, root models, and type adapters.

All six demos are mandatory in the package gates.

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

The canonical single-file example is
[`demos/pydantic_sifr_demo.sifr`](../demos/pydantic_sifr_demo.sifr). The gate
runs that exact file in a dependent package and compares its output with the
checked snapshot.
