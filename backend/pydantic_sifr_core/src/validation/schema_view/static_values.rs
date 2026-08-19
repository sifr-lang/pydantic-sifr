use std::str::FromStr;

use num_bigint::BigInt;
use sifr_runtime::interop::structural::StaticProgramValue;

use crate::validation::{ErrorDetail, ValidationError, scalars::type_error};

pub(super) fn record(
    value: &'static StaticProgramValue,
    label: &'static str,
) -> Result<&'static [(&'static str, StaticProgramValue)], ValidationError> {
    match value {
        StaticProgramValue::Record(fields) => Ok(fields),
        _ => Err(schema_error_with_label(label)),
    }
}

pub(super) fn field(
    fields: &'static [(&'static str, StaticProgramValue)],
    name: &'static str,
) -> Result<&'static StaticProgramValue, ValidationError> {
    fields
        .iter()
        .find(|(field, _)| *field == name)
        .map(|(_, value)| value)
        .ok_or_else(|| schema_error("Static schema field is missing"))
}

pub(super) fn field_value(
    fields: &'static [(&'static str, StaticProgramValue)],
    name: &'static str,
) -> Result<&'static StaticProgramValue, ValidationError> {
    field(fields, name)
}

pub(super) fn list(
    value: &'static StaticProgramValue,
    label: &'static str,
) -> Result<&'static [StaticProgramValue], ValidationError> {
    match value {
        StaticProgramValue::List(values) => Ok(values),
        _ => Err(schema_error_with_label(label)),
    }
}

pub(super) fn string(
    value: &'static StaticProgramValue,
    label: &'static str,
) -> Result<&'static str, ValidationError> {
    match value {
        StaticProgramValue::String(value) => Ok(value),
        _ => Err(schema_error_with_label(label)),
    }
}

pub(super) fn bool_value(
    value: &'static StaticProgramValue,
    label: &'static str,
) -> Result<bool, ValidationError> {
    match value {
        StaticProgramValue::Bool(value) => Ok(*value),
        _ => Err(schema_error_with_label(label)),
    }
}

fn integer_text(
    value: &'static StaticProgramValue,
    label: &'static str,
) -> Result<&'static str, ValidationError> {
    match value {
        StaticProgramValue::Integer(value) | StaticProgramValue::String(value) => Ok(value),
        _ => Err(schema_error_with_label(label)),
    }
}

pub(super) fn usize_value(
    value: &'static StaticProgramValue,
    label: &'static str,
) -> Result<usize, ValidationError> {
    integer_text(value, label)?
        .parse::<usize>()
        .map_err(|_| schema_error_with_label(label))
}

pub(super) fn optional_big_int(
    value: &'static StaticProgramValue,
) -> Result<Option<BigInt>, ValidationError> {
    if matches!(value, StaticProgramValue::None) {
        return Ok(None);
    }
    BigInt::from_str(integer_text(value, "integer constraint")?)
        .map(Some)
        .map_err(|_| schema_error("Static integer constraint is invalid"))
}

pub(super) fn optional_usize(
    value: &'static StaticProgramValue,
) -> Result<Option<usize>, ValidationError> {
    if matches!(value, StaticProgramValue::None) {
        return Ok(None);
    }
    usize_value(value, "length constraint").map(Some)
}

pub(super) fn optional_f64(
    value: &'static StaticProgramValue,
) -> Result<Option<f64>, ValidationError> {
    match value {
        StaticProgramValue::None => Ok(None),
        StaticProgramValue::FloatBits(bits) => Ok(Some(f64::from_bits(*bits))),
        StaticProgramValue::Integer(value) => value
            .parse::<f64>()
            .map(Some)
            .map_err(|_| schema_error("Static float constraint is invalid")),
        _ => Err(schema_error("Static float constraint is invalid")),
    }
}

pub(super) fn schema_error(message: &'static str) -> ValidationError {
    type_error("schema_invalid", message, "valid compiler-emitted schema")
}

fn schema_error_with_label(label: &'static str) -> ValidationError {
    ValidationError::one(
        ErrorDetail::new("schema_invalid", "Static schema value has an invalid type")
            .expected(label),
    )
}
