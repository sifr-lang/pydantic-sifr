use crate::InputId;

use super::{Schema, ValidationError, ValidationState, ValueId};

pub(crate) fn validate_collection(
    _state: &mut ValidationState<'_>,
    _schema: &Schema,
    _input: InputId,
    _depth: usize,
) -> Result<ValueId, ValidationError> {
    Err(super::scalars::type_error(
        "schema_kind",
        "Collection validator is not available",
        "scalar schema",
    ))
}
