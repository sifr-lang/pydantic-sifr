use std::collections::{BTreeMap, BTreeSet};

use crate::{InputId, InputValue, JsonLimits, ObjectKind, build_native_input};

use super::{
    AliasPath, AliasSegment, ErrorDetail, FieldDefault, InputProfile, LocationItem, ModelValue,
    ValidatedValue, ValidationError, ValidationState, ValueId,
    collections::{collect_error, stop_after_error_cap},
    schema_view::{DefaultRef, ExtraRef, FieldRef, ModelRef},
};

pub(crate) fn validate_model(
    state: &mut ValidationState<'_>,
    schema: ModelRef<'_>,
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
    if state.options().effective_strict()
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

    let field_specs = schema.fields()?;
    let extra_policy = schema.extra()?;
    let mut field_values = BTreeMap::new();
    let mut consumed = BTreeSet::new();
    let mut errors = None;
    let mut validated_field_count = 0;
    for (field_index, field) in field_specs.iter().copied().enumerate() {
        let field_name = field.name()?;
        if !field.input() {
            if field.default()?.is_some() {
                match validate_default(state, field, depth) {
                    Ok(Some(value)) => {
                        field_values.insert(field_name, value);
                    }
                    Ok(None) => {}
                    Err(error) => collect_error(
                        &mut errors,
                        error.at(LocationItem::Field(field_name.to_owned())),
                        state.options().limits.max_errors,
                    ),
                }
            }
        } else {
            match select_field(state, schema, field, input_id, &entries)? {
                Some((value_id, entry_index, location)) => {
                    consumed.insert(entry_index);
                    let field_schema = field.schema()?;
                    let validated = if state.options().strict_override.is_none() {
                        match field.strict()? {
                            Some(strict) => {
                                let mut options = state.options();
                                options.strict_override = Some(strict);
                                state
                                    .validate_branch_with_options(
                                        field_schema,
                                        value_id,
                                        depth + 1,
                                        options,
                                    )
                                    .and_then(|arena| state.import(arena))
                            }
                            None => state.validate_node(field_schema, value_id, depth + 1),
                        }
                    } else {
                        state.validate_node(field_schema, value_id, depth + 1)
                    };
                    match validated {
                        Ok(value) => {
                            field_values.insert(field_name, value);
                            validated_field_count += 1;
                        }
                        Err(error) => collect_error(
                            &mut errors,
                            at_path(error, &location),
                            state.options().limits.max_errors,
                        ),
                    }
                }
                None if field.uses_construction_default()? => {}
                None => match validate_default(state, field, depth) {
                    Ok(Some(value)) => {
                        field_values.insert(field_name, value);
                    }
                    Ok(None) => collect_error(
                        &mut errors,
                        ValidationError::one(
                            ErrorDetail::new("missing", "Field is required")
                                .expected("field value"),
                        )
                        .at(missing_location(schema, field)?),
                        state.options().limits.max_errors,
                    ),
                    Err(error) => collect_error(
                        &mut errors,
                        error.at(LocationItem::Field(field_name.to_owned())),
                        state.options().limits.max_errors,
                    ),
                },
            }
        }
        let mut has_more_fields = false;
        for candidate in &field_specs[field_index + 1..] {
            if candidate.input() || candidate.default()?.is_some() {
                has_more_fields = true;
                break;
            }
        }
        let has_possible_extras =
            !matches!(extra_policy, ExtraRef::Ignore) && consumed.len() < entries.len();
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
            extra_policy,
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
    if let ExtraRef::Allow { destination, .. } = extra_policy {
        let mut entries = Vec::with_capacity(extras.len());
        for (name, value) in &extras {
            let key = state.push(ValidatedValue::String(name.clone()))?;
            entries.push((key, *value));
        }
        let mapping = state.push(ValidatedValue::Mapping(entries))?;
        field_values.insert(destination, mapping);
    }
    let mut ordered_fields = Vec::with_capacity(field_specs.len());
    for field in &field_specs {
        let name = field.name()?;
        if let Some(value) = field_values.get(name).copied() {
            ordered_fields.push((name, value));
        } else if !field.uses_construction_default()? {
            return Err(type_error(
                "schema_invalid",
                "A non-input model field has no value source",
                "default or extra destination field",
            ));
        }
    }
    state.push(ValidatedValue::Model(ModelValue::new(
        schema.name()?,
        ordered_fields,
        extras,
        validated_field_count,
    )))
}

fn select_field(
    state: &ValidationState<'_>,
    model: ModelRef<'_>,
    field: FieldRef<'_>,
    root: InputId,
    entries: &[(String, InputId)],
) -> Result<Option<(InputId, usize, Vec<LocationItem>)>, ValidationError> {
    let aliases = field.aliases()?;
    for alias in &aliases {
        if let Some(value) = resolve_alias(state, root, alias)
            && let Some(index) = top_level_index(alias, entries)
        {
            return Ok(Some((
                value,
                index,
                location_for_alias(model, field, alias)?,
            )));
        }
    }
    if aliases.is_empty() || model.populate_by_name()? {
        let field_name = field.name()?;
        Ok(entries
            .iter()
            .enumerate()
            .find(|(_, (name, _))| name == field_name)
            .map(|(index, (_, value))| {
                (
                    *value,
                    index,
                    vec![LocationItem::Field(field_name.to_owned())],
                )
            }))
    } else {
        Ok(None)
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
    model: ModelRef<'_>,
    field: FieldRef<'_>,
    alias: &AliasPath,
) -> Result<Vec<LocationItem>, ValidationError> {
    if !model.location_by_alias()? {
        return Ok(vec![LocationItem::Field(field.name()?.to_owned())]);
    }
    Ok(alias
        .segments
        .iter()
        .map(|segment| match segment {
            AliasSegment::Field(name) => LocationItem::Field((*name).to_owned()),
            AliasSegment::Index(index) => LocationItem::Index(*index),
        })
        .collect())
}

fn missing_location(
    model: ModelRef<'_>,
    field: FieldRef<'_>,
) -> Result<LocationItem, ValidationError> {
    let aliases = field.aliases()?;
    if model.location_by_alias()?
        && let Some(AliasSegment::Field(name)) =
            aliases.first().and_then(|path| path.segments.first())
    {
        return Ok(LocationItem::Field((*name).to_owned()));
    }
    Ok(LocationItem::Field(field.name()?.to_owned()))
}

fn at_path(mut error: ValidationError, path: &[LocationItem]) -> ValidationError {
    for item in path.iter().rev() {
        error = error.at(item.clone());
    }
    error
}

fn validate_default(
    state: &mut ValidationState<'_>,
    field: FieldRef<'_>,
    depth: usize,
) -> Result<Option<ValueId>, ValidationError> {
    let Some(default) = field.default()? else {
        return Ok(None);
    };
    let value = match default {
        DefaultRef::Owned(FieldDefault::Static(value)) => value.clone(),
        DefaultRef::Owned(FieldDefault::Factory(factory)) => factory(),
        DefaultRef::Static(value) => static_default(value)?,
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
    if options.strict_override.is_none() {
        options.strict_override = field.strict()?;
    }
    let output = state.validate_input(field.schema()?, &input, input.root(), options, depth + 1)?;
    state.import(output).map(Some)
}

pub(crate) fn static_default(
    value: &'static sifr_runtime::interop::structural::StaticProgramValue,
) -> Result<crate::NativeValue, ValidationError> {
    use sifr_runtime::interop::structural::StaticProgramValue;

    let value = match value {
        StaticProgramValue::None => crate::NativeValue::Null,
        StaticProgramValue::Bool(value) => crate::NativeValue::Bool(*value),
        StaticProgramValue::Integer(value) => crate::NativeValue::Integer((*value).to_owned()),
        StaticProgramValue::FloatBits(value) => crate::NativeValue::Float(f64::from_bits(*value)),
        StaticProgramValue::String(value) => crate::NativeValue::String((*value).to_owned()),
        StaticProgramValue::Bytes(value) => crate::NativeValue::Bytes((*value).to_vec()),
        StaticProgramValue::List(values) => crate::NativeValue::List(
            values
                .iter()
                .map(static_default)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        StaticProgramValue::Tuple(values) => crate::NativeValue::Tuple(
            values
                .iter()
                .map(static_default)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        _ => {
            return Err(type_error(
                "schema_invalid",
                "Static model defaults must be closed scalar or sequence values",
                "supported static default",
            ));
        }
    };
    Ok(value)
}

fn validate_extras(
    state: &mut ValidationState<'_>,
    extra: ExtraRef<'_>,
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
        match extra {
            ExtraRef::Ignore => {}
            ExtraRef::Forbid => collect_error(
                errors,
                ValidationError::one(
                    ErrorDetail::new("extra_forbidden", "Extra inputs are not permitted")
                        .expected("declared model field"),
                )
                .at(LocationItem::Field(name.clone())),
                state.options().limits.max_errors,
            ),
            ExtraRef::Allow { value_schema, .. } => {
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
