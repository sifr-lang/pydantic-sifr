use crate::InputValue;

use super::{ClockSnapshot, InputProfile, Schema, ValidatedValue, ValidationError};

pub(crate) fn validate_special(
    _schema: &Schema,
    _input: &InputValue,
    _strict: bool,
    _profile: InputProfile,
    _clock: ClockSnapshot,
) -> Option<Result<ValidatedValue, ValidationError>> {
    None
}
