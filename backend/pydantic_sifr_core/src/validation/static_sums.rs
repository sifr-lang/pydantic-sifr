use num_bigint::BigInt;

use crate::{InputArena, InputId, InputValue, SequenceKind};

use super::{
    AliasSegment, EnumValue, LiteralValue, LocationItem, SchemaRef, ValidatedArena, ValidatedValue,
    ValidationError, ValidationState, ValueId,
    schema_view::{SchemaTag, StaticMetadata, StaticVariant},
    sums::{
        Exactness, apply_override, candidate_is_better, collect_branch_error, input_literal,
        input_matches_literal, invalid_input, type_error, validated_field_score, validated_literal,
    },
    validate_branch_at,
};

pub(super) fn validate(
    state: &mut ValidationState<'_>,
    schema: SchemaRef<'_>,
    input_id: InputId,
    depth: usize,
) -> Result<ValueId, ValidationError> {
    match schema.tag()? {
        SchemaTag::Literal => validate_literal(state, schema, input_id, depth),
        SchemaTag::Enum => validate_enum(state, schema, input_id),
        SchemaTag::Nullable => validate_nullable(state, schema, input_id, depth),
        SchemaTag::Union => validate_union(state, schema, input_id, depth),
        SchemaTag::TaggedUnion => validate_tagged_union(state, schema, input_id, depth),
        _ => Err(type_error(
            "schema_invalid",
            "Schema node is not a static sum schema",
            "static sum schema",
        )),
    }
}

fn validate_literal(
    state: &mut ValidationState<'_>,
    schema: SchemaRef<'_>,
    input_id: InputId,
    depth: usize,
) -> Result<ValueId, ValidationError> {
    let values = literal_values(&schema.static_metadata()?, "pydantic.literal")?;
    if values.is_empty() {
        return Err(type_error(
            "schema_invalid",
            "Static literal schema has no declared values",
            "declared literal values",
        ));
    }
    let input = state.input().get(input_id).ok_or_else(invalid_input)?;
    let expected = values
        .iter()
        .find(|value| input_matches_literal(value, input, state.options()))
        .ok_or_else(|| {
            type_error(
                "literal_error",
                "Input does not match a declared literal",
                "declared literal value",
            )
        })?;
    let child_count = schema.child_count()?;
    if child_count == 0 {
        if matches!(expected, LiteralValue::Integer(_))
            && let Ok((target, constraints)) = schema.integer()
        {
            let value =
                super::scalars::validate_integer(input, state.options(), target, &constraints)?;
            return state.push(value);
        }
        return state.push(validated_literal(expected));
    }
    if child_count == 1 {
        if matches!(expected, LiteralValue::None) {
            return state.push(ValidatedValue::Nullable(None));
        }
        let candidate = validate_branch_at(
            schema.child(0)?,
            state.input(),
            input_id,
            state.options(),
            depth + 1,
        )?;
        let value = state.import(candidate)?;
        return state.push(ValidatedValue::Nullable(Some(value)));
    }
    let index = (0..child_count)
        .find_map(|index| {
            schema
                .child(index)
                .ok()
                .and_then(|child| literal_matches_tag(expected, child.tag().ok()?).then_some(index))
        })
        .ok_or_else(|| {
            type_error(
                "schema_invalid",
                "Literal value has no structural union member",
                "matching literal member",
            )
        })?;
    let candidate = validate_branch_at(
        schema.child(index)?,
        state.input(),
        input_id,
        state.options(),
        depth + 1,
    )?;
    wrap_union(state, index, candidate)
}

fn validate_enum(
    state: &mut ValidationState<'_>,
    schema: SchemaRef<'_>,
    input_id: InputId,
) -> Result<ValueId, ValidationError> {
    let input = state.input().get(input_id).ok_or_else(invalid_input)?;
    let variants = schema.static_variants()?;
    let declared = enum_values(schema, &variants)?;
    let selected = variants
        .iter()
        .zip(&declared)
        .enumerate()
        .find(|(_, (_, value))| input_matches_literal(value, input, state.options()))
        .ok_or_else(|| type_error("enum", "Input is not a declared enum value", "enum value"))?;
    let discriminant = state.push(ValidatedValue::FixedInt {
        kind: "int64",
        value: BigInt::from(selected.1.0.discriminant),
    })?;
    state.push(ValidatedValue::Enum(EnumValue {
        name: schema.static_definition()?,
        variant: selected.1.0.name,
        index: selected.0,
        discriminant,
    }))
}

fn validate_nullable(
    state: &mut ValidationState<'_>,
    schema: SchemaRef<'_>,
    input_id: InputId,
    depth: usize,
) -> Result<ValueId, ValidationError> {
    if matches!(state.input().get(input_id), Some(InputValue::Null)) {
        return state.push(ValidatedValue::Nullable(None));
    }
    let candidate = validate_branch_at(
        schema.child(0)?,
        state.input(),
        input_id,
        state.options(),
        depth + 1,
    )?;
    let value = state.import(candidate)?;
    state.push(ValidatedValue::Nullable(Some(value)))
}

fn validate_union(
    state: &mut ValidationState<'_>,
    schema: SchemaRef<'_>,
    input_id: InputId,
    depth: usize,
) -> Result<ValueId, ValidationError> {
    let metadata = schema.static_metadata()?;
    let mode = metadata_value(&metadata, "pydantic.union.mode")?.unwrap_or("smart");
    if mode != "smart" && mode != "left_to_right" {
        return Err(type_error(
            "schema_invalid",
            "Static union mode is invalid",
            "smart or left_to_right",
        ));
    }
    let auto_collapse = metadata_bool(&metadata, "pydantic.union.auto_collapse", true)?;
    let count = schema.child_count()?;
    if count == 0 {
        return Err(type_error(
            "schema_invalid",
            "Static union has no choices",
            "nonempty union",
        ));
    }
    if count == 1 && auto_collapse {
        let candidate = validate_branch_at(
            schema.child(0)?,
            state.input(),
            input_id,
            state.options(),
            depth + 1,
        )?;
        return state.import(candidate);
    }
    let labels = union_labels(&metadata, count)?;
    let mut errors = None;
    let mut best: Option<(Option<usize>, Exactness, usize, ValidatedArena)> = None;
    for (index, label) in labels.iter().enumerate() {
        let choice = schema.child(index)?;
        match validate_branch_at(choice, state.input(), input_id, state.options(), depth + 1) {
            Ok(candidate) if mode == "left_to_right" => {
                return wrap_union(state, index, candidate);
            }
            Ok(candidate) => {
                let fields = validated_field_score(&candidate);
                let exactness = candidate_exactness(choice, state.input(), input_id);
                if exactness == Exactness::Exact && fields.is_none() {
                    return wrap_union(state, index, candidate);
                }
                if best
                    .as_ref()
                    .is_none_or(|(best_fields, best_exactness, _, _)| {
                        candidate_is_better(fields, exactness, *best_fields, *best_exactness)
                    })
                {
                    best = Some((fields, exactness, index, candidate));
                }
            }
            Err(error) => collect_branch_error(
                &mut errors,
                error.at(LocationItem::Branch((*label).to_owned())),
                state.options().limits.max_errors,
            ),
        }
    }
    if let Some((_, _, index, candidate)) = best {
        wrap_union(state, index, candidate)
    } else {
        Err(apply_override(
            errors.unwrap_or_else(|| {
                type_error(
                    "union",
                    "Input does not match a union choice",
                    "union choice",
                )
            }),
            schema.static_error()?,
        ))
    }
}

fn validate_tagged_union(
    state: &mut ValidationState<'_>,
    schema: SchemaRef<'_>,
    input_id: InputId,
    depth: usize,
) -> Result<ValueId, ValidationError> {
    let metadata = schema.static_metadata()?;
    let error = schema.static_error()?;
    let path = discriminator_path(&metadata)?;
    let tag_id = resolve_path(state.input(), input_id, &path).ok_or_else(|| {
        apply_override(
            type_error(
                "union_tag_not_found",
                "Tagged union discriminator is missing",
                "declared discriminator path",
            ),
            error,
        )
    })?;
    let input = state.input().get(tag_id).ok_or_else(invalid_input)?;
    let count = schema.child_count()?;
    let labels = union_labels(&metadata, count)?;
    let tags = discriminator_tags(&metadata, count)?;
    let index = tags
        .iter()
        .position(|values| {
            values
                .iter()
                .any(|value| input_matches_literal(value, input, state.options()))
        })
        .ok_or_else(|| {
            apply_override(
                type_error(
                    "union_tag_invalid",
                    "Tagged union discriminator does not select a choice",
                    "declared discriminator value",
                ),
                error,
            )
        })?;
    let candidate = validate_branch_at(
        schema.child(index)?,
        state.input(),
        input_id,
        state.options(),
        depth + 1,
    )
    .map_err(|error_value| {
        apply_override(
            error_value.at(LocationItem::Branch(labels[index].to_owned())),
            error,
        )
    })?;
    wrap_union(state, index, candidate)
}

fn wrap_union(
    state: &mut ValidationState<'_>,
    index: usize,
    candidate: ValidatedArena,
) -> Result<ValueId, ValidationError> {
    let value = state.import(candidate)?;
    state.push(ValidatedValue::Union(super::UnionValue { index, value }))
}

fn candidate_exactness(schema: SchemaRef<'_>, input: &InputArena, input_id: InputId) -> Exactness {
    if input_matches(schema, input, input_id, MatchLevel::Exact, 0) {
        Exactness::Exact
    } else if input_matches(schema, input, input_id, MatchLevel::Strict, 0) {
        Exactness::Strict
    } else {
        Exactness::Lax
    }
}

#[derive(Clone, Copy)]
enum MatchLevel {
    Strict,
    Exact,
}

fn input_matches(
    schema: SchemaRef<'_>,
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
    let Ok(tag) = schema.tag() else {
        return false;
    };
    match tag {
        SchemaTag::None => matches!(value, InputValue::Null),
        SchemaTag::Bool => matches!(value, InputValue::Bool(_)),
        SchemaTag::Integer => matches!(value, InputValue::Integer(_)),
        SchemaTag::Float => {
            matches!(value, InputValue::Float(_))
                || (matches!(level, MatchLevel::Strict) && matches!(value, InputValue::Integer(_)))
        }
        SchemaTag::Decimal => matches!(value, InputValue::Decimal(_)),
        SchemaTag::String => matches!(value, InputValue::String(_)),
        SchemaTag::Bytes => matches!(value, InputValue::Bytes(_)),
        SchemaTag::Literal => literal_values(
            &schema.static_metadata().unwrap_or_default(),
            "pydantic.literal",
        )
        .unwrap_or_default()
        .iter()
        .any(|expected| input_literal(value).as_ref() == Some(expected)),
        SchemaTag::Enum => schema.static_variants().is_ok_and(|variants| {
            enum_values(schema, &variants).is_ok_and(|values| {
                values
                    .iter()
                    .any(|expected| input_literal(value).as_ref() == Some(expected))
            })
        }),
        SchemaTag::Nullable => {
            matches!(value, InputValue::Null)
                || schema
                    .child(0)
                    .is_ok_and(|child| input_matches(child, input, input_id, level, depth + 1))
        }
        SchemaTag::Union | SchemaTag::TaggedUnion => {
            (0..schema.child_count().unwrap_or(0)).any(|index| {
                schema
                    .child(index)
                    .is_ok_and(|child| input_matches(child, input, input_id, level, depth + 1))
            })
        }
        SchemaTag::Model => model_input_matches(schema, input, input_id, level, depth + 1),
        SchemaTag::List | SchemaTag::Generator => {
            sequence_matches(schema, SequenceKind::List, input, value, level, depth + 1)
        }
        SchemaTag::Tuple => match value {
            InputValue::Sequence {
                kind: SequenceKind::Tuple,
                items,
            } if schema.child_count().ok() == Some(items.len()) => {
                items.iter().enumerate().all(|(index, id)| {
                    schema
                        .child(index)
                        .is_ok_and(|child| input_matches(child, input, *id, level, depth + 1))
                })
            }
            _ => false,
        },
        SchemaTag::Set => {
            sequence_matches(schema, SequenceKind::Set, input, value, level, depth + 1)
        }
        SchemaTag::Mapping => mapping_input_matches(schema, input, value, level, depth + 1),
        _ => false,
    }
}

fn model_input_matches(
    schema: SchemaRef<'_>,
    input: &InputArena,
    input_id: InputId,
    level: MatchLevel,
    depth: usize,
) -> bool {
    let Some(InputValue::Object { entries, .. }) = input.get(input_id) else {
        return false;
    };
    let Ok(model) = schema.model() else {
        return false;
    };
    let Ok(fields) = model.fields() else {
        return false;
    };
    fields
        .iter()
        .copied()
        .filter(|field| field.input())
        .all(|field| {
            let Ok(aliases) = field.aliases() else {
                return false;
            };
            let selected = aliases
                .iter()
                .find_map(|alias| resolve_path(input, input_id, &alias.segments))
                .or_else(|| {
                    (aliases.is_empty() || model.populate_by_name().unwrap_or(false))
                        .then(|| {
                            let name = field.name().ok()?;
                            entries
                                .iter()
                                .find(|(candidate, _)| candidate == name)
                                .map(|(_, value)| *value)
                        })
                        .flatten()
                });
            selected.map_or_else(
                || field.default().is_ok_and(|value| value.is_some()),
                |value| {
                    field.schema().is_ok_and(|field_schema| {
                        input_matches(field_schema, input, value, level, depth)
                    })
                },
            )
        })
}

fn mapping_input_matches(
    schema: SchemaRef<'_>,
    input: &InputArena,
    value: &InputValue,
    level: MatchLevel,
    depth: usize,
) -> bool {
    let (Ok(key), Ok(item)) = (schema.child(0), schema.child(1)) else {
        return false;
    };
    match value {
        InputValue::Mapping(entries) => entries.iter().all(|(key_id, value_id)| {
            input_matches(key, input, *key_id, level, depth)
                && input_matches(item, input, *value_id, level, depth)
        }),
        InputValue::Object { entries, .. } if key.tag().ok() == Some(SchemaTag::String) => entries
            .iter()
            .all(|(_, value_id)| input_matches(item, input, *value_id, level, depth)),
        _ => false,
    }
}

fn sequence_matches(
    schema: SchemaRef<'_>,
    expected: SequenceKind,
    input: &InputArena,
    value: &InputValue,
    level: MatchLevel,
    depth: usize,
) -> bool {
    match value {
        InputValue::Sequence { kind, items } if *kind == expected => {
            schema.child(0).is_ok_and(|child| {
                items
                    .iter()
                    .all(|id| input_matches(child, input, *id, level, depth))
            })
        }
        _ => false,
    }
}

fn literal_values(
    metadata: &[StaticMetadata],
    prefix: &str,
) -> Result<Vec<LiteralValue>, ValidationError> {
    metadata
        .iter()
        .filter(|item| item.key.starts_with(prefix))
        .map(|item| parse_literal(item, prefix))
        .collect()
}

fn enum_value(
    variant: &StaticVariant,
    declared: Option<&LiteralValue>,
) -> Result<LiteralValue, ValidationError> {
    let values = literal_values(&variant.metadata, "pydantic.enum")?;
    match values.as_slice() {
        [] => Ok(declared
            .cloned()
            .unwrap_or_else(|| LiteralValue::Integer(BigInt::from(variant.discriminant)))),
        [value] => Ok(value.clone()),
        _ => Err(type_error(
            "schema_invalid",
            "Enum variant has multiple declared input values",
            "one enum input value",
        )),
    }
}

fn enum_values(
    schema: SchemaRef<'_>,
    variants: &[StaticVariant],
) -> Result<Vec<LiteralValue>, ValidationError> {
    let declared_values = literal_values(&schema.static_metadata()?, "pydantic.enum")?;
    if !declared_values.is_empty() && declared_values.len() != variants.len() {
        return Err(type_error(
            "schema_invalid",
            "Enum input metadata does not match its variants",
            "one enum input value per variant",
        ));
    }
    variants
        .iter()
        .enumerate()
        .map(|(index, variant)| enum_value(variant, declared_values.get(index)))
        .collect()
}

fn parse_literal(metadata: &StaticMetadata, prefix: &str) -> Result<LiteralValue, ValidationError> {
    let kind = metadata.key.strip_prefix(prefix).unwrap_or_default();
    match kind {
        ".none" if metadata.value == "none" => Ok(LiteralValue::None),
        ".bool" if metadata.value == "true" => Ok(LiteralValue::Bool(true)),
        ".bool" if metadata.value == "false" => Ok(LiteralValue::Bool(false)),
        ".int" => metadata
            .value
            .parse()
            .map(LiteralValue::Integer)
            .map_err(|_| invalid_literal_metadata()),
        ".str" => Ok(LiteralValue::String(metadata.value.to_owned())),
        ".bytes" => decode_hex(metadata.value)
            .map(LiteralValue::Bytes)
            .ok_or_else(invalid_literal_metadata),
        _ => Err(invalid_literal_metadata()),
    }
}

fn discriminator_path(metadata: &[StaticMetadata]) -> Result<Vec<AliasSegment>, ValidationError> {
    let path = metadata
        .iter()
        .filter_map(|item| match item.key {
            "pydantic.discriminator.field" => Some(Ok(AliasSegment::Field(item.value))),
            "pydantic.discriminator.index" => Some(
                item.value
                    .parse()
                    .map(AliasSegment::Index)
                    .map_err(|_| invalid_literal_metadata()),
            ),
            _ => None,
        })
        .collect::<Result<Vec<_>, _>>()?;
    if path.is_empty() {
        Err(type_error(
            "schema_invalid",
            "Tagged union has no discriminator path",
            "discriminator path",
        ))
    } else {
        Ok(path)
    }
}

fn discriminator_tags(
    metadata: &[StaticMetadata],
    count: usize,
) -> Result<Vec<Vec<LiteralValue>>, ValidationError> {
    let mut output = vec![Vec::new(); count];
    let mut choice = None;
    for item in metadata {
        if item.key == "pydantic.discriminator.choice" {
            choice = Some(
                item.value
                    .parse::<usize>()
                    .map_err(|_| invalid_literal_metadata())?,
            );
        } else if item.key.starts_with("pydantic.discriminator.tag") {
            let index = choice.ok_or_else(invalid_literal_metadata)?;
            let tags = output.get_mut(index).ok_or_else(invalid_literal_metadata)?;
            tags.push(parse_literal(item, "pydantic.discriminator.tag")?);
        }
    }
    if output.iter().any(Vec::is_empty) {
        return Err(type_error(
            "schema_invalid",
            "Tagged union choice has no declared tag",
            "tag for every choice",
        ));
    }
    Ok(output)
}

fn union_labels(
    metadata: &[StaticMetadata],
    count: usize,
) -> Result<Vec<&'static str>, ValidationError> {
    let labels = metadata
        .iter()
        .filter(|item| item.key == "pydantic.union.label")
        .map(|item| item.value)
        .collect::<Vec<_>>();
    if labels.is_empty() {
        return Ok(vec!["member"; count]);
    }
    if labels.len() != count || labels.iter().any(|label| label.is_empty()) {
        return Err(type_error(
            "schema_invalid",
            "Static union labels do not match its choices",
            "one nonempty label per choice",
        ));
    }
    Ok(labels)
}

fn metadata_value(
    metadata: &[StaticMetadata],
    key: &str,
) -> Result<Option<&'static str>, ValidationError> {
    let values = metadata
        .iter()
        .filter(|item| item.key == key)
        .map(|item| item.value)
        .collect::<Vec<_>>();
    match values.as_slice() {
        [] => Ok(None),
        [value] => Ok(Some(*value)),
        _ => Err(type_error(
            "schema_invalid",
            "Static sum metadata key is duplicated",
            "one metadata value",
        )),
    }
}

fn metadata_bool(
    metadata: &[StaticMetadata],
    key: &str,
    default: bool,
) -> Result<bool, ValidationError> {
    match metadata_value(metadata, key)? {
        None => Ok(default),
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        Some(_) => Err(type_error(
            "schema_invalid",
            "Static sum boolean metadata is invalid",
            "true or false",
        )),
    }
}

fn literal_matches_tag(value: &LiteralValue, tag: SchemaTag) -> bool {
    matches!(
        (value, tag),
        (LiteralValue::None, SchemaTag::None)
            | (LiteralValue::Bool(_), SchemaTag::Bool)
            | (LiteralValue::Integer(_), SchemaTag::Integer)
            | (LiteralValue::String(_), SchemaTag::String)
            | (LiteralValue::Bytes(_), SchemaTag::Bytes)
    )
}

fn resolve_path(input: &InputArena, root: InputId, path: &[AliasSegment]) -> Option<InputId> {
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

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_digit(pair[0])?;
            let low = hex_digit(pair[1])?;
            Some((high << 4) | low)
        })
        .collect()
}

const fn hex_digit(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn invalid_literal_metadata() -> ValidationError {
    type_error(
        "schema_invalid",
        "Static sum literal metadata is invalid",
        "canonical literal metadata",
    )
}

#[cfg(test)]
#[path = "static_sums_tests.rs"]
mod tests;
