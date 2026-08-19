use sifr_runtime::interop::structural::StructuralProject;

use crate::{
    InputArena, InputProfile, NativeValue, PreparedSchema, ValidatedArena, ValidatedValue,
    ValidationOptions, validation::validate_ref_for_serialization,
};

use super::{
    SerializationError, SerializationErrorKind, SerializationOptions, SerializationPlan,
    SerializerCallbackKind, SerializerCallbackPlan, SerializerWhenUsed,
    output::{apply_options, native_json_bytes, native_value, root_model_value, verify_shape},
};

pub trait SerializationCallbacks {
    fn invoke(&self, slot: usize, input: ValidatedArena) -> Result<InputArena, SerializationError>;
}

pub fn serializer_callback_error(message: impl Into<String>) -> SerializationError {
    SerializationError::new(
        SerializationErrorKind::Callback,
        format!("Serializer callback failed: {}", message.into()),
    )
}

pub fn serialize_structural_with_callbacks<T: StructuralProject>(
    schema: PreparedSchema<'_>,
    plan: &SerializationPlan,
    value: &T,
    options: &SerializationOptions,
    callbacks: &dyn SerializationCallbacks,
) -> Result<NativeValue, SerializationError> {
    serialize_with_callbacks(
        schema,
        plan,
        value,
        options,
        callbacks,
        SerializationMode::Structural,
    )
}

pub fn serialize_json_with_callbacks<T: StructuralProject>(
    schema: PreparedSchema<'_>,
    plan: &SerializationPlan,
    value: &T,
    options: &SerializationOptions,
    callbacks: &dyn SerializationCallbacks,
) -> Result<Vec<u8>, SerializationError> {
    let value = serialize_with_callbacks(
        schema,
        plan,
        value,
        options,
        callbacks,
        SerializationMode::Json,
    )?;
    native_json_bytes(
        &value,
        plan.integer_profile(),
        options.limits.max_integer_digits,
        "$",
    )?
    .ok_or_else(|| {
        SerializationError::new(
            SerializationErrorKind::UnsupportedJsonValue,
            "serializer callback produced a value that is not JSON-compatible",
        )
    })
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum SerializationMode {
    Structural,
    Json,
}

fn serialize_with_callbacks<T: StructuralProject>(
    schema: PreparedSchema<'_>,
    plan: &SerializationPlan,
    value: &T,
    options: &SerializationOptions,
    callbacks: &dyn SerializationCallbacks,
    mode: SerializationMode,
) -> Result<NativeValue, SerializationError> {
    verify_shape::<T>(plan)?;
    if schema.structural_identity() != plan.structural_identity() {
        return Err(SerializationError::new(
            SerializationErrorKind::ShapeMismatch,
            "serializer plan does not match the prepared schema",
        ));
    }
    let input = crate::project_structural_input(value, options.limits).map_err(|error| {
        let kind = match error {
            crate::NativeInputError::Limit(_) => SerializationErrorKind::Limit,
            _ => SerializationErrorKind::InvalidProjection,
        };
        SerializationError::new(kind, error.to_string())
    })?;
    let validation_options = ValidationOptions {
        profile: InputProfile::Native,
        ..ValidationOptions::default()
    };
    let validated = validate_ref_for_serialization(schema.schema(), &input, validation_options)
        .map_err(|error| {
            SerializationError::new(
                SerializationErrorKind::InvalidProjection,
                format!("serialization input does not match its sealed schema: {error}"),
            )
        })?;
    let mut output = native_value(&input, input.root())?;

    for callback in plan
        .callbacks()
        .iter()
        .filter(|callback| callback.kind() != SerializerCallbackKind::Model)
    {
        apply_callback(callback, &validated, &mut output, callbacks, mode)?;
    }
    for callback in plan
        .callbacks()
        .iter()
        .filter(|callback| callback.kind() == SerializerCallbackKind::Model)
    {
        if callback_enabled(callback.when_used(), &output, mode) {
            let callback_output = callbacks.invoke(
                callback.slot(),
                validated.selected(validated.root()).ok_or_else(|| {
                    SerializationError::new(
                        SerializationErrorKind::InvalidProjection,
                        "serialized model root is missing",
                    )
                })?,
            )?;
            output = native_value(&callback_output, callback_output.root())?;
        }
    }
    if plan.root_model() && matches!(output, NativeValue::Object(_)) {
        output = root_model_value(output)?;
    }
    apply_options(plan, &output, options, &mut Vec::new()).ok_or_else(|| {
        SerializationError::new(
            SerializationErrorKind::InvalidProjection,
            "serialization selection removed the root value",
        )
    })
}

fn apply_callback(
    callback: &SerializerCallbackPlan,
    validated: &ValidatedArena,
    output: &mut NativeValue,
    callbacks: &dyn SerializationCallbacks,
    mode: SerializationMode,
) -> Result<(), SerializationError> {
    match callback.kind() {
        SerializerCallbackKind::Field => {
            let field = model_field(validated, callback.target())?;
            let NativeValue::Object(entries) = output else {
                return Err(SerializationError::new(
                    SerializationErrorKind::InvalidProjection,
                    "field serializer requires a model output",
                ));
            };
            let Some((_, current)) = entries
                .iter_mut()
                .find(|(name, _)| name == callback.target())
            else {
                return Err(SerializationError::new(
                    SerializationErrorKind::InvalidProjection,
                    "field serializer target is missing from the projected model",
                ));
            };
            if !callback_enabled(callback.when_used(), current, mode) {
                return Ok(());
            }
            let input = if callback.takes_receiver() {
                validated
                    .tuple_root_with_cloned_item(validated.root(), field)
                    .map_err(|error| arena_error(&error.to_string()))?
            } else {
                validated.selected(field).ok_or_else(|| {
                    SerializationError::new(
                        SerializationErrorKind::InvalidProjection,
                        "field serializer input is missing",
                    )
                })?
            };
            let callback_output = callbacks.invoke(callback.slot(), input)?;
            *current = native_value(&callback_output, callback_output.root())?;
        }
        SerializerCallbackKind::Computed => {
            let current = NativeValue::Null;
            if !callback_enabled(callback.when_used(), &current, mode) {
                return Ok(());
            }
            let callback_output = callbacks.invoke(
                callback.slot(),
                validated.selected(validated.root()).ok_or_else(|| {
                    SerializationError::new(
                        SerializationErrorKind::InvalidProjection,
                        "computed field model input is missing",
                    )
                })?,
            )?;
            let computed = native_value(&callback_output, callback_output.root())?;
            let NativeValue::Object(entries) = output else {
                return Err(SerializationError::new(
                    SerializationErrorKind::InvalidProjection,
                    "computed field requires a model output",
                ));
            };
            entries.push((callback.target().to_owned(), computed));
        }
        SerializerCallbackKind::Model => {}
    }
    Ok(())
}

fn model_field(
    validated: &ValidatedArena,
    target: &str,
) -> Result<crate::ValueId, SerializationError> {
    let Some(ValidatedValue::Model(model)) = validated.get(validated.root()) else {
        return Err(SerializationError::new(
            SerializationErrorKind::InvalidProjection,
            "serializer root is not a model",
        ));
    };
    model
        .fields()
        .iter()
        .find_map(|(name, value)| (*name == target).then_some(*value))
        .ok_or_else(|| {
            SerializationError::new(
                SerializationErrorKind::InvalidProjection,
                "serializer field is missing from the validated model",
            )
        })
}

fn callback_enabled(
    when_used: SerializerWhenUsed,
    value: &NativeValue,
    mode: SerializationMode,
) -> bool {
    match when_used {
        SerializerWhenUsed::Always => true,
        SerializerWhenUsed::UnlessNone => !matches!(value, NativeValue::Null),
        SerializerWhenUsed::Json => mode == SerializationMode::Json,
        SerializerWhenUsed::JsonUnlessNone => {
            mode == SerializationMode::Json && !matches!(value, NativeValue::Null)
        }
    }
}

fn arena_error(message: &str) -> SerializationError {
    SerializationError::new(SerializationErrorKind::Limit, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_used_modes_cover_structural_json_and_null_combinations() {
        let value = NativeValue::String("value".to_owned());
        let null = NativeValue::Null;

        for mode in [SerializationMode::Structural, SerializationMode::Json] {
            assert!(callback_enabled(SerializerWhenUsed::Always, &value, mode));
            assert!(callback_enabled(SerializerWhenUsed::Always, &null, mode));
            assert!(callback_enabled(
                SerializerWhenUsed::UnlessNone,
                &value,
                mode
            ));
            assert!(!callback_enabled(
                SerializerWhenUsed::UnlessNone,
                &null,
                mode
            ));
        }

        assert!(!callback_enabled(
            SerializerWhenUsed::Json,
            &value,
            SerializationMode::Structural
        ));
        assert!(callback_enabled(
            SerializerWhenUsed::Json,
            &null,
            SerializationMode::Json
        ));
        assert!(!callback_enabled(
            SerializerWhenUsed::JsonUnlessNone,
            &value,
            SerializationMode::Structural
        ));
        assert!(callback_enabled(
            SerializerWhenUsed::JsonUnlessNone,
            &value,
            SerializationMode::Json
        ));
        assert!(!callback_enabled(
            SerializerWhenUsed::JsonUnlessNone,
            &null,
            SerializationMode::Json
        ));
    }
}
