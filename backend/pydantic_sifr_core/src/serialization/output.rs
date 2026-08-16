use core::fmt;

use sifr_runtime::interop::structural::StructuralProject;

use crate::{InputArena, InputId, InputValue, JsonLimits, NativeValue, ObjectKind, SequenceKind};

use super::SerializationPlan;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SerializationErrorKind {
    ShapeMismatch,
    InvalidProjection,
    Limit,
    UnsupportedJsonValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SerializationError {
    kind: SerializationErrorKind,
    message: String,
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

    pub(super) fn new(kind: SerializationErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
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
    limits: JsonLimits,
) -> Result<NativeValue, SerializationError> {
    verify_shape::<T>(plan)?;
    let input = crate::project_structural_input(value, limits).map_err(|error| {
        let kind = match error {
            crate::NativeInputError::Limit(_) => SerializationErrorKind::Limit,
            _ => SerializationErrorKind::InvalidProjection,
        };
        SerializationError::new(kind, error.to_string())
    })?;
    native_value(&input, input.root())
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
