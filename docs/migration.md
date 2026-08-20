# Migrate from Pydantic

`pydantic-sifr` uses static Sifr classes and one native validation engine. It
does not load Python or inspect runtime model objects.

Read the [compatibility matrix](compatibility.md) before migration. The matrix
identifies delivered behavior and terminal exclusions. The current matrix uses
`adapted` and `excluded` statuses.

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

Use the attached model operations. Each operation returns `Result` and
constructs the requested Sifr type.

```sifr
from pydantic_sifr import ValidationError


def parse_user(payload: bytes) -> Result[User, ValidationError | RustPanicError]:
    try:
        user: User = User.model_validate_json(payload)
        return user
    except ValidationError as error:
        raise error
    except RustPanicError as error:
        raise error
```

Use `User.model_validate` for typed structural input. Use
`User.model_validate_strings` for a bare string or a structural value whose
mapping keys and scalar leaves are strings.

Use `user.model_dump_json()` for JSON output. Use
`User.model_json_schema()` to emit a Draft 2020-12 document without a dummy
model value. All operations use the same sealed schema program.

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

## Convert serializers and computed fields

Use `field_serializer` for selected fields. Use `model_serializer` when the
complete serialized result has a different structural type. Use
`computed_field` for a checked, zero-argument instance method whose result is
present only in serialized output.

Each serializer declares a checked input and output type. The `when_used`
argument selects always, unless-`None`, JSON, or JSON-unless-`None` execution.
Use the serializer-aware dump methods when a callback needs typed context.

The complete example is in `demos/milestone_m10_serializers`.

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

`model_construct` is not available because it bypasses validation. Use an
ordinary Sifr constructor when values are already trusted and typed.

Dynamic `model_copy` updates are not available. Construct a new value with the
ordinary Sifr constructor. Use explicit cloning first when the value and its
fields support cloning.

The package also excludes runtime model creation and schema rebuilding,
attribute probing, private attributes, call interception, assignment
interception, Python metaclasses and dataclasses, multiple data inheritance,
and frozen-model emulation. Validator wrap mode and wildcard field targets are
not available. The compatibility matrix records every terminal exclusion and
its reason.

Do not retain a Python model path beside the Sifr model. Choose one schema
owner and one validation path for each migrated boundary.

## Migration checklist

1. Derive each selected model from `pydantic_sifr.BaseModel`.
2. Convert field rules to `Field` calls.
3. Convert model configuration to `ConfigDict`.
4. Convert selected validators to checked field or model handlers.
5. Convert serializers and computed fields to checked handlers.
6. Replace exception-based calls with `Result` return types.
7. Select one input function for each input boundary.
8. Replace dynamic construction and copy updates with ordinary construction or
   explicit cloning.
9. Compare each required API with the compatibility matrix.
10. Remove the Python model path after the Sifr boundary passes its tests.

The complete field and configuration example is in
`demos/milestone_m8_fields_configuration`. The validator example is in
`demos/milestone_m9_validators`. The serializer and computed-field example is
in `demos/milestone_m10_serializers`.
