use sifr_runtime::interop::structural::{
    StructuralConstruct, StructuralProject, structural_construct,
};

use crate::{
    InputArena, JsonInputError, JsonLimits, NativeInputError, NativeValue, build_native_input,
    parse_json, project_structural_input,
};

use super::{
    ErrorDetail, InputProfile, LocationItem, PreparedSchema, ValidationCallbacks, ValidationError,
    ValidationOptions, validate_ref, validate_ref_with_callbacks,
};

pub fn validate_and_construct<T>(
    schema: &PreparedSchema<'_>,
    input: &InputArena,
    options: ValidationOptions,
) -> Result<T, ValidationError>
where
    T: StructuralConstruct,
{
    let mut arena = validate_ref(schema.schema(), input, options)?;
    arena
        .prepare_structural(schema.structural_identity())
        .map_err(construction_error)?;
    structural_construct(arena).map_err(construction_error)
}

pub fn validate_and_construct_with_callbacks<T>(
    schema: &PreparedSchema<'_>,
    input: &InputArena,
    options: ValidationOptions,
    callbacks: &dyn ValidationCallbacks,
) -> Result<T, ValidationError>
where
    T: StructuralConstruct,
{
    let mut arena = validate_ref_with_callbacks(schema.schema(), input, options, callbacks)?;
    arena
        .prepare_structural(schema.structural_identity())
        .map_err(construction_error)?;
    structural_construct(arena).map_err(construction_error)
}

pub fn validate_json_and_construct<T>(
    schema: &PreparedSchema<'_>,
    input: &[u8],
    json_limits: JsonLimits,
    mut options: ValidationOptions,
) -> Result<T, ValidationError>
where
    T: StructuralConstruct,
{
    options.profile = InputProfile::Json;
    let input = parse_json(input, json_limits).map_err(json_input_error)?;
    validate_and_construct(schema, &input, options)
}

pub fn validate_json_and_construct_with_callbacks<T>(
    schema: &PreparedSchema<'_>,
    input: &[u8],
    json_limits: JsonLimits,
    mut options: ValidationOptions,
    callbacks: &dyn ValidationCallbacks,
) -> Result<T, ValidationError>
where
    T: StructuralConstruct,
{
    options.profile = InputProfile::Json;
    let input = parse_json(input, json_limits).map_err(json_input_error)?;
    validate_and_construct_with_callbacks(schema, &input, options, callbacks)
}

pub fn validate_native_and_construct<T>(
    schema: &PreparedSchema<'_>,
    input: &NativeValue,
    input_limits: JsonLimits,
    mut options: ValidationOptions,
) -> Result<T, ValidationError>
where
    T: StructuralConstruct,
{
    options.profile = InputProfile::Native;
    let input = build_native_input(input, input_limits).map_err(native_input_error)?;
    validate_and_construct(schema, &input, options)
}

pub fn validate_structural_and_construct<T, Input>(
    schema: &PreparedSchema<'_>,
    input: &Input,
    input_limits: JsonLimits,
    mut options: ValidationOptions,
) -> Result<T, ValidationError>
where
    T: StructuralConstruct,
    Input: StructuralProject,
{
    options.profile = InputProfile::Native;
    let input = project_structural_input(input, input_limits).map_err(native_input_error)?;
    validate_and_construct(schema, &input, options)
}

pub fn validate_structural_and_construct_with_callbacks<T, Input>(
    schema: &PreparedSchema<'_>,
    input: &Input,
    input_limits: JsonLimits,
    mut options: ValidationOptions,
    callbacks: &dyn ValidationCallbacks,
) -> Result<T, ValidationError>
where
    T: StructuralConstruct,
    Input: StructuralProject,
{
    options.profile = InputProfile::Native;
    let input = project_structural_input(input, input_limits).map_err(native_input_error)?;
    validate_and_construct_with_callbacks(schema, &input, options, callbacks)
}

pub fn validate_strings_and_construct<T>(
    schema: &PreparedSchema<'_>,
    input: &NativeValue,
    input_limits: JsonLimits,
    mut options: ValidationOptions,
) -> Result<T, ValidationError>
where
    T: StructuralConstruct,
{
    options.profile = InputProfile::Strings;
    let input = build_native_input(input, input_limits).map_err(native_input_error)?;
    validate_and_construct(schema, &input, options)
}

pub fn validate_json_strings_and_construct<T>(
    schema: &PreparedSchema<'_>,
    input: &[u8],
    json_limits: JsonLimits,
    mut options: ValidationOptions,
) -> Result<T, ValidationError>
where
    T: StructuralConstruct,
{
    options.profile = InputProfile::Strings;
    let input = parse_json(input, json_limits).map_err(json_input_error)?;
    validate_and_construct(schema, &input, options)
}

pub fn validate_json_strings_and_construct_with_callbacks<T>(
    schema: &PreparedSchema<'_>,
    input: &[u8],
    json_limits: JsonLimits,
    mut options: ValidationOptions,
    callbacks: &dyn ValidationCallbacks,
) -> Result<T, ValidationError>
where
    T: StructuralConstruct,
{
    options.profile = InputProfile::Strings;
    let input = parse_json(input, json_limits).map_err(json_input_error)?;
    validate_and_construct_with_callbacks(schema, &input, options, callbacks)
}

fn construction_error(
    error: sifr_runtime::interop::structural::StructuralContractError,
) -> ValidationError {
    ValidationError::one(
        ErrorDetail::new(
            "internal_construction",
            "Validated value does not match the target structural type",
        )
        .expected("verified target structural shape")
        .context("error", error.to_string()),
    )
}

fn json_input_error(error: JsonInputError) -> ValidationError {
    let mut detail = ErrorDetail::new(error.code, error.message)
        .expected("valid bounded JSON")
        .context("line", error.line.to_string())
        .context("column", error.column.to_string());
    for item in error.path.into_iter().rev() {
        detail.location.insert(0, LocationItem::Field(item));
    }
    ValidationError::one(detail)
}

fn native_input_error(error: NativeInputError) -> ValidationError {
    ValidationError::one(
        ErrorDetail::new("input_limit_exceeded", "Native input could not be bounded")
            .expected("bounded structural input")
            .context("error", error.to_string()),
    )
}
