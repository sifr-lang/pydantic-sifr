use std::collections::BTreeSet;

use crate::{InputId, InputValue, JsonLimits, ObjectKind, build_native_input};

use super::{
    AliasPath, AliasSegment, ErrorDetail, ExtraPolicy, FieldDefault, InputProfile, LocationItem,
    ModelField, ModelSchema, ModelValue, ValidatedValue, ValidationError, ValidationState, ValueId,
    collections::{collect_error, stop_after_error_cap},
    validate_at_depth,
};

pub(crate) fn validate_model(
    state: &mut ValidationState<'_>,
    schema: &ModelSchema,
    input_id: InputId,
    depth: usize,
) -> Result<ValueId, ValidationError> {
    let (kind, entries) = match state.input().get(input_id) {
        Some(InputValue::Object { kind, entries }) => (*kind, entries.clone()),
        Some(_) => {
            return Err(type_error(
                "model_type",
                "Input must be an object",
                "object",
            ));
        }
        None => {
            return Err(type_error(
                "internal_input",
                "Input arena index is invalid",
                "valid input arena",
            ));
        }
    };
    if state.options().strict
        && !matches!(
            (state.options().profile, kind),
            (InputProfile::Json, ObjectKind::JsonObject)
                | (
                    InputProfile::Native | InputProfile::Strings,
                    ObjectKind::Object
                )
        )
    {
        return Err(type_error(
            "model_type",
            "Object kind does not match the input profile",
            "profile object",
        ));
    }

    let mut fields = Vec::with_capacity(schema.fields.len());
    let mut consumed = BTreeSet::new();
    let mut errors = None;
    let mut validated_field_count = 0;
    for (field_index, field) in schema.fields.iter().enumerate() {
        match select_field(state, schema, field, input_id, &entries) {
            Some((value_id, entry_index, location)) => {
                consumed.insert(entry_index);
                match state.validate_node(&field.schema, value_id, depth + 1) {
                    Ok(value) => {
                        fields.push((field.name.clone(), value));
                        validated_field_count += 1;
                    }
                    Err(error) => collect_error(
                        &mut errors,
                        at_path(error, &location),
                        state.options().limits.max_errors,
                    ),
                }
            }
            None => match validate_default(state, field, depth) {
                Ok(Some(value)) => fields.push((field.name.clone(), value)),
                Ok(None) => collect_error(
                    &mut errors,
                    ValidationError::one(
                        ErrorDetail::new("missing", "Field is required").expected("field value"),
                    )
                    .at(missing_location(schema, field)),
                    state.options().limits.max_errors,
                ),
                Err(error) => collect_error(
                    &mut errors,
                    error.at(LocationItem::Field(field.name.clone())),
                    state.options().limits.max_errors,
                ),
            },
        }
        let has_more_fields = field_index + 1 < schema.fields.len();
        let has_possible_extras =
            !matches!(schema.extra, ExtraPolicy::Ignore) && consumed.len() < entries.len();
        if stop_after_error_cap(state, &mut errors, has_more_fields || has_possible_extras) {
            break;
        }
    }

    let mut extras = Vec::new();
    if !errors
        .as_ref()
        .is_some_and(|error| error.is_full(state.options().limits.max_errors))
    {
        validate_extras(
            state,
            schema,
            &entries,
            &consumed,
            depth,
            &mut extras,
            &mut errors,
        );
    }
    if let Some(error) = errors {
        return Err(error);
    }
    state.push(ValidatedValue::Model(ModelValue::new(
        schema.name.clone(),
        fields,
        extras,
        validated_field_count,
    )))
}

fn select_field(
    state: &ValidationState<'_>,
    model: &ModelSchema,
    field: &ModelField,
    root: InputId,
    entries: &[(String, InputId)],
) -> Option<(InputId, usize, Vec<LocationItem>)> {
    for alias in &field.validation_aliases {
        if let Some(value) = resolve_alias(state, root, alias)
            && let Some(index) = top_level_index(alias, entries)
        {
            return Some((value, index, location_for_alias(model, field, alias)));
        }
    }
    if field.validation_aliases.is_empty() || model.populate_by_name {
        entries
            .iter()
            .enumerate()
            .find(|(_, (name, _))| name == &field.name)
            .map(|(index, (_, value))| {
                (*value, index, vec![LocationItem::Field(field.name.clone())])
            })
    } else {
        None
    }
}

fn resolve_alias(state: &ValidationState<'_>, root: InputId, path: &AliasPath) -> Option<InputId> {
    let mut current = root;
    for segment in &path.segments {
        current = match (segment, state.input().get(current)?) {
            (AliasSegment::Field(field), InputValue::Object { entries, .. }) => entries
                .iter()
                .find(|(name, _)| name == field)
                .map(|(_, id)| *id)?,
            (AliasSegment::Index(index), InputValue::Sequence { items, .. }) => {
                *items.get(*index)?
            }
            _ => return None,
        };
    }
    Some(current)
}

fn top_level_index(path: &AliasPath, entries: &[(String, InputId)]) -> Option<usize> {
    let Some(AliasSegment::Field(field)) = path.segments.first() else {
        return None;
    };
    entries.iter().position(|(name, _)| name == field)
}

fn location_for_alias(
    model: &ModelSchema,
    field: &ModelField,
    alias: &AliasPath,
) -> Vec<LocationItem> {
    if !model.location_by_alias {
        return vec![LocationItem::Field(field.name.clone())];
    }
    alias
        .segments
        .iter()
        .map(|segment| match segment {
            AliasSegment::Field(name) => LocationItem::Field(name.clone()),
            AliasSegment::Index(index) => LocationItem::Index(*index),
        })
        .collect()
}

fn missing_location(model: &ModelSchema, field: &ModelField) -> LocationItem {
    if model.location_by_alias
        && let Some(AliasSegment::Field(name)) = field
            .validation_aliases
            .first()
            .and_then(|path| path.segments.first())
    {
        return LocationItem::Field(name.clone());
    }
    LocationItem::Field(field.name.clone())
}

fn at_path(mut error: ValidationError, path: &[LocationItem]) -> ValidationError {
    for item in path.iter().rev() {
        error = error.at(item.clone());
    }
    error
}

fn validate_default(
    state: &mut ValidationState<'_>,
    field: &ModelField,
    depth: usize,
) -> Result<Option<ValueId>, ValidationError> {
    let Some(default) = &field.default else {
        return Ok(None);
    };
    let value = match default {
        FieldDefault::Static(value) => value.clone(),
        FieldDefault::Factory(factory) => factory(),
    };
    let limits = state.options().limits;
    let input = build_native_input(
        &value,
        JsonLimits {
            max_input_bytes: limits.max_string_bytes,
            max_depth: limits.max_depth,
            max_nodes: limits.max_collection_items,
            max_string_bytes: limits.max_string_bytes,
            max_integer_digits: limits.max_numeric_digits,
            max_collection_items: limits.max_collection_items,
        },
    )
    .map_err(|_| {
        type_error(
            "default_invalid",
            "Default input is invalid",
            "valid default",
        )
    })?;
    let mut options = state.options();
    options.profile = InputProfile::Native;
    let output = validate_at_depth(&field.schema, &input, input.root(), options, depth + 1)?;
    state.import(output).map(Some)
}

fn validate_extras(
    state: &mut ValidationState<'_>,
    model: &ModelSchema,
    entries: &[(String, InputId)],
    consumed: &BTreeSet<usize>,
    depth: usize,
    extras: &mut Vec<(String, ValueId)>,
    errors: &mut Option<ValidationError>,
) {
    for (index, (name, value_id)) in entries.iter().enumerate() {
        if consumed.contains(&index) {
            continue;
        }
        match &model.extra {
            ExtraPolicy::Ignore => {}
            ExtraPolicy::Forbid => collect_error(
                errors,
                ValidationError::one(
                    ErrorDetail::new("extra_forbidden", "Extra inputs are not permitted")
                        .expected("declared model field"),
                )
                .at(LocationItem::Field(name.clone())),
                state.options().limits.max_errors,
            ),
            ExtraPolicy::Allow { value_schema, .. } => {
                match state.validate_node(value_schema, *value_id, depth + 1) {
                    Ok(value) => extras.push((name.clone(), value)),
                    Err(error) => collect_error(
                        errors,
                        error.at(LocationItem::Field(name.clone())),
                        state.options().limits.max_errors,
                    ),
                }
            }
        }
        let has_more = ((index + 1)..entries.len()).any(|next| !consumed.contains(&next));
        if stop_after_error_cap(state, errors, has_more) {
            break;
        }
    }
}

fn type_error(
    code: &'static str,
    message: &'static str,
    expected: &'static str,
) -> ValidationError {
    super::scalars::type_error(code, message, expected)
}
