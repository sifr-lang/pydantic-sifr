use std::collections::{BTreeMap, BTreeSet};

use crate::{Arena, InputArena, InputId, InputValue, ObjectKind, SequenceKind};

use super::{
    InputProfile, SchemaRef, ValidatedArena, ValidatedValue, ValidationError, ValidationState,
    ValueId, schema_view::SchemaTag,
};

pub(crate) fn validate_control(
    state: &mut ValidationState<'_>,
    schema: SchemaRef<'_>,
    input_id: InputId,
    depth: usize,
) -> Result<ValueId, ValidationError> {
    match schema.tag()? {
        SchemaTag::LaxOrStrict => validate_lax_or_strict(state, schema, input_id, depth),
        SchemaTag::JsonOrStructural => validate_json_or_structural(state, schema, input_id, depth),
        SchemaTag::Chain => validate_chain(state, schema, input_id, depth),
        _ => Err(control_error(
            "Schema node is not a validation control",
            "validation control schema",
        )),
    }
}

fn validate_lax_or_strict(
    state: &mut ValidationState<'_>,
    schema: SchemaRef<'_>,
    input_id: InputId,
    depth: usize,
) -> Result<ValueId, ValidationError> {
    require_arity(schema, 2)?;
    let options = state.options();
    let strict = options
        .strict_override
        .unwrap_or(options.strict || schema.default_strict()?);
    let selected = schema.child(usize::from(strict))?;
    let arena = state.validate_branch(selected, input_id, depth + 1)?;
    state.import(arena)
}

fn validate_json_or_structural(
    state: &mut ValidationState<'_>,
    schema: SchemaRef<'_>,
    input_id: InputId,
    depth: usize,
) -> Result<ValueId, ValidationError> {
    require_arity(schema, 2)?;
    let selected = if state.options().profile == InputProfile::Json {
        schema.child(0)?
    } else {
        schema.child(1)?
    };
    let arena = state.validate_branch(selected, input_id, depth + 1)?;
    state.import(arena)
}

fn validate_chain(
    state: &mut ValidationState<'_>,
    schema: SchemaRef<'_>,
    input_id: InputId,
    depth: usize,
) -> Result<ValueId, ValidationError> {
    let count = schema.child_count()?;
    if count == 0 {
        return Err(control_error(
            "A typed chain must contain at least one step",
            "nonempty typed chain",
        ));
    }
    let mut output = state.validate_branch(schema.child(0)?, input_id, depth + 1)?;
    for index in 1..count {
        let input = validated_input(&output, state.options().profile)?;
        output =
            state.validate_chain_input(schema.child(index)?, &input, input.root(), depth + 1)?;
    }
    state.import(output)
}

fn require_arity(schema: SchemaRef<'_>, expected: usize) -> Result<(), ValidationError> {
    if schema.child_count()? == expected {
        Ok(())
    } else {
        Err(control_error(
            "Control schema has invalid child arity",
            "complete control branches",
        ))
    }
}

fn validated_input(
    source: &ValidatedArena,
    profile: InputProfile,
) -> Result<InputArena, ValidationError> {
    let mut builder = ChainInputBuilder {
        source,
        profile,
        values: Arena::new(),
        converted: BTreeMap::new(),
        active: BTreeSet::new(),
    };
    let root = builder.convert(source.root(), 0)?;
    Ok(InputArena::from_parts(root, builder.values))
}

struct ChainInputBuilder<'a> {
    source: &'a ValidatedArena,
    profile: InputProfile,
    values: Arena<InputValue>,
    converted: BTreeMap<ValueId, InputId>,
    active: BTreeSet<ValueId>,
}

impl ChainInputBuilder<'_> {
    fn convert(&mut self, id: ValueId, depth: usize) -> Result<InputId, ValidationError> {
        if depth > 256 {
            return Err(control_error(
                "Typed chain output exceeds the handoff depth limit",
                "bounded typed chain output",
            ));
        }
        if let Some(converted) = self.converted.get(&id) {
            return Ok(*converted);
        }
        if !self.active.insert(id) {
            return Err(control_error(
                "Typed chain output contains a cycle",
                "acyclic typed chain output",
            ));
        }
        let value = self
            .source
            .get(id)
            .ok_or_else(|| control_error("Typed chain output index is invalid", "valid output"))?
            .clone();
        let converted = match value {
            ValidatedValue::Enum(value) => self.convert(value.discriminant, depth + 1)?,
            ValidatedValue::Union(value) => self.convert(value.value(), depth + 1)?,
            ValidatedValue::Nullable(Some(child)) => self.convert(child, depth + 1)?,
            ValidatedValue::Nullable(None) => self.push(InputValue::Null)?,
            value => {
                let input = self.convert_value(value, depth)?;
                self.push(input)?
            }
        };
        self.active.remove(&id);
        self.converted.insert(id, converted);
        Ok(converted)
    }

    fn convert_value(
        &mut self,
        value: ValidatedValue,
        depth: usize,
    ) -> Result<InputValue, ValidationError> {
        let value = match value {
            ValidatedValue::None => InputValue::Null,
            ValidatedValue::Bool(value) => InputValue::Bool(value),
            ValidatedValue::ExactInt(value) | ValidatedValue::FixedInt { value, .. } => {
                InputValue::Integer(value.to_string())
            }
            ValidatedValue::Float(value) => InputValue::Float(value),
            ValidatedValue::Decimal(value) => InputValue::Decimal(value.to_string()),
            ValidatedValue::Fraction(value) => InputValue::Fraction {
                numerator: value.numer().to_string(),
                denominator: value.denom().to_string(),
            },
            ValidatedValue::Complex(value) => InputValue::Complex {
                real: value.re,
                imaginary: value.im,
            },
            ValidatedValue::String(value) => InputValue::String(value),
            ValidatedValue::Bytes(value) => InputValue::Bytes(value),
            ValidatedValue::Date(value) => InputValue::Date(format!(
                "{:04}-{:02}-{:02}",
                value.year, value.month, value.day
            )),
            ValidatedValue::Time(value) => InputValue::Time(time_text(&value)),
            ValidatedValue::DateTime(value) => InputValue::DateTime(format!(
                "{:04}-{:02}-{:02}T{}",
                value.date.year,
                value.date.month,
                value.date.day,
                time_text(&value.time)
            )),
            ValidatedValue::Duration(value) => InputValue::Duration(duration_text(&value)),
            ValidatedValue::Uuid(value) => {
                InputValue::Uuid(uuid::Uuid::from_bytes(value).to_string())
            }
            ValidatedValue::Url(value) => InputValue::Url(value),
            ValidatedValue::MultiHostUrl(value) => InputValue::Url(value),
            ValidatedValue::Pattern(value) => InputValue::Pattern {
                source: value.source().to_owned(),
                flags: value.flags(),
            },
            ValidatedValue::Model(value) => {
                let mut entries = Vec::with_capacity(value.fields().len() + value.extras().len());
                for (name, child) in value.fields() {
                    entries.push(((*name).to_owned(), self.convert(*child, depth + 1)?));
                }
                for (name, child) in value.extras() {
                    entries.push((name.clone(), self.convert(*child, depth + 1)?));
                }
                InputValue::Object {
                    kind: self.object_kind(),
                    entries,
                }
            }
            ValidatedValue::Sequence(items) => self.sequence(items, SequenceKind::List, depth)?,
            ValidatedValue::Tuple(items) => self.sequence(items, SequenceKind::Tuple, depth)?,
            ValidatedValue::Set(items) => self.sequence(items, SequenceKind::Set, depth)?,
            ValidatedValue::FrozenSet(items) => {
                self.sequence(items, SequenceKind::FrozenSet, depth)?
            }
            ValidatedValue::Mapping(entries) => {
                if self.profile == InputProfile::Json {
                    let mut converted = Vec::with_capacity(entries.len());
                    for (key, value) in entries {
                        converted.push((
                            self.json_key_text(key, depth + 1)?,
                            self.convert(value, depth + 1)?,
                        ));
                    }
                    InputValue::Object {
                        kind: ObjectKind::JsonObject,
                        entries: converted,
                    }
                } else {
                    let mut converted = Vec::with_capacity(entries.len());
                    for (key, value) in entries {
                        converted.push((
                            self.convert(key, depth + 1)?,
                            self.convert(value, depth + 1)?,
                        ));
                    }
                    InputValue::Mapping(converted)
                }
            }
            ValidatedValue::Enum(_) | ValidatedValue::Union(_) | ValidatedValue::Nullable(_) => {
                return Err(control_error(
                    "Typed chain wrapper was not reduced",
                    "reduced typed chain value",
                ));
            }
        };
        Ok(value)
    }

    fn sequence(
        &mut self,
        items: Vec<ValueId>,
        native_kind: SequenceKind,
        depth: usize,
    ) -> Result<InputValue, ValidationError> {
        let kind = if self.profile == InputProfile::Json {
            SequenceKind::JsonArray
        } else {
            native_kind
        };
        let mut converted = Vec::with_capacity(items.len());
        for item in items {
            converted.push(self.convert(item, depth + 1)?);
        }
        Ok(InputValue::Sequence {
            kind,
            items: converted,
        })
    }

    fn object_kind(&self) -> ObjectKind {
        if self.profile == InputProfile::Json {
            ObjectKind::JsonObject
        } else {
            ObjectKind::Object
        }
    }

    fn json_key_text(&self, id: ValueId, depth: usize) -> Result<String, ValidationError> {
        if depth > 256 {
            return Err(control_error(
                "Typed chain mapping key exceeds the handoff depth limit",
                "bounded JSON mapping key",
            ));
        }
        let value = self.source.get(id).ok_or_else(|| {
            control_error(
                "Typed chain mapping key index is invalid",
                "valid mapping key",
            )
        })?;
        let text = match value {
            ValidatedValue::Bool(value) => value.to_string(),
            ValidatedValue::ExactInt(value) | ValidatedValue::FixedInt { value, .. } => {
                value.to_string()
            }
            ValidatedValue::Float(value) => value.to_string(),
            ValidatedValue::Decimal(value) => value.to_string(),
            ValidatedValue::Fraction(value) => value.to_string(),
            ValidatedValue::Complex(value) => value.to_string(),
            ValidatedValue::String(value) => value.clone(),
            ValidatedValue::Bytes(value) => String::from_utf8(value.clone()).map_err(|_| {
                control_error(
                    "Typed chain JSON mapping key is not valid UTF-8",
                    "textual JSON mapping key",
                )
            })?,
            ValidatedValue::Date(value) => {
                format!("{:04}-{:02}-{:02}", value.year, value.month, value.day)
            }
            ValidatedValue::Time(value) => time_text(value),
            ValidatedValue::DateTime(value) => format!(
                "{:04}-{:02}-{:02}T{}",
                value.date.year,
                value.date.month,
                value.date.day,
                time_text(&value.time)
            ),
            ValidatedValue::Duration(value) => duration_text(value),
            ValidatedValue::Uuid(value) => uuid::Uuid::from_bytes(*value).to_string(),
            ValidatedValue::Url(value) => value.clone(),
            ValidatedValue::MultiHostUrl(value) => value.clone(),
            ValidatedValue::Pattern(value) => value.source().to_owned(),
            ValidatedValue::Enum(value) => {
                return self.json_key_text(value.discriminant, depth + 1);
            }
            ValidatedValue::Union(value) => {
                return self.json_key_text(value.value(), depth + 1);
            }
            ValidatedValue::Nullable(Some(value)) => {
                return self.json_key_text(*value, depth + 1);
            }
            ValidatedValue::None
            | ValidatedValue::Nullable(None)
            | ValidatedValue::Model(_)
            | ValidatedValue::Sequence(_)
            | ValidatedValue::Tuple(_)
            | ValidatedValue::Mapping(_)
            | ValidatedValue::Set(_)
            | ValidatedValue::FrozenSet(_) => {
                return Err(control_error(
                    "Typed chain output cannot represent this JSON mapping key",
                    "textual JSON mapping key",
                ));
            }
        };
        Ok(text)
    }

    fn push(&mut self, value: InputValue) -> Result<InputId, ValidationError> {
        self.values.push(value).map_err(|_| {
            control_error(
                "Typed chain handoff arena capacity exceeded",
                "bounded typed chain output",
            )
        })
    }
}

fn time_text(value: &super::TimeValue) -> String {
    let fraction = if value.microsecond == 0 {
        String::new()
    } else {
        format!(".{:06}", value.microsecond)
    };
    let offset = value.offset_seconds.map_or_else(String::new, |seconds| {
        let sign = if seconds < 0 { '-' } else { '+' };
        let magnitude = seconds.unsigned_abs();
        format!("{sign}{:02}:{:02}", magnitude / 3600, magnitude % 3600 / 60)
    });
    format!(
        "{:02}:{:02}:{:02}{fraction}{offset}",
        value.hour, value.minute, value.second
    )
}

fn duration_text(value: &super::DurationValue) -> String {
    let sign = if value.positive { "" } else { "-" };
    if value.microseconds == 0 {
        format!("{sign}P{}DT{}S", value.days, value.seconds)
    } else {
        format!(
            "{sign}P{}DT{}.{:06}S",
            value.days, value.seconds, value.microseconds
        )
    }
}

fn control_error(message: &'static str, expected: &'static str) -> ValidationError {
    super::scalars::type_error("schema_invalid", message, expected)
}
