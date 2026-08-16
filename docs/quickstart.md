# Quick start

`pydantic_sifr` validates ordinary Sifr classes. It returns typed `Result`
values and does not create Python objects.

## Declare and validate a model

Import the public validator and schema verifier. Attach the verifier to the
class that defines the validation target.

```sifr
from pydantic_sifr import ValidationError
from pydantic_sifr import model_validate_json
from pydantic_sifr import verify_schema


@const_specialize("pydantic_sifr.schema_contract", "verify_schema")
@metadata("field", "id", "pydantic.gt", "0")
class User:
    id: int64
    name: str


def main() -> Result[None, ValidationError | RustPanicError]:
    try:
        user: User = model_validate_json(b'{"id":7,"name":"Ada"}')
        expected_id: int64 = 7
        assert user.id == expected_id
    except ValidationError as error:
        raise error
    except RustPanicError as error:
        raise error
    return None
```

The compiler creates and seals one schema program for `User`. The native core
parses the JSON, validates it, and constructs `User` directly. A validation
failure returns `ValidationError` with stable codes and locations.

## Run the end-to-end demos

Set `SIFR_BIN` to the certified compiler binary. Then run either dependent app:

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

The PS6 demo covers model inputs, constraints, aliases, defaults, structural
input, and public errors. The PS7 demo covers literals, enums, ordinary and
tagged unions, recursion, and branch errors. Both demos are mandatory in the
companion create-PR and merge gates.

## Select an input profile

Use `model_validate_json` for JSON bytes. Use `model_validate` for typed
structural input. Use `model_validate_strings` when scalar leaves contain text
that the declared schema can convert. All three functions use the same native
validation engine.

See the [migration guide](migration.md) for field and configuration mappings.
See the [compatibility ledger](compatibility.md) for supported and blocked
surfaces. See [certification](certification.md) for the exact release tuple and
test procedure.
