# Migrate from Pydantic

`pydantic-sifr` uses static Sifr classes and one native validation engine. It
does not load Python or inspect runtime model objects.

Read the [compatibility matrix](compatibility.md) before migration. The matrix
identifies each adapted or blocked API.

## Declare a model

Replace a Python `BaseModel` subclass with a Sifr `BaseModel` subclass.

```sifr
from pydantic_sifr import BaseModel


class User(BaseModel):
    id: int64
    name: str
    active: bool = True
```

Sifr keeps required, defaulted, and nullable fields distinct. Use `T | None`
only when `None` is a valid field value.

## Replace `Field` and model configuration

Use `Field` for field rules. Use `ConfigDict` for model configuration.

The compiler evaluates these typed descriptors during compilation.

```sifr
from pydantic_sifr import BaseModel, ConfigDict, Field


class User(BaseModel):
    model_config = ConfigDict(extra="forbid")
    id: int64 = Field(alias="user_id", gt=0)
    name: str = Field(min_length=1, max_length=100)
    active: bool = True
```

The package normalizes each descriptor before it derives the static schema.
An invalid argument stops compilation at that argument.

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
class User(BaseModel):
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

## Convert validators

Use `field_validator` for field callbacks. Use `model_validator` for model
callbacks. A field validator must name each target. Wildcard targets and wrap
mode are not available.

```sifr
from pydantic_sifr import BaseModel, field_validator, model_validator


class RawUser:
    name: bool


class User(BaseModel):
    name: str

    @field_validator("name", mode="before")
    @classmethod
    def normalize_name(cls, own value: bool) -> str:
        return str(value)

    @model_validator(mode="before")
    @classmethod
    def normalize_input(cls, own value: RawUser) -> RawUser:
        return value

    @model_validator(mode="after")
    def check_user(own self) -> Self:
        return self
```

A field before handler can use an input type that differs from the field type.
A model before handler must use one concrete structural input type. An after
model handler consumes the constructed model. Its returned `Self` becomes the
input to the next after handler.

Use the validator-aware validation functions when a handler needs typed
context or when a model declares validators. Callback failures join the normal
validation error. They keep the field or model location and package context.

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

Use `AliasPath` for a nested input path. Each segment selects the next input
value.

```sifr
from pydantic_sifr import AliasPath, BaseModel, Field


class Address(BaseModel):
    city: str
    postal_code: int64 = Field(
        validation_alias=AliasPath(["address_data", 0, "postal"])
    )
```

The validation error location uses the alias path when the model enables alias
locations.

## APIs that are not available

The package does not publish serializer decorators or computed fields in this
milestone. Validator wrap mode and wildcard field targets are also not
available.

Do not retain a Python model path beside the Sifr model. Choose one schema
owner and one validation path for each migrated boundary.

## Migration checklist

1. Derive each selected model from `pydantic_sifr.BaseModel`.
2. Convert field rules to `Field` calls.
3. Convert model configuration to `ConfigDict`.
4. Convert selected validators to checked field or model handlers.
5. Replace exception-based calls with `Result` return types.
6. Select one input function for each input boundary.
7. Compare each required API with the compatibility matrix.
8. Remove the Python model path after the Sifr boundary passes its tests.

The complete field and configuration example is in
`demos/milestone_m8_fields_configuration`. The validator example is in
`demos/milestone_m9_validators`.
