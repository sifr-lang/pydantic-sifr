# Migrate from Pydantic

`pydantic-sifr` uses static Sifr classes and one native validation engine. It
does not load Python or inspect runtime model objects.

Read the [compatibility matrix](compatibility.md) before migration. The matrix
identifies each adapted or blocked API.

## Declare a model

Replace a Python `BaseModel` subclass with an ordinary Sifr class. Add the
package schema specialization to each validated class.

```sifr
from pydantic_sifr import verify_schema


@const_specialize("pydantic_sifr.schema_contract", "verify_schema")
class User:
    id: int64
    name: str
    active: bool = True
```

Sifr keeps required, defaulted, and nullable fields distinct. Use `T | None`
only when `None` is a valid field value.

## Replace `Field` and model configuration

Use static declaration metadata instead of runtime `Field` or `ConfigDict`
objects. Metadata values are compile-time strings.

```sifr
@const_specialize("pydantic_sifr.schema_contract", "verify_schema")
@metadata("pydantic.extra", "forbid")
@metadata("field", "id", "pydantic.alias.field", "user_id")
@metadata("field", "id", "pydantic.gt", "0")
@metadata("field", "name", "pydantic.min_length", "1")
@metadata("field", "name", "pydantic.max_length", "100")
class User:
    id: int64
    name: str
    active: bool = True
```

The package applies this metadata during static schema specialization. Invalid
metadata stops compilation with a package-owned diagnostic.

## Validate input

Import the functional entry points. Each entry point returns `Result` and
constructs the requested Sifr type.

```sifr
from pydantic_sifr import ValidationError
from pydantic_sifr import model_validate
from pydantic_sifr import model_validate_json
from pydantic_sifr import model_validate_strings
from pydantic_sifr import model_dump_json
from pydantic_sifr import model_json_schema


def parse_user(payload: bytes) -> Result[User, ValidationError | RustPanicError]:
    try:
        user: User = model_validate_json(payload)
        return user
    except ValidationError as error:
        raise error
    except RustPanicError as error:
        raise error
```

Use `model_validate` for a typed structural input. Use
`model_validate_strings` when scalar leaves contain text that needs declared
coercion.

Use `model_dump_json(value)` for JSON output. Use
`model_json_schema(value)` to select the value's sealed schema and emit a Draft
2020-12 document. The schema function does not inspect the value.

You can add thin class methods when an application needs method syntax. These
methods must call the same functional entry points.

```sifr
class User:
    id: int64
    name: str

    @classmethod
    def model_validate_json(
        cls, payload: bytes
    ) -> Result[User, ValidationError | RustPanicError]:
        try:
            user: User = model_validate_json(payload)
            return user
        except ValidationError as error:
            raise error
        except RustPanicError as error:
            raise error
```

This method does not create a second validator. Static specialization and the
native core still own the schema and validation operation.

## Handle errors

Pydantic raises `ValidationError`. Sifr returns a typed error through
`Result`.

```sifr
def load_user(payload: bytes) -> Result[User, ValidationError | RustPanicError]:
    try:
        return model_validate_json(payload)
    except ValidationError as error:
        print(error.message)
        raise error
    except RustPanicError as error:
        raise error
```

`ValidationError.message` contains stable JSON. It includes ordered error
details, typed locations, expected values, context, and truncation state.

A contained Rust panic uses `RustPanicError`. It is not a validation error.

## Use aliases and nested paths

Repeat alias metadata in path order. Each `field` or `index` segment selects
the next part of the input path.

```sifr
@metadata("field", "postal_code", "pydantic.alias.field", "address_data")
@metadata("field", "postal_code", "pydantic.alias.index", "0")
@metadata("field", "postal_code", "pydantic.alias.field", "postal")
class Address:
    city: str
    postal_code: int64
```

The validation error location uses the alias path when the model enables alias
locations.

## APIs that are not available

The package does not publish validator decorators, serializer decorators,
computed fields, or a general `BaseModel` facade. These APIs require the typed
method-slot work in issues [#10](https://github.com/sifr-lang/pydantic-sifr/issues/10)
and [#14](https://github.com/sifr-lang/pydantic-sifr/issues/14).

Nominal network and compiled-pattern values require the structural mapping in
issue [#27](https://github.com/sifr-lang/pydantic-sifr/issues/27). Core support
does not make these Sifr APIs available.

Do not retain a Python model path beside the Sifr model. Choose one schema
owner and one validation path for each migrated boundary.

## Migration checklist

1. Replace each selected `BaseModel` subclass with an ordinary Sifr class.
2. Add `@const_specialize` to each validated class.
3. Convert supported field and model rules to `@metadata` declarations.
4. Replace exception-based calls with `Result` return types.
5. Select the structural, JSON, or strings entry point for each input boundary.
6. Compare each required API with the compatibility matrix.
7. Remove the Python model path after the Sifr boundary passes its tests.

The complete model example is in `demos/milestone_ps_6_demo`.
