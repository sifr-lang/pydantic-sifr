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
from pydantic_sifr import model_dump_json
from pydantic_sifr import model_json_schema
from pydantic_sifr import model_validate_json
class User(BaseModel):
    model_config = ConfigDict(extra="forbid")
    id: int64 = Field(gt=0)
    name: str


def main() -> Result[
    None,
    ValidationError | SerializationError | JsonSchemaError | RustPanicError,
]:
    try:
        user: User = model_validate_json(b'{"id":7,"name":"Ada"}')
        expected_id: int64 = 7
        assert user.id == expected_id
        output: bytes = model_dump_json(user)
        schema: bytes = model_json_schema(user)
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

`model_dump_json` serializes a typed value. `model_json_schema` uses its typed
argument only to select the sealed schema and returns a Draft 2020-12 document.
Both functions use the same static program as validation.

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

The PS6 demo covers inputs, constraints, aliases, defaults, and errors. The PS7
demo covers sums and recursion. The M8 demo covers the declaration facade. The
M9 demo covers checked field and model validators.

All four demos are mandatory in the package gates.

## Select an input profile

Use `model_validate_json` for JSON bytes. Use `model_validate` for typed
structural input. Use `model_validate_strings` when scalar leaves contain text
that the declared schema can convert. All three functions use the same native
validation engine.

See the [migration guide](migration.md) for field and configuration mappings.
See the [compatibility ledger](compatibility.md) for supported and blocked
surfaces. See [certification](certification.md) for the exact release tuple and
test procedure.

The canonical single-file example is
[`demos/pydantic_sifr_demo.sifr`](../demos/pydantic_sifr_demo.sifr). The gate
runs that exact file in a dependent package and compares its output with the
checked snapshot.
