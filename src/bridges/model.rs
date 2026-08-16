use core::fmt;

use pydantic_sifr_core::{
    JsonIntegerProfile, JsonLimits, JsonSchemaMode, JsonSchemaOptions, PreparedSchema,
    SerializationOptions, SerializationPlan, ValidationError, ValidationOptions,
    generate_prepared_json_schema_bytes, serialize_json, validate_json_and_construct,
    validate_json_strings_and_construct, validate_structural_and_construct,
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

#[derive(Debug)]
pub struct ModelSerializationError {
    message: String,
}

impl fmt::Display for ModelSerializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ModelSerializationError {}

#[derive(Debug)]
pub struct ModelJsonSchemaError {
    message: String,
}

impl fmt::Display for ModelJsonSchemaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ModelJsonSchemaError {}

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
    validate_json_strings_and_construct(
        &schema,
        payload,
        JsonLimits::default(),
        ValidationOptions::default(),
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

pub fn dump_json<T>(value: &T) -> Result<Vec<u8>, ModelSerializationError>
where
    T: StructuralProject + StaticProgramType,
{
    let schema = PreparedSchema::from_static::<T>().map_err(serialization_setup_error)?;
    let plan =
        SerializationPlan::from_prepared(schema, JsonIntegerProfile::Exact).map_err(|error| {
            ModelSerializationError {
                message: error.to_string(),
            }
        })?;
    serialize_json(&plan, value, &SerializationOptions::default()).map_err(|error| {
        ModelSerializationError {
            message: error.to_string(),
        }
    })
}

pub fn json_schema<T>(_target: &T) -> Result<Vec<u8>, ModelJsonSchemaError>
where
    T: StructuralProject + StaticProgramType,
{
    let schema = PreparedSchema::from_static::<T>().map_err(|error| ModelJsonSchemaError {
        message: error.to_string(),
    })?;
    generate_prepared_json_schema_bytes(
        &schema,
        JsonSchemaOptions::new(JsonSchemaMode::Validation, true),
        JsonIntegerProfile::Exact,
    )
    .map_err(|error| ModelJsonSchemaError {
        message: error.to_string(),
    })
}

fn serialization_setup_error(error: ValidationError) -> ModelSerializationError {
    ModelSerializationError {
        message: error.to_string(),
    }
}

fn validation_error_json(error: &ValidationError) -> String {
    let details = error
        .details()
        .iter()
        .map(|detail| {
            let location = detail
                .location
                .iter()
                .map(location_item_json)
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

fn location_item_json(item: &pydantic_sifr_core::LocationItem) -> String {
    match item {
        pydantic_sifr_core::LocationItem::Field(value)
        | pydantic_sifr_core::LocationItem::Branch(value) => {
            format!("\"{}\"", escape_json(value))
        }
        pydantic_sifr_core::LocationItem::Index(value)
        | pydantic_sifr_core::LocationItem::MappingKey(value) => value.to_string(),
    }
}

fn escape_json(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\u{08}' => escaped.push_str("\\b"),
            '\u{0c}' => escaped.push_str("\\f"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            control if control <= '\u{1f}' => {
                let value = u32::from(control);
                escaped.push_str("\\u00");
                escaped.push(hex_digit(value >> 4));
                escaped.push(hex_digit(value & 0x0f));
            }
            character => escaped.push(character),
        }
    }
    escaped
}

const fn hex_digit(value: u32) -> char {
    match value {
        0 => '0',
        1 => '1',
        2 => '2',
        3 => '3',
        4 => '4',
        5 => '5',
        6 => '6',
        7 => '7',
        8 => '8',
        9 => '9',
        10 => 'a',
        11 => 'b',
        12 => 'c',
        13 => 'd',
        14 => 'e',
        15 => 'f',
        _ => '0',
    }
}

#[cfg(test)]
mod tests {
    use pydantic_sifr_core::LocationItem;

    use super::{escape_json, location_item_json};

    #[test]
    fn json_escape_covers_every_control_character() {
        let controls = (0_u8..=0x1f).map(char::from).collect::<String>();
        let escaped = escape_json(&controls);
        assert_eq!(
            escaped,
            "\\u0000\\u0001\\u0002\\u0003\\u0004\\u0005\\u0006\\u0007\\b\\t\\n\\u000b\\f\\r\\u000e\\u000f\\u0010\\u0011\\u0012\\u0013\\u0014\\u0015\\u0016\\u0017\\u0018\\u0019\\u001a\\u001b\\u001c\\u001d\\u001e\\u001f"
        );
    }

    #[test]
    fn branch_locations_use_the_stable_string_representation() {
        assert_eq!(
            location_item_json(&LocationItem::Branch("int\"choice".to_owned())),
            "\"int\\\"choice\""
        );
    }
}
