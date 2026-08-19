use crate::{InputArena, InputId};

use super::{
    InputProfile, SchemaRef, ValidatedArena, ValidationError, ValidationState, ValueId,
    schema_view::SchemaTag,
};

pub trait ValidationCallbacks {
    fn invoke(&self, slot: usize, input: ValidatedArena) -> Result<InputArena, ValidationError>;
}

pub fn validator_callback_error(message: impl Into<String>) -> ValidationError {
    let message = message.into();
    ValidationError::one(
        super::ErrorDetail::new(
            "validator_error",
            format!("Validator callback failed: {message}"),
        )
        .expected("successful checked validator")
        .context("error", message),
    )
}

pub(crate) fn validate_function(
    state: &mut ValidationState<'_>,
    schema: SchemaRef<'_>,
    input_id: InputId,
    depth: usize,
) -> Result<ValueId, ValidationError> {
    if !matches!(
        schema.tag()?,
        SchemaTag::FunctionBefore | SchemaTag::FunctionAfter | SchemaTag::FunctionPlain
    ) || schema.child_count()? != 2
    {
        return Err(super::scalars::type_error(
            "schema_invalid",
            "Validator schema has an invalid shape",
            "two-stage validator schema",
        ));
    }
    let input_schema = schema.child(0)?;
    let output_schema = schema.child(1)?;
    let validated_input = state.validate_branch(input_schema, input_id, depth + 1)?;
    let callback_output = state.invoke_validator(schema.validator_slot()?, validated_input)?;
    let mut options = state.options();
    options.profile = InputProfile::Native;
    let validated_output = state.validate_input(
        output_schema,
        &callback_output,
        callback_output.root(),
        options,
        depth + 1,
    )?;
    state.import(validated_output)
}

#[cfg(test)]
mod tests {
    use super::validator_callback_error;

    #[test]
    fn callback_error_preserves_the_checked_error_as_context() {
        let error = validator_callback_error("code is rejected");
        let detail = &error.details()[0];

        assert_eq!(detail.code, "validator_error");
        assert_eq!(
            detail.message,
            "Validator callback failed: code is rejected"
        );
        assert_eq!(detail.expected, "successful checked validator");
        assert_eq!(
            detail.context.get("error").map(String::as_str),
            Some("code is rejected")
        );
    }
}
