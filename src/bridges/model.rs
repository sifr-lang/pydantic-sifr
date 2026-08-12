use core::fmt;

use pydantic_sifr_core::{
    InputProfile, JsonLimits, PreparedSchema, ValidationError, ValidationOptions, parse_json,
    validate_and_construct, validate_json_and_construct, validate_structural_and_construct,
};
use sifr_runtime::interop::structural::StructuralProject;
use sifr_runtime::interop::structural::{StaticProgramType, StructuralConstruct};

#[derive(Debug)]
pub struct ModelValidationError {
    errors_json: String,
}

impl ModelValidationError {
    fn from_validation(error: &ValidationError) -> Self {
        Self {
            errors_json: validation_error_json(error),
        }
    }
}

impl fmt::Display for ModelValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.errors_json)
    }
}

impl std::error::Error for ModelValidationError {}

pub fn validate_json<T>(payload: &[u8]) -> Result<T, ModelValidationError>
where
    T: StructuralConstruct + StaticProgramType,
{
    let schema = PreparedSchema::from_static::<T>()
        .map_err(|error| ModelValidationError::from_validation(&error))?;
    validate_json_and_construct(
        &schema,
        payload,
        JsonLimits::default(),
        ValidationOptions::default(),
    )
    .map_err(|error| ModelValidationError::from_validation(&error))
}

pub fn validate_strings<T>(payload: &[u8]) -> Result<T, ModelValidationError>
where
    T: StructuralConstruct + StaticProgramType,
{
    let schema = PreparedSchema::from_static::<T>()
        .map_err(|error| ModelValidationError::from_validation(&error))?;
    let input = parse_json(payload, JsonLimits::default())
        .map_err(|error| ModelValidationError {
            errors_json: format!(
                "{{\"errors\":[{{\"code\":\"{}\",\"message\":\"{}\",\"location\":[{},{}]}}],\"truncated\":false}}",
                escape_json(error.code),
                escape_json(&error.message),
                error.line,
                error.column,
            ),
        })?;
    validate_and_construct(
        &schema,
        &input,
        ValidationOptions {
            profile: InputProfile::Strings,
            ..ValidationOptions::default()
        },
    )
    .map_err(|error| ModelValidationError::from_validation(&error))
}

pub fn validate<Input, T>(payload: &Input) -> Result<T, ModelValidationError>
where
    T: StructuralConstruct + StaticProgramType,
    Input: StructuralProject,
{
    let schema = PreparedSchema::from_static::<T>()
        .map_err(|error| ModelValidationError::from_validation(&error))?;
    validate_structural_and_construct(
        &schema,
        payload,
        JsonLimits::default(),
        ValidationOptions::default(),
    )
    .map_err(|error| ModelValidationError::from_validation(&error))
}

fn validation_error_json(error: &ValidationError) -> String {
    let details = error
        .details()
        .iter()
        .map(|detail| {
            let location = detail
                .location
                .iter()
                .map(|item| match item {
                    pydantic_sifr_core::LocationItem::Field(value) => {
                        format!("\"{}\"", escape_json(value))
                    }
                    pydantic_sifr_core::LocationItem::Index(value)
                    | pydantic_sifr_core::LocationItem::MappingKey(value) => value.to_string(),
                })
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"code\":\"{}\",\"message\":\"{}\",\"expected\":\"{}\",\"location\":[{}]}}",
                escape_json(detail.code),
                escape_json(&detail.message),
                escape_json(&detail.expected),
                location,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"errors\":[{}],\"truncated\":{}}}",
        details,
        error.is_truncated(),
    )
}

fn escape_json(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
