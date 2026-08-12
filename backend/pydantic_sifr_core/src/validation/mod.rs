mod collections;
mod error;
mod scalars;
mod schema;
mod special;
mod value;

pub use error::{ErrorDetail, LocationItem, ValidationError, ValidationLimits};
pub use schema::{
    BytesConstraints, CollectionConstraints, ComplexConstraints, DecimalConstraints,
    FloatConstraints, InputProfile, IntegerConstraints, IntegerTarget, PatternSchema,
    RelativeTimeConstraint, Schema, StringConstraints, StringPattern, TemporalKind, TemporalSchema,
};
pub use value::{
    DateTimeValue, DateValue, DurationValue, PatternValue, TimeValue, ValidatedArena,
    ValidatedValue, ValueId,
};

use std::time::{SystemTime, UNIX_EPOCH};

use crate::{Arena, InputArena, InputId, InputValue};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
            clock: ClockSnapshot::system_utc(),
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
        || options.limits.max_errors == 0
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
        let value = if let Some(result) =
            scalars::validate_scalar(schema, input, self.options.strict, self.options.profile)
        {
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
