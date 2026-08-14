use num_bigint::BigInt;
use sifr_runtime::interop::structural::{ShapeIdentity, primitive};

use crate::{InputArena, InputId, InputValue, SequenceKind};

use super::{
    AliasSegment, BytesConstraints, EnumValue, ErrorDetail, LiteralSchema, LiteralValue,
    LocationItem, Schema, SchemaErrorOverride, SchemaRef, StringConstraints, TaggedUnionSchema,
    UnionMode, UnionSchema, UnionValue, ValidatedArena, ValidatedValue, ValidationError,
    ValidationOptions, ValidationState, ValueId,
    sum_schema::{CanonicalSumLayout, nullable_layout},
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Exactness {
    Lax,
    Strict,
    Exact,
}

pub(crate) fn validate_sum(
    state: &mut ValidationState<'_>,
    schema: SchemaRef<'_>,
    input_id: InputId,
    depth: usize,
) -> Result<ValueId, ValidationError> {
    let SchemaRef::Owned(schema) = schema else {
        return super::static_sums::validate(state, schema, input_id, depth);
    };
    match schema {
        Schema::Literal(schema) => validate_literal(state, schema, input_id),
        Schema::Enum(schema) => {
            let input = state.input().get(input_id).ok_or_else(invalid_input)?;
            let (index, variant) = schema
                .variants()
                .iter()
                .enumerate()
                .find(|(_, variant)| input_matches_literal(&variant.input, input, state.options()))
                .ok_or_else(|| {
                    type_error("enum", "Input is not a declared enum value", "enum value")
                })?;
            let discriminant = state.push(ValidatedValue::FixedInt {
                kind: "int64",
                value: BigInt::from(variant.discriminant),
            })?;
            state.push(ValidatedValue::Enum(EnumValue {
                name: schema.name(),
                variant: variant.name,
                index,
                discriminant,
            }))
        }
        Schema::Nullable(inner) => validate_nullable(state, inner, input_id, depth),
        Schema::Union(schema) => validate_union(state, schema, input_id, depth),
        Schema::TaggedUnion(schema) => validate_tagged_union(state, schema, input_id, depth),
        _ => Err(type_error(
            "schema_invalid",
            "Schema node is not a sum schema",
            "sum schema",
        )),
    }
}

fn validate_literal(
    state: &mut ValidationState<'_>,
    schema: &LiteralSchema,
    input_id: InputId,
) -> Result<ValueId, ValidationError> {
    let input = state.input().get(input_id).ok_or_else(invalid_input)?;
    let Some(expected) = schema
        .values()
        .iter()
        .find(|expected| input_matches_literal(expected, input, state.options()))
    else {
        return Err(literal_error(schema));
    };
    let value = state.push(validated_literal(expected))?;
    wrap_existing(
        state,
        schema.layout(),
        literal_identity(expected),
        Some(value),
    )
}

fn validate_nullable(
    state: &mut ValidationState<'_>,
    inner: &Schema,
    input_id: InputId,
    depth: usize,
) -> Result<ValueId, ValidationError> {
    let layout = nullable_layout(inner)?;
    if matches!(state.input().get(input_id), Some(InputValue::Null)) {
        return wrap_existing(state, &layout, primitive("None"), None);
    }
    let candidate = state.validate_branch(SchemaRef::owned(inner), input_id, depth + 1)?;
    let selected = selected_member(inner, &candidate)?;
    import_selected(state, &layout, candidate, selected)
}

fn validate_union(
    state: &mut ValidationState<'_>,
    schema: &UnionSchema,
    input_id: InputId,
    depth: usize,
) -> Result<ValueId, ValidationError> {
    if schema.choices().len() == 1 && schema.auto_collapse() {
        let choice = &schema.choices()[0];
        let candidate =
            state.validate_branch(SchemaRef::owned(&choice.schema), input_id, depth + 1)?;
        let selected = selected_member(&choice.schema, &candidate)?;
        return import_selected(state, schema.layout(), candidate, selected);
    }
    let mut errors = None;
    let mut best: Option<(Option<usize>, Exactness, SelectedMember, ValidatedArena)> = None;
    for choice in schema.choices() {
        match state.validate_branch(SchemaRef::owned(&choice.schema), input_id, depth + 1) {
            Ok(candidate) if schema.mode() == UnionMode::LeftToRight => {
                let selected = selected_member(&choice.schema, &candidate)?;
                return import_selected(state, schema.layout(), candidate, selected);
            }
            Ok(candidate) => {
                let field_count = validated_field_score(&candidate);
                let exactness = candidate_exactness(&choice.schema, state, input_id);
                let selected = selected_member(&choice.schema, &candidate)?;
                if exactness == Exactness::Exact && field_count.is_none() {
                    return import_selected(state, schema.layout(), candidate, selected);
                }
                if best.as_ref().is_none_or(|(fields, exact, _, _)| {
                    candidate_is_better(field_count, exactness, *fields, *exact)
                }) {
                    best = Some((field_count, exactness, selected, candidate));
                }
            }
            Err(error) => collect_branch_error(
                &mut errors,
                error.at(LocationItem::Branch(choice.label.to_owned())),
                state.options().limits.max_errors,
            ),
        }
    }
    if let Some((_, _, selected, candidate)) = best {
        import_selected(state, schema.layout(), candidate, selected)
    } else {
        Err(apply_override(
            errors.unwrap_or_else(|| {
                type_error(
                    "union",
                    "Input does not match a union choice",
                    "union choice",
                )
            }),
            schema.error(),
        ))
    }
}

fn validate_tagged_union(
    state: &mut ValidationState<'_>,
    schema: &TaggedUnionSchema,
    input_id: InputId,
    depth: usize,
) -> Result<ValueId, ValidationError> {
    let tag_id =
        resolve_path(state, input_id, &schema.discriminator().segments).ok_or_else(|| {
            apply_override(
                type_error(
                    "union_tag_not_found",
                    "Tagged union discriminator is missing",
                    "declared discriminator path",
                ),
                schema.error(),
            )
        })?;
    let tag_input = state.input().get(tag_id).ok_or_else(invalid_input)?;
    let choice = schema
        .choices()
        .iter()
        .find(|choice| {
            choice
                .tags
                .iter()
                .any(|tag| input_matches_literal(tag, tag_input, state.options()))
        })
        .ok_or_else(|| {
            apply_override(
                type_error(
                    "union_tag_invalid",
                    "Tagged union discriminator does not select a choice",
                    "declared discriminator value",
                ),
                schema.error(),
            )
        })?;
    let candidate = state
        .validate_branch(SchemaRef::owned(&choice.schema), input_id, depth + 1)
        .map_err(|error| {
            apply_override(
                error.at(LocationItem::Branch(choice.label.to_owned())),
                schema.error(),
            )
        })?;
    let selected = selected_member(&choice.schema, &candidate)?;
    import_selected(state, schema.layout(), candidate, selected)
}

fn resolve_path(
    state: &ValidationState<'_>,
    root: InputId,
    path: &[AliasSegment],
) -> Option<InputId> {
    let mut current = root;
    for segment in path {
        current = match (segment, state.input().get(current)?) {
            (AliasSegment::Field(field), InputValue::Object { entries, .. }) => entries
                .iter()
                .find(|(name, _)| name == field)
                .map(|(_, value)| *value)?,
            (AliasSegment::Index(index), InputValue::Sequence { items, .. }) => {
                *items.get(*index)?
            }
            _ => return None,
        };
    }
    Some(current)
}

#[derive(Clone, Copy)]
struct SelectedMember {
    identity: ShapeIdentity,
    value: Option<ValueId>,
}

fn selected_member(
    schema: &Schema,
    arena: &ValidatedArena,
) -> Result<SelectedMember, ValidationError> {
    match schema {
        Schema::Literal(schema) => selected_from_layout(schema.layout(), arena),
        Schema::Union(schema) => selected_from_layout(schema.layout(), arena),
        Schema::TaggedUnion(schema) => selected_from_layout(schema.layout(), arena),
        Schema::Nullable(inner) => selected_from_layout(&nullable_layout(inner)?, arena),
        Schema::EmbeddedJson(inner) => selected_member(inner, arena),
        _ => Ok(SelectedMember {
            identity: schema.structural_identity_at(0)?,
            value: Some(arena.root()),
        }),
    }
}

fn selected_from_layout(
    layout: &CanonicalSumLayout,
    arena: &ValidatedArena,
) -> Result<SelectedMember, ValidationError> {
    if layout.is_direct() {
        return Ok(SelectedMember {
            identity: layout.members()[0].identity,
            value: Some(arena.root()),
        });
    }
    if layout.is_optional() {
        return match arena.get(arena.root()) {
            Some(ValidatedValue::Nullable(None)) => Ok(SelectedMember {
                identity: primitive("None"),
                value: None,
            }),
            Some(ValidatedValue::Nullable(Some(value))) => Ok(SelectedMember {
                identity: layout.members()[1].identity,
                value: Some(*value),
            }),
            _ => Err(invalid_sum_output()),
        };
    }
    let Some(ValidatedValue::Union(value)) = arena.get(arena.root()) else {
        return Err(invalid_sum_output());
    };
    let member = layout
        .members()
        .get(value.index())
        .ok_or_else(invalid_sum_output)?;
    Ok(SelectedMember {
        identity: member.identity,
        value: Some(value.value()),
    })
}

fn import_selected(
    state: &mut ValidationState<'_>,
    layout: &CanonicalSumLayout,
    candidate: ValidatedArena,
    selected: SelectedMember,
) -> Result<ValueId, ValidationError> {
    let value = match selected.value {
        Some(value) => Some(state.import_at(candidate, value)?),
        None => None,
    };
    wrap_existing(state, layout, selected.identity, value)
}

fn wrap_existing(
    state: &mut ValidationState<'_>,
    layout: &CanonicalSumLayout,
    identity: ShapeIdentity,
    value: Option<ValueId>,
) -> Result<ValueId, ValidationError> {
    if layout.is_direct() {
        return value.map_or_else(|| state.push(ValidatedValue::None), Ok);
    }
    if layout.is_optional() {
        return if identity == primitive("None") {
            state.push(ValidatedValue::Nullable(None))
        } else {
            let value = value.ok_or_else(invalid_sum_output)?;
            state.push(ValidatedValue::Nullable(Some(value)))
        };
    }
    let index = layout.index_of(identity).ok_or_else(invalid_sum_output)?;
    let value = match value {
        Some(value) => value,
        None => state.push(ValidatedValue::None)?,
    };
    state.push(ValidatedValue::Union(UnionValue { index, value }))
}

fn invalid_sum_output() -> ValidationError {
    type_error(
        "schema_invalid",
        "Sum validation output does not match its canonical layout",
        "canonical sum output",
    )
}

fn candidate_exactness(
    schema: &Schema,
    state: &ValidationState<'_>,
    input_id: InputId,
) -> Exactness {
    if input_matches(schema, state.input(), input_id, MatchLevel::Exact, 0) {
        Exactness::Exact
    } else if input_matches(schema, state.input(), input_id, MatchLevel::Strict, 0) {
        Exactness::Strict
    } else {
        Exactness::Lax
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum MatchLevel {
    Strict,
    Exact,
}

fn input_matches(
    schema: &Schema,
    input: &InputArena,
    input_id: InputId,
    level: MatchLevel,
    depth: usize,
) -> bool {
    if depth > 256 {
        return false;
    }
    let Some(value) = input.get(input_id) else {
        return false;
    };
    match schema {
        Schema::None => matches!(value, InputValue::Null),
        Schema::Bool => matches!(value, InputValue::Bool(_)),
        Schema::Integer { .. } => matches!(value, InputValue::Integer(_)),
        Schema::Float(_) => {
            matches!(value, InputValue::Float(_))
                || (level == MatchLevel::Strict && matches!(value, InputValue::Integer(_)))
        }
        Schema::Decimal(_) => matches!(value, InputValue::Decimal(_)),
        Schema::Fraction(_) => matches!(value, InputValue::Fraction { .. }),
        Schema::Complex(_) => matches!(value, InputValue::Complex { .. }),
        Schema::String(_) => matches!(value, InputValue::String(_)),
        Schema::Bytes(_) => matches!(value, InputValue::Bytes(_)),
        Schema::Temporal(schema) => match schema.kind {
            super::TemporalKind::Date => matches!(value, InputValue::Date(_)),
            super::TemporalKind::Time => matches!(value, InputValue::Time(_)),
            super::TemporalKind::DateTime => matches!(value, InputValue::DateTime(_)),
            super::TemporalKind::Duration => matches!(value, InputValue::Duration(_)),
        },
        Schema::Uuid { .. } => matches!(value, InputValue::Uuid(_)),
        Schema::Url(_) => matches!(value, InputValue::Url(_)),
        Schema::Pattern(_) => matches!(value, InputValue::Pattern { .. }),
        Schema::Literal(schema) => {
            input_literal(value).is_some_and(|value| schema.values().contains(&value))
        }
        Schema::Enum(schema) => input_literal(value).is_some_and(|value| {
            schema
                .variants()
                .iter()
                .any(|variant| variant.input == value)
        }),
        Schema::Nullable(inner) => {
            matches!(value, InputValue::Null)
                || input_matches(inner, input, input_id, level, depth + 1)
        }
        Schema::Union(schema) => schema
            .choices()
            .iter()
            .any(|choice| input_matches(&choice.schema, input, input_id, level, depth + 1)),
        Schema::TaggedUnion(schema) => schema
            .choices()
            .iter()
            .any(|choice| input_matches(&choice.schema, input, input_id, level, depth + 1)),
        Schema::Definitions(schema) => {
            input_matches(schema.root(), input, input_id, level, depth + 1)
        }
        Schema::DefinitionRef { .. } => false,
        Schema::Model(model) => model_input_matches(model, input, input_id, level, depth + 1),
        Schema::List { item, .. } | Schema::Generator { item, .. } => {
            sequence_matches(item, SequenceKind::List, input, value, level, depth + 1)
        }
        Schema::Tuple(items) => match value {
            InputValue::Sequence {
                kind: SequenceKind::Tuple,
                items: input_items,
            } if items.len() == input_items.len() => items
                .iter()
                .zip(input_items)
                .all(|(schema, id)| input_matches(schema, input, *id, level, depth + 1)),
            _ => false,
        },
        Schema::Set { item, .. } => {
            sequence_matches(item, SequenceKind::Set, input, value, level, depth + 1)
        }
        Schema::FrozenSet { item, .. } => sequence_matches(
            item,
            SequenceKind::FrozenSet,
            input,
            value,
            level,
            depth + 1,
        ),
        Schema::Mapping {
            key, value: item, ..
        } => match value {
            InputValue::Mapping(entries) => entries.iter().all(|(key_id, value_id)| {
                input_matches(key, input, *key_id, level, depth + 1)
                    && input_matches(item, input, *value_id, level, depth + 1)
            }),
            InputValue::Object { entries, .. } if matches!(key.as_ref(), Schema::String(_)) => {
                entries
                    .iter()
                    .all(|(_, value_id)| input_matches(item, input, *value_id, level, depth + 1))
            }
            _ => false,
        },
        Schema::EmbeddedJson(_) => {
            matches!(value, InputValue::String(_) | InputValue::Bytes(_))
        }
    }
}

fn sequence_matches(
    schema: &Schema,
    expected_kind: SequenceKind,
    input: &InputArena,
    value: &InputValue,
    level: MatchLevel,
    depth: usize,
) -> bool {
    match value {
        InputValue::Sequence { kind, items } if *kind == expected_kind => items
            .iter()
            .all(|id| input_matches(schema, input, *id, level, depth)),
        _ => false,
    }
}

fn model_input_matches(
    model: &super::ModelSchema,
    input: &InputArena,
    input_id: InputId,
    level: MatchLevel,
    depth: usize,
) -> bool {
    let Some(InputValue::Object { entries, .. }) = input.get(input_id) else {
        return false;
    };
    model
        .fields
        .iter()
        .filter(|field| field.input)
        .all(|field| {
            let selected = field
                .validation_aliases
                .iter()
                .find_map(|alias| resolve_path_in_arena(input, input_id, &alias.segments))
                .or_else(|| {
                    (field.validation_aliases.is_empty() || model.populate_by_name)
                        .then(|| {
                            entries
                                .iter()
                                .find(|(name, _)| name == field.name)
                                .map(|(_, value)| *value)
                        })
                        .flatten()
                });
            selected.map_or(field.default.is_some(), |value| {
                input_matches(&field.schema, input, value, level, depth)
            })
        })
}

fn resolve_path_in_arena(
    input: &InputArena,
    root: InputId,
    path: &[AliasSegment],
) -> Option<InputId> {
    let mut current = root;
    for segment in path {
        current = match (segment, input.get(current)?) {
            (AliasSegment::Field(field), InputValue::Object { entries, .. }) => entries
                .iter()
                .find(|(name, _)| name == field)
                .map(|(_, value)| *value)?,
            (AliasSegment::Index(index), InputValue::Sequence { items, .. }) => {
                *items.get(*index)?
            }
            _ => return None,
        };
    }
    Some(current)
}

pub(super) fn validated_field_score(arena: &ValidatedArena) -> Option<usize> {
    let mut total = 0_usize;
    let mut found_model = false;
    let mut pending = vec![arena.root()];
    while let Some(id) = pending.pop() {
        match arena.get(id) {
            Some(ValidatedValue::Model(model)) => {
                found_model = true;
                total = total.saturating_add(model.validated_field_count());
                pending.extend(model.fields().iter().map(|(_, value)| *value));
            }
            Some(ValidatedValue::Sequence(items))
            | Some(ValidatedValue::Tuple(items))
            | Some(ValidatedValue::Set(items))
            | Some(ValidatedValue::FrozenSet(items)) => pending.extend(items),
            Some(ValidatedValue::Mapping(entries)) => {
                pending.extend(entries.iter().flat_map(|(key, value)| [*key, *value]));
            }
            Some(ValidatedValue::Nullable(Some(value))) => pending.push(*value),
            Some(ValidatedValue::Union(value)) => pending.push(value.value()),
            _ => {}
        }
    }
    found_model.then_some(total)
}

pub(super) const fn candidate_is_better(
    fields: Option<usize>,
    exactness: Exactness,
    best_fields: Option<usize>,
    best_exactness: Exactness,
) -> bool {
    match (fields, best_fields) {
        (Some(fields), Some(best_fields)) if fields != best_fields => fields > best_fields,
        _ => exactness as u8 > best_exactness as u8,
    }
}

pub(super) fn input_literal(input: &InputValue) -> Option<LiteralValue> {
    match input {
        InputValue::Null => Some(LiteralValue::None),
        InputValue::Bool(value) => Some(LiteralValue::Bool(*value)),
        InputValue::Integer(value) => value.parse().ok().map(LiteralValue::Integer),
        InputValue::String(value) => Some(LiteralValue::String(value.clone())),
        InputValue::Bytes(value) => Some(LiteralValue::Bytes(value.clone())),
        _ => None,
    }
}

pub(super) fn input_matches_literal(
    expected: &LiteralValue,
    input: &InputValue,
    options: ValidationOptions,
) -> bool {
    if options.profile != super::InputProfile::Strings {
        return input_literal(input).as_ref() == Some(expected);
    }
    let schema = match expected {
        LiteralValue::None => Schema::None,
        LiteralValue::Bool(_) => Schema::Bool,
        LiteralValue::Integer(_) => Schema::exact_integer(),
        LiteralValue::String(_) => Schema::String(StringConstraints::default()),
        LiteralValue::Bytes(_) => Schema::Bytes(BytesConstraints::default()),
    };
    let Some(Ok(value)) =
        super::scalars::validate_scalar(SchemaRef::owned(&schema), input, options)
    else {
        return false;
    };
    match (expected, value) {
        (LiteralValue::None, ValidatedValue::None) => true,
        (LiteralValue::Bool(expected), ValidatedValue::Bool(actual)) => *expected == actual,
        (LiteralValue::Integer(expected), ValidatedValue::ExactInt(actual)) => *expected == actual,
        (LiteralValue::String(expected), ValidatedValue::String(actual)) => *expected == actual,
        (LiteralValue::Bytes(expected), ValidatedValue::Bytes(actual)) => *expected == actual,
        _ => false,
    }
}

pub(super) fn validated_literal(value: &LiteralValue) -> ValidatedValue {
    match value {
        LiteralValue::None => ValidatedValue::None,
        LiteralValue::Bool(value) => ValidatedValue::Bool(*value),
        LiteralValue::Integer(value) => ValidatedValue::ExactInt(value.clone()),
        LiteralValue::String(value) => ValidatedValue::String(value.clone()),
        LiteralValue::Bytes(value) => ValidatedValue::Bytes(value.clone()),
    }
}

fn literal_identity(value: &LiteralValue) -> ShapeIdentity {
    primitive(match value {
        LiteralValue::None => "None",
        LiteralValue::Bool(_) => "bool",
        LiteralValue::Integer(_) => "int",
        LiteralValue::String(_) => "str",
        LiteralValue::Bytes(_) => "bytes",
    })
}

fn literal_error(schema: &LiteralSchema) -> ValidationError {
    type_error(
        "literal_error",
        "Input does not match a declared literal",
        &format!("one of {} literal values", schema.values().len()),
    )
}

pub(super) fn collect_branch_error(
    errors: &mut Option<ValidationError>,
    error: ValidationError,
    limit: usize,
) {
    if let Some(errors) = errors {
        errors.append(error, limit);
    } else {
        *errors = Some(error);
    }
}

pub(super) fn apply_override(
    error: ValidationError,
    declaration: Option<SchemaErrorOverride>,
) -> ValidationError {
    declaration.map_or(error, |declaration| {
        ValidationError::one(
            ErrorDetail::new(declaration.code, declaration.message).expected("valid union input"),
        )
    })
}

pub(super) fn invalid_input() -> ValidationError {
    type_error(
        "internal_input",
        "Input arena index is invalid",
        "valid input arena",
    )
}

pub(super) fn type_error(
    code: &'static str,
    message: &'static str,
    expected: &str,
) -> ValidationError {
    ValidationError::one(ErrorDetail::new(code, message).expected(expected))
}
