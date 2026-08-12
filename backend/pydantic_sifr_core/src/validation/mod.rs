mod collections;
mod error;
mod scalars;
mod schema;
mod special;
mod value;

pub use error::{ErrorDetail, LocationItem, ValidationError, ValidationLimits};
pub use schema::{
    BytesConstraints, CollectionConstraints, ComplexConstraints, DecimalConstraints,
    FloatConstraints, FractionConstraints, InputProfile, IntegerConstraints, IntegerTarget,
    PatternCompileError, PatternSchema, RelativeTimeConstraint, Schema, StringConstraints,
    StringPattern, TemporalKind, TemporalSchema,
};
pub use value::{
    DateTimeValue, DateValue, DurationValue, PatternValue, TimeValue, ValidatedArena,
    ValidatedValue, ValueId,
};

use std::time::{SystemTime, UNIX_EPOCH};

use crate::{Arena, InputArena, InputId, InputValue};

const HARD_MAX_DEPTH: usize = 256;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ClockSnapshot {
    pub unix_seconds: i64,
    pub microsecond: u32,
}

impl ClockSnapshot {
    #[must_use]
    pub fn system_utc() -> Self {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => Self {
                unix_seconds: i64::try_from(duration.as_secs()).unwrap_or(i64::MAX),
                microsecond: duration.subsec_micros(),
            },
            Err(error) => {
                let duration = error.duration();
                Self {
                    unix_seconds: -i64::try_from(duration.as_secs()).unwrap_or(i64::MAX),
                    microsecond: duration.subsec_micros(),
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidationOptions {
    pub strict: bool,
    pub profile: InputProfile,
    pub limits: ValidationLimits,
    pub clock: ClockSnapshot,
}

impl Default for ValidationOptions {
    fn default() -> Self {
        Self {
            strict: false,
            profile: InputProfile::Native,
            limits: ValidationLimits::default(),
            clock: ClockSnapshot::default(),
        }
    }
}

pub fn validate(
    schema: &Schema,
    input: &InputArena,
    options: ValidationOptions,
) -> Result<ValidatedArena, ValidationError> {
    if options.limits.max_depth == 0
        || options.limits.max_collection_items == 0
        || options.limits.max_string_bytes == 0
        || options.limits.max_numeric_digits == 0
        || options.limits.max_decimal_exponent == 0
        || options.limits.max_errors == 0
        || options.limits.max_depth > HARD_MAX_DEPTH
    {
        return Err(scalars::type_error(
            "resource_limit",
            "Validation limits must be greater than zero",
            "positive limits",
        ));
    }
    if options.profile == InputProfile::Strings {
        check_strings_profile(input)?;
    }
    check_input_limits(input, options.limits)?;
    let mut state = ValidationState {
        input,
        values: Arena::new(),
        options,
    };
    let root = state.validate_node(schema, input.root(), 0)?;
    Ok(ValidatedArena::new(root, state.values))
}

pub(crate) struct ValidationState<'a> {
    input: &'a InputArena,
    values: Arena<ValidatedValue>,
    options: ValidationOptions,
}

impl ValidationState<'_> {
    pub(crate) fn validate_node(
        &mut self,
        schema: &Schema,
        input_id: InputId,
        depth: usize,
    ) -> Result<ValueId, ValidationError> {
        if depth > self.options.limits.max_depth {
            return Err(scalars::type_error(
                "recursion_limit",
                "Validation recursion limit exceeded",
                "bounded input",
            ));
        }
        let input = self.input.get(input_id).ok_or_else(|| {
            scalars::type_error(
                "internal_input",
                "Input arena index is invalid",
                "valid input arena",
            )
        })?;
        let value = if let Some(result) = scalars::validate_scalar(schema, input, self.options) {
            result?
        } else if let Some(result) = special::validate_special(
            schema,
            input,
            self.options.strict,
            self.options.profile,
            self.options.clock,
        ) {
            result?
        } else {
            return collections::validate_collection(self, schema, input_id, depth);
        };
        value::push_value(&mut self.values, value).map_err(|_| {
            scalars::type_error(
                "resource_limit",
                "Validated arena capacity exceeded",
                "bounded output",
            )
        })
    }
}

fn check_input_limits(input: &InputArena, limits: ValidationLimits) -> Result<(), ValidationError> {
    let mut pending = vec![(input.root(), 0_usize)];
    let mut string_bytes = 0_usize;
    while let Some((id, depth)) = pending.pop() {
        if depth > limits.max_depth {
            return Err(scalars::type_error(
                "recursion_limit",
                "Validation recursion limit exceeded",
                "bounded input",
            ));
        }
        let value = input.get(id).ok_or_else(|| {
            scalars::type_error(
                "internal_input",
                "Input arena index is invalid",
                "valid input arena",
            )
        })?;
        let byte_count = match value {
            InputValue::Integer(value) | InputValue::Decimal(value) | InputValue::String(value) => {
                value.len()
            }
            InputValue::Bytes(value) => value.len(),
            InputValue::Fraction {
                numerator,
                denominator,
            } => numerator.len().saturating_add(denominator.len()),
            InputValue::Array(children) => {
                check_collection_limit(children.len(), limits)?;
                pending.extend(children.iter().map(|id| (*id, depth + 1)));
                0
            }
            InputValue::Object(entries) => {
                check_collection_limit(entries.len(), limits)?;
                for (key, id) in entries {
                    string_bytes = string_bytes.saturating_add(key.len());
                    pending.push((*id, depth + 1));
                }
                0
            }
            InputValue::Mapping(entries) => {
                check_collection_limit(entries.len(), limits)?;
                for (key, value) in entries {
                    pending.push((*key, depth + 1));
                    pending.push((*value, depth + 1));
                }
                0
            }
            InputValue::Null
            | InputValue::Bool(_)
            | InputValue::Float(_)
            | InputValue::Complex { .. } => 0,
        };
        string_bytes = string_bytes.saturating_add(byte_count);
        if string_bytes > limits.max_string_bytes {
            return Err(scalars::type_error(
                "resource_limit",
                "Validation string byte limit exceeded",
                "bounded input",
            ));
        }
    }
    Ok(())
}

fn check_collection_limit(length: usize, limits: ValidationLimits) -> Result<(), ValidationError> {
    if length > limits.max_collection_items {
        Err(scalars::type_error(
            "resource_limit",
            "Validation collection item limit exceeded",
            "bounded input",
        ))
    } else {
        Ok(())
    }
}

fn check_strings_profile(input: &InputArena) -> Result<(), ValidationError> {
    let mut pending = vec![input.root()];
    while let Some(id) = pending.pop() {
        match input.get(id) {
            Some(InputValue::String(_)) => {}
            Some(InputValue::Array(children)) => pending.extend(children),
            Some(InputValue::Object(entries)) => {
                pending.extend(entries.iter().map(|(_, value)| value));
            }
            Some(InputValue::Mapping(entries)) => {
                for (key, value) in entries {
                    pending.push(*key);
                    pending.push(*value);
                }
            }
            Some(_) => {
                return Err(scalars::type_error(
                    "strings_type",
                    "Strings profile requires string scalar leaves",
                    "structural strings",
                ));
            }
            None => {
                return Err(scalars::type_error(
                    "internal_input",
                    "Input arena index is invalid",
                    "valid input arena",
                ));
            }
        }
    }
    Ok(())
}
