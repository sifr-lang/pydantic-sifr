use base64::Engine;

use crate::InputValue;

use super::{
    BytesConstraints, BytesJsonMode, ErrorDetail, InputProfile, StringConstraints, ValidatedValue,
    ValidationError, scalars::type_error,
};

pub(crate) fn validate_string(
    input: &InputValue,
    strict: bool,
    constraints: &StringConstraints,
) -> Result<ValidatedValue, ValidationError> {
    if constraints.to_upper && constraints.to_lower {
        return Err(type_error(
            "schema_invalid",
            "to_upper and to_lower cannot both be enabled",
            "one case conversion",
        ));
    }
    let converted = match input {
        InputValue::String(value) => value.clone(),
        InputValue::Bytes(value) if !strict => String::from_utf8(value.clone()).map_err(|_| {
            type_error("string_unicode", "Bytes must contain UTF-8", "UTF-8 string")
        })?,
        InputValue::Integer(value) if !strict && constraints.coerce_numbers_to_str => value.clone(),
        InputValue::Float(value)
            if !strict && constraints.coerce_numbers_to_str && value.is_finite() =>
        {
            value.to_string()
        }
        InputValue::Decimal(value) if !strict && constraints.coerce_numbers_to_str => value.clone(),
        _ => return Err(type_error("string_type", "Input must be a string", "str")),
    };
    let mut value = if constraints.strip_whitespace {
        converted.trim().to_owned()
    } else {
        converted
    };
    if constraints.ascii_only && !value.is_ascii() {
        return Err(type_error(
            "string_not_ascii",
            "String must contain only ASCII characters",
            "ASCII string",
        ));
    }
    let length = value.chars().count();
    validate_length(
        length,
        constraints.min_length,
        constraints.max_length,
        "string",
    )?;
    if let Some(pattern) = &constraints.pattern
        && !pattern.is_match(&value)
    {
        return Err(ValidationError::one(
            ErrorDetail::new(
                "string_pattern_mismatch",
                "String does not match the pattern",
            )
            .context("pattern", pattern.source()),
        ));
    }
    if constraints.to_upper {
        value = value.to_uppercase();
    } else if constraints.to_lower {
        value = value.to_lowercase();
    }
    Ok(ValidatedValue::String(value))
}

pub(crate) fn validate_bytes(
    input: &InputValue,
    strict: bool,
    profile: InputProfile,
    constraints: &BytesConstraints,
) -> Result<ValidatedValue, ValidationError> {
    let value = match input {
        InputValue::Bytes(value) => value.clone(),
        InputValue::String(value) if !strict || profile != InputProfile::Native => {
            decode_text_bytes(value, profile, constraints.json_mode)?
        }
        _ => return Err(type_error("bytes_type", "Input must be bytes", "bytes")),
    };
    validate_length(
        value.len(),
        constraints.min_length,
        constraints.max_length,
        "bytes",
    )?;
    Ok(ValidatedValue::Bytes(value))
}

fn decode_text_bytes(
    value: &str,
    profile: InputProfile,
    json_mode: BytesJsonMode,
) -> Result<Vec<u8>, ValidationError> {
    if profile != InputProfile::Json || json_mode == BytesJsonMode::Utf8 {
        return Ok(value.as_bytes().to_vec());
    }
    base64::engine::general_purpose::URL_SAFE
        .decode(value)
        .map_err(|error| {
            ValidationError::one(
                ErrorDetail::new("bytes_base64", "Input must use valid URL-safe base64")
                    .expected("URL-safe base64 without whitespace")
                    .context("error", error.to_string()),
            )
        })
}

pub(crate) fn validate_length(
    length: usize,
    minimum: Option<usize>,
    maximum: Option<usize>,
    kind: &str,
) -> Result<(), ValidationError> {
    if minimum.is_some_and(|minimum| length < minimum) {
        return Err(ValidationError::one(
            ErrorDetail::new("too_short", format!("{kind} is too short"))
                .context("minimum", minimum.unwrap_or_default().to_string()),
        ));
    }
    if maximum.is_some_and(|maximum| length > maximum) {
        return Err(ValidationError::one(
            ErrorDetail::new("too_long", format!("{kind} is too long"))
                .context("maximum", maximum.unwrap_or_default().to_string()),
        ));
    }
    Ok(())
}
