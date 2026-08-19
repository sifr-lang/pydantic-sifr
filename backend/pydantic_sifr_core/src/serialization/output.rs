use core::fmt;

use sifr_runtime::SifrInt;
use sifr_runtime::interop::structural::StructuralProject;
use sifr_runtime::json::{
    JsonIntegerEncoding, JsonIntegerProfile, JsonIntegerRangeError, encode_integer_for_profile,
};

use crate::{InputArena, InputId, InputValue, NativeValue, ObjectKind, SequenceKind};

use super::{SelectionSegment, SerializationOptions, SerializationPlan, selection::selected};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SerializationErrorKind {
    ShapeMismatch,
    InvalidProjection,
    Limit,
    UnsupportedJsonValue,
    IntegerRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SerializationError {
    kind: SerializationErrorKind,
    message: String,
    integer_range: Option<JsonIntegerRangeError>,
}

impl SerializationError {
    #[must_use]
    pub const fn kind(&self) -> SerializationErrorKind {
        self.kind
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub fn integer_range_error(&self) -> Option<&JsonIntegerRangeError> {
        self.integer_range.as_ref()
    }

    pub(super) fn new(kind: SerializationErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            integer_range: None,
        }
    }

    pub(super) fn from_integer_range(error: JsonIntegerRangeError) -> Self {
        Self {
            kind: SerializationErrorKind::IntegerRange,
            message: error.to_string(),
            integer_range: Some(error),
        }
    }
}

impl fmt::Display for SerializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for SerializationError {}

pub fn serialize_structural<T: StructuralProject>(
    plan: &SerializationPlan,
    value: &T,
    options: &SerializationOptions,
) -> Result<NativeValue, SerializationError> {
    verify_shape::<T>(plan)?;
    let input = crate::project_structural_input(value, options.limits).map_err(|error| {
        let kind = match error {
            crate::NativeInputError::Limit(_) => SerializationErrorKind::Limit,
            _ => SerializationErrorKind::InvalidProjection,
        };
        SerializationError::new(kind, error.to_string())
    })?;
    let value = native_value(&input, input.root())?;
    apply_options(plan, &value, options, &mut Vec::new()).ok_or_else(|| {
        SerializationError::new(
            SerializationErrorKind::InvalidProjection,
            "serialization selection removed the root value",
        )
    })
}

pub(super) fn verify_shape<T: StructuralProject>(
    plan: &SerializationPlan,
) -> Result<(), SerializationError> {
    if T::shape_identity() == plan.structural_identity() {
        Ok(())
    } else {
        Err(SerializationError::new(
            SerializationErrorKind::ShapeMismatch,
            "serialization value does not match the prepared structural shape",
        ))
    }
}

fn native_value(input: &InputArena, id: InputId) -> Result<NativeValue, SerializationError> {
    let value = input.get(id).ok_or_else(|| {
        SerializationError::new(
            SerializationErrorKind::InvalidProjection,
            "structural projection references a missing value",
        )
    })?;
    match value {
        InputValue::Null => Ok(NativeValue::Null),
        InputValue::Bool(value) => Ok(NativeValue::Bool(*value)),
        InputValue::Integer(value) => Ok(NativeValue::Integer(value.clone())),
        InputValue::Float(value) => Ok(NativeValue::Float(*value)),
        InputValue::Decimal(value) => Ok(NativeValue::Decimal(value.clone())),
        InputValue::Complex { real, imaginary } => Ok(NativeValue::Complex {
            real: *real,
            imaginary: *imaginary,
        }),
        InputValue::String(value) => Ok(NativeValue::String(value.clone())),
        InputValue::Bytes(value) => Ok(NativeValue::Bytes(value.clone())),
        InputValue::Date(value) => Ok(NativeValue::Date(value.clone())),
        InputValue::Time(value) => Ok(NativeValue::Time(value.clone())),
        InputValue::DateTime(value) => Ok(NativeValue::DateTime(value.clone())),
        InputValue::Duration(value) => Ok(NativeValue::Duration(value.clone())),
        InputValue::Uuid(value) => Ok(NativeValue::Uuid(value.clone())),
        InputValue::Url(value) => Ok(NativeValue::Url(value.clone())),
        InputValue::Pattern { source, flags } => Ok(NativeValue::Pattern {
            source: source.clone(),
            flags: *flags,
        }),
        InputValue::Fraction {
            numerator,
            denominator,
        } => Ok(NativeValue::Fraction {
            numerator: numerator.clone(),
            denominator: denominator.clone(),
        }),
        InputValue::Sequence { kind, items } => {
            let values = items
                .iter()
                .map(|id| native_value(input, *id))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(match kind {
                SequenceKind::JsonArray | SequenceKind::List => NativeValue::List(values),
                SequenceKind::Tuple => NativeValue::Tuple(values),
                SequenceKind::Set => NativeValue::Set(values),
                SequenceKind::FrozenSet => NativeValue::FrozenSet(values),
            })
        }
        InputValue::Object { kind, entries } => {
            let values = entries
                .iter()
                .map(|(name, id)| Ok((name.clone(), native_value(input, *id)?)))
                .collect::<Result<Vec<_>, SerializationError>>()?;
            match kind {
                ObjectKind::JsonObject | ObjectKind::Object => Ok(NativeValue::Object(values)),
            }
        }
        InputValue::Mapping(entries) => Ok(NativeValue::Mapping(
            entries
                .iter()
                .map(|(key, value)| Ok((native_value(input, *key)?, native_value(input, *value)?)))
                .collect::<Result<Vec<_>, SerializationError>>()?,
        )),
    }
}

fn apply_options(
    plan: &SerializationPlan,
    value: &NativeValue,
    options: &SerializationOptions,
    path: &mut Vec<SelectionSegment>,
) -> Option<NativeValue> {
    match value {
        NativeValue::Object(entries) => {
            let mut output = Vec::with_capacity(entries.len());
            for (name, value) in entries {
                path.push(SelectionSegment::Field(name.clone()));
                let policy = plan.field_policy(path);
                let keep = policy.is_some() && selected(options, path);
                let excluded_by_value = (options.exclude_none && *value == NativeValue::Null)
                    || (options.exclude_defaults
                        && policy
                            .and_then(super::plan::FieldPolicy::default)
                            .is_some_and(|default| default == value));
                if keep
                    && !excluded_by_value
                    && let Some(value) = apply_options(plan, value, options, path)
                {
                    let output_name = if options.by_alias {
                        policy
                            .and_then(super::plan::FieldPolicy::alias)
                            .unwrap_or(name)
                    } else {
                        name
                    };
                    output.push((output_name.to_owned(), value));
                }
                path.pop();
            }
            Some(NativeValue::Object(output))
        }
        NativeValue::List(values)
        | NativeValue::Tuple(values)
        | NativeValue::Set(values)
        | NativeValue::FrozenSet(values) => {
            let output = values
                .iter()
                .enumerate()
                .filter_map(|(index, value)| {
                    path.push(SelectionSegment::Index(index));
                    let output = selected(options, path)
                        .then(|| apply_options(plan, value, options, path))
                        .flatten();
                    path.pop();
                    output
                })
                .collect();
            Some(match value {
                NativeValue::List(_) => NativeValue::List(output),
                NativeValue::Tuple(_) => NativeValue::Tuple(output),
                NativeValue::Set(_) => NativeValue::Set(output),
                NativeValue::FrozenSet(_) => NativeValue::FrozenSet(output),
                _ => return None,
            })
        }
        NativeValue::Mapping(entries) => Some(NativeValue::Mapping(
            entries
                .iter()
                .filter_map(|(key, value)| {
                    Some((
                        apply_options(plan, key, options, path)?,
                        apply_options(plan, value, options, path)?,
                    ))
                })
                .collect(),
        )),
        _ => Some(value.clone()),
    }
}

pub(super) fn native_json_bytes(
    value: &NativeValue,
    profile: JsonIntegerProfile,
    max_integer_digits: usize,
    path: &str,
) -> Result<Option<Vec<u8>>, SerializationError> {
    let mut output = Vec::new();
    if write_native_json(value, &mut output, profile, max_integer_digits, path)? {
        Ok(Some(output))
    } else {
        Ok(None)
    }
}

fn write_native_json(
    value: &NativeValue,
    output: &mut Vec<u8>,
    profile: JsonIntegerProfile,
    max_integer_digits: usize,
    path: &str,
) -> Result<bool, SerializationError> {
    match value {
        NativeValue::Null => output.extend_from_slice(b"null"),
        NativeValue::Bool(true) => output.extend_from_slice(b"true"),
        NativeValue::Bool(false) => output.extend_from_slice(b"false"),
        NativeValue::Integer(value) => {
            let integer = SifrInt::parse_decimal(value, max_integer_digits).map_err(|_| {
                SerializationError::new(
                    SerializationErrorKind::InvalidProjection,
                    "serializer default contains an invalid integer",
                )
            })?;
            match encode_integer_for_profile(&integer, profile, path)
                .map_err(SerializationError::from_integer_range)?
            {
                JsonIntegerEncoding::Number(value) => output.extend_from_slice(value.as_bytes()),
                JsonIntegerEncoding::DecimalString(value) => {
                    serde_json::to_writer(output, &value).map_err(|_| {
                        SerializationError::new(
                            SerializationErrorKind::InvalidProjection,
                            "serializer default string could not be encoded",
                        )
                    })?;
                }
            }
        }
        NativeValue::Float(value) if value.is_finite() => {
            serde_json::to_writer(output, value).map_err(|_| {
                SerializationError::new(
                    SerializationErrorKind::InvalidProjection,
                    "serializer default float could not be encoded",
                )
            })?;
        }
        NativeValue::String(value) => {
            serde_json::to_writer(output, value).map_err(|_| {
                SerializationError::new(
                    SerializationErrorKind::InvalidProjection,
                    "serializer default string could not be encoded",
                )
            })?;
        }
        NativeValue::List(values)
        | NativeValue::Tuple(values)
        | NativeValue::Set(values)
        | NativeValue::FrozenSet(values) => {
            output.push(b'[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                if !write_native_json(
                    value,
                    output,
                    profile,
                    max_integer_digits,
                    &format!("{path}[{index}]"),
                )? {
                    return Ok(false);
                }
            }
            output.push(b']');
        }
        NativeValue::Object(entries) => {
            output.push(b'{');
            for (index, (name, value)) in entries.iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                serde_json::to_writer(&mut *output, name).map_err(|_| {
                    SerializationError::new(
                        SerializationErrorKind::InvalidProjection,
                        "serializer default field name could not be encoded",
                    )
                })?;
                output.push(b':');
                if !write_native_json(
                    value,
                    output,
                    profile,
                    max_integer_digits,
                    &format!("{path}.{name}"),
                )? {
                    return Ok(false);
                }
            }
            output.push(b'}');
        }
        _ => return Ok(false),
    }
    Ok(true)
}
