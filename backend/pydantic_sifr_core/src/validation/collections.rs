use std::collections::BTreeSet;

use crate::{
    InputArena, InputId, InputValue, JsonLimits, NativeValue, ObjectKind, SequenceKind,
    build_native_input, parse_json,
};

use super::{
    CollectionConstraints, ErrorDetail, InputProfile, LocationItem, Schema, ValidatedArena,
    ValidatedValue, ValidationError, ValidationOptions, ValidationState, ValueId, validate_at,
    validate_at_depth, validate_options,
};

pub(crate) fn validate_collection(
    state: &mut ValidationState<'_>,
    schema: &Schema,
    input_id: InputId,
    depth: usize,
) -> Result<ValueId, ValidationError> {
    match schema {
        Schema::List { item, constraints } => {
            let children = sequence_input(
                state.input(),
                input_id,
                "list",
                SequenceKind::List,
                state.options(),
            )?;
            validate_length(children.len(), constraints, "list")?;
            let values = validate_items(state, item, &children, depth)?;
            state.push(ValidatedValue::Sequence(values))
        }
        Schema::Tuple(items) => validate_tuple(state, items, input_id, depth),
        Schema::Mapping {
            key,
            value,
            constraints,
        } => validate_mapping(state, key, value, constraints, input_id, depth),
        Schema::Set { item, constraints } => {
            validate_set(state, item, constraints, input_id, depth, false)
        }
        Schema::FrozenSet { item, constraints } => {
            validate_set(state, item, constraints, input_id, depth, true)
        }
        Schema::Generator { .. } => Err(super::scalars::type_error(
            "generator_iterator",
            "Generator schemas require validated_iterator",
            "lazy iterator entry point",
        )),
        Schema::EmbeddedJson(inner) => validate_embedded_json(state, inner, input_id, depth),
        _ => Err(super::scalars::type_error(
            "schema_kind",
            "Validator is not available for this schema kind",
            "implemented schema",
        )),
    }
}

fn sequence_input(
    input: &InputArena,
    input_id: InputId,
    expected: &'static str,
    expected_kind: SequenceKind,
    options: ValidationOptions,
) -> Result<Vec<InputId>, ValidationError> {
    match input.get(input_id) {
        Some(InputValue::Sequence { kind, items })
            if !options.strict
                || match options.profile {
                    InputProfile::Native | InputProfile::Strings => *kind == expected_kind,
                    InputProfile::Json => *kind == SequenceKind::JsonArray,
                } =>
        {
            Ok(items.clone())
        }
        Some(InputValue::Sequence { .. }) => Err(super::scalars::type_error(
            "collection_type",
            "Native collection kind does not match the schema",
            expected,
        )),
        Some(_) => Err(super::scalars::type_error(
            "collection_type",
            "Input must be a sequence",
            expected,
        )),
        None => Err(super::scalars::type_error(
            "internal_input",
            "Input arena index is invalid",
            "valid input arena",
        )),
    }
}

fn validate_items(
    state: &mut ValidationState<'_>,
    schema: &Schema,
    children: &[InputId],
    depth: usize,
) -> Result<Vec<ValueId>, ValidationError> {
    let mut values = Vec::with_capacity(children.len());
    let mut errors = None;
    for (index, child) in children.iter().enumerate() {
        match state.validate_node(schema, *child, depth + 1) {
            Ok(value) => values.push(value),
            Err(error) => {
                collect_error(
                    &mut errors,
                    error.at(LocationItem::Index(index)),
                    state.options().limits.max_errors,
                );
                if errors
                    .as_ref()
                    .is_some_and(|error| error.is_full(state.options().limits.max_errors))
                {
                    if index + 1 < children.len()
                        && let Some(error) = &mut errors
                    {
                        error.mark_truncated();
                    }
                    break;
                }
            }
        }
    }
    match errors {
        Some(error) => Err(error),
        None => Ok(values),
    }
}

fn validate_tuple(
    state: &mut ValidationState<'_>,
    schemas: &[Schema],
    input_id: InputId,
    depth: usize,
) -> Result<ValueId, ValidationError> {
    let children = sequence_input(
        state.input(),
        input_id,
        "tuple",
        SequenceKind::Tuple,
        state.options(),
    )?;
    if children.len() != schemas.len() {
        return Err(ValidationError::one(
            ErrorDetail::new("tuple_length", "Tuple length does not match the schema")
                .context("expected_length", schemas.len().to_string())
                .context("actual_length", children.len().to_string()),
        ));
    }
    let mut values = Vec::with_capacity(children.len());
    let mut errors = None;
    for (index, (schema, child)) in schemas.iter().zip(&children).enumerate() {
        match state.validate_node(schema, *child, depth + 1) {
            Ok(value) => values.push(value),
            Err(error) => collect_error(
                &mut errors,
                error.at(LocationItem::Index(index)),
                state.options().limits.max_errors,
            ),
        }
        if stop_after_error_cap(state, &mut errors, index + 1 < children.len()) {
            break;
        }
    }
    if let Some(error) = errors {
        Err(error)
    } else {
        state.push(ValidatedValue::Tuple(values))
    }
}

fn validate_mapping(
    state: &mut ValidationState<'_>,
    key_schema: &Schema,
    value_schema: &Schema,
    constraints: &CollectionConstraints,
    input_id: InputId,
    depth: usize,
) -> Result<ValueId, ValidationError> {
    let input = state.input().get(input_id).cloned().ok_or_else(|| {
        super::scalars::type_error(
            "internal_input",
            "Input arena index is invalid",
            "valid input arena",
        )
    })?;
    let length = match &input {
        InputValue::Object { kind, entries }
            if !state.options().strict
                || (state.options().profile == InputProfile::Strings
                    && *kind == ObjectKind::Object)
                || (state.options().profile == InputProfile::Json
                    && *kind == ObjectKind::JsonObject) =>
        {
            entries.len()
        }
        InputValue::Mapping(entries)
            if !state.options().strict || state.options().profile != InputProfile::Json =>
        {
            entries.len()
        }
        _ => {
            return Err(super::scalars::type_error(
                "mapping_type",
                "Input must be a mapping",
                "mapping",
            ));
        }
    };
    validate_length(length, constraints, "mapping")?;
    let mut output = Vec::with_capacity(length);
    let mut seen = BTreeSet::new();
    let mut errors = None;
    match input {
        InputValue::Object { kind, entries } => {
            for (index, (field, value_id)) in entries.into_iter().enumerate() {
                let key =
                    validate_object_key(state, key_schema, &field, kind == ObjectKind::JsonObject)
                        .map_err(|error| error.at(LocationItem::MappingKey(index)));
                let value = state
                    .validate_node(value_schema, value_id, depth + 1)
                    .map_err(|error| error.at(LocationItem::Field(field)));
                collect_mapping_entry(
                    state,
                    index,
                    key,
                    value,
                    &mut output,
                    &mut seen,
                    &mut errors,
                );
                if stop_after_error_cap(state, &mut errors, index + 1 < length) {
                    break;
                }
            }
        }
        InputValue::Mapping(entries) => {
            for (index, (key_id, value_id)) in entries.into_iter().enumerate() {
                let key = state
                    .validate_node(key_schema, key_id, depth + 1)
                    .map_err(|error| error.at(LocationItem::MappingKey(index)));
                let value = state
                    .validate_node(value_schema, value_id, depth + 1)
                    .map_err(|error| error.at(LocationItem::Index(index)));
                collect_mapping_entry(
                    state,
                    index,
                    key,
                    value,
                    &mut output,
                    &mut seen,
                    &mut errors,
                );
                if stop_after_error_cap(state, &mut errors, index + 1 < length) {
                    break;
                }
            }
        }
        _ => {}
    }
    if let Some(error) = errors {
        Err(error)
    } else {
        state.push(ValidatedValue::Mapping(output))
    }
}

pub(crate) fn stop_after_error_cap(
    state: &ValidationState<'_>,
    errors: &mut Option<ValidationError>,
    has_more: bool,
) -> bool {
    let full = errors
        .as_ref()
        .is_some_and(|error| error.is_full(state.options().limits.max_errors));
    if full
        && has_more
        && let Some(error) = errors
    {
        error.mark_truncated();
    }
    full
}

fn collect_mapping_entry(
    state: &ValidationState<'_>,
    index: usize,
    key: Result<ValueId, ValidationError>,
    value: Result<ValueId, ValidationError>,
    output: &mut Vec<(ValueId, ValueId)>,
    seen: &mut BTreeSet<Vec<u8>>,
    errors: &mut Option<ValidationError>,
) {
    let limit = state.options().limits.max_errors;
    match (key, value) {
        (Ok(key), Ok(value)) => match canonical_key(state, key, KeyUse::Mapping) {
            Ok(identity) => {
                if seen.insert(identity) {
                    output.push((key, value));
                } else {
                    collect_error(
                        errors,
                        ValidationError::one(ErrorDetail::new(
                            "mapping_key_duplicate",
                            "Validated mapping key is duplicated",
                        ))
                        .at(LocationItem::MappingKey(index)),
                        limit,
                    );
                }
            }
            Err(error) => collect_error(errors, error.at(LocationItem::MappingKey(index)), limit),
        },
        (Err(key_error), Ok(_)) => collect_error(errors, key_error, limit),
        (Ok(_), Err(value_error)) => collect_error(errors, value_error, limit),
        (Err(key_error), Err(value_error)) => {
            collect_error(errors, key_error, limit);
            collect_error(errors, value_error, limit);
        }
    }
}

fn validate_object_key(
    state: &mut ValidationState<'_>,
    schema: &Schema,
    field: &str,
    json_key: bool,
) -> Result<ValueId, ValidationError> {
    let input = build_native_input(
        &NativeValue::String(field.to_owned()),
        json_limits(state.options()),
    )
    .map_err(|_| {
        super::scalars::type_error(
            "resource_limit",
            "Mapping key exceeds input limits",
            "bounded mapping key",
        )
    })?;
    let mut options = state.options();
    if json_key && options.profile == InputProfile::Json {
        options.strict = false;
    }
    let arena = validate_at(schema, &input, input.root(), options)?;
    state.import(arena)
}

fn validate_set(
    state: &mut ValidationState<'_>,
    item: &Schema,
    constraints: &CollectionConstraints,
    input_id: InputId,
    depth: usize,
    frozen: bool,
) -> Result<ValueId, ValidationError> {
    let expected_kind = if frozen {
        SequenceKind::FrozenSet
    } else {
        SequenceKind::Set
    };
    let children = sequence_input(
        state.input(),
        input_id,
        "set",
        expected_kind,
        state.options(),
    )?;
    let values = validate_items(state, item, &children, depth)?;
    let mut seen = BTreeSet::new();
    let mut unique = Vec::with_capacity(values.len());
    let mut errors = None;
    for (index, value) in values.into_iter().enumerate() {
        match canonical_key(state, value, KeyUse::Set) {
            Ok(identity) => {
                if seen.insert(identity) {
                    unique.push(value);
                }
            }
            Err(error) => collect_error(
                &mut errors,
                error.at(LocationItem::Index(index)),
                state.options().limits.max_errors,
            ),
        }
        if stop_after_error_cap(state, &mut errors, index + 1 < children.len()) {
            break;
        }
    }
    if let Some(error) = errors {
        return Err(error);
    }
    validate_length(
        unique.len(),
        constraints,
        if frozen { "frozenset" } else { "set" },
    )?;
    if frozen {
        state.push(ValidatedValue::FrozenSet(unique))
    } else {
        state.push(ValidatedValue::Set(unique))
    }
}

fn validate_embedded_json(
    state: &mut ValidationState<'_>,
    inner: &Schema,
    input_id: InputId,
    depth: usize,
) -> Result<ValueId, ValidationError> {
    let bytes = match state.input().get(input_id) {
        Some(InputValue::String(value)) => value.as_bytes(),
        Some(InputValue::Bytes(value)) => value.as_slice(),
        Some(_) => {
            return Err(super::scalars::type_error(
                "json_type",
                "Embedded JSON input must be text or bytes",
                "JSON text",
            ));
        }
        None => {
            return Err(super::scalars::type_error(
                "internal_input",
                "Input arena index is invalid",
                "valid input arena",
            ));
        }
    };
    let input = parse_json(bytes, json_limits(state.options())).map_err(|error| {
        ValidationError::one(
            ErrorDetail::new(error.code, error.message)
                .context("line", error.line.to_string())
                .context("column", error.column.to_string()),
        )
    })?;
    let mut options = state.options();
    options.profile = InputProfile::Json;
    let arena = validate_at_depth(inner, &input, input.root(), options, depth + 1)?;
    state.import(arena)
}

fn validate_length(
    length: usize,
    constraints: &CollectionConstraints,
    kind: &str,
) -> Result<(), ValidationError> {
    super::scalars::validate_length(length, constraints.min_length, constraints.max_length, kind)
}

pub(crate) fn collect_error(
    target: &mut Option<ValidationError>,
    error: ValidationError,
    limit: usize,
) {
    match target {
        Some(target) => target.append(error, limit),
        None => *target = Some(error),
    }
}

fn json_limits(options: ValidationOptions) -> JsonLimits {
    JsonLimits {
        max_input_bytes: options.limits.max_string_bytes,
        max_depth: options.limits.max_depth,
        max_nodes: options.limits.max_collection_items,
        max_string_bytes: options.limits.max_string_bytes,
        max_integer_digits: options.limits.max_numeric_digits,
        max_collection_items: options.limits.max_collection_items,
    }
}

fn canonical_key(
    state: &ValidationState<'_>,
    id: ValueId,
    usage: KeyUse,
) -> Result<Vec<u8>, ValidationError> {
    let value = state.value(id).ok_or_else(|| {
        super::scalars::type_error(
            "internal_output",
            "Validated arena index is invalid",
            "valid output arena",
        )
    })?;
    let mut key = Vec::new();
    match value {
        ValidatedValue::None => key.push(0),
        ValidatedValue::Bool(value) => key.extend([1, u8::from(*value)]),
        ValidatedValue::ExactInt(value) | ValidatedValue::FixedInt { value, .. } => {
            key.push(2);
            append_bytes(&mut key, &value.to_signed_bytes_be());
        }
        ValidatedValue::Float(value) => {
            key.push(3);
            let bits = if *value == 0.0 { 0 } else { value.to_bits() };
            key.extend(bits.to_be_bytes());
        }
        ValidatedValue::Decimal(value) => {
            key.push(4);
            append_bytes(&mut key, value.normalized().to_string().as_bytes());
        }
        ValidatedValue::Fraction(value) => {
            key.push(5);
            append_bytes(&mut key, &value.numer().to_signed_bytes_be());
            append_bytes(&mut key, &value.denom().to_signed_bytes_be());
        }
        ValidatedValue::Complex(value) => {
            key.push(6);
            let real = if value.re == 0.0 {
                0
            } else {
                value.re.to_bits()
            };
            let imaginary = if value.im == 0.0 {
                0
            } else {
                value.im.to_bits()
            };
            key.extend(real.to_be_bytes());
            key.extend(imaginary.to_be_bytes());
        }
        ValidatedValue::String(value) => {
            key.push(7);
            append_bytes(&mut key, value.as_bytes());
        }
        ValidatedValue::Bytes(value) => {
            key.push(8);
            append_bytes(&mut key, value);
        }
        ValidatedValue::Date(value) => {
            key.push(9);
            key.extend(value.year.to_be_bytes());
            key.extend([value.month, value.day]);
        }
        ValidatedValue::Time(value) => {
            key.push(10);
            key.extend([value.hour, value.minute, value.second]);
            key.extend(value.microsecond.to_be_bytes());
            key.push(u8::from(value.offset_seconds.is_some()));
            key.extend(value.offset_seconds.unwrap_or_default().to_be_bytes());
        }
        ValidatedValue::DateTime(value) => {
            key.push(11);
            key.extend(value.date.year.to_be_bytes());
            key.extend([value.date.month, value.date.day]);
            key.extend([value.time.hour, value.time.minute, value.time.second]);
            key.extend(value.time.microsecond.to_be_bytes());
            key.push(u8::from(value.time.offset_seconds.is_some()));
            key.extend(value.time.offset_seconds.unwrap_or_default().to_be_bytes());
        }
        ValidatedValue::Duration(value) => {
            key.push(12);
            key.push(u8::from(value.positive));
            key.extend(value.days.to_be_bytes());
            key.extend(value.seconds.to_be_bytes());
            key.extend(value.microseconds.to_be_bytes());
        }
        ValidatedValue::Uuid(value) => {
            key.push(13);
            key.extend(value);
        }
        ValidatedValue::Url(value) => {
            key.push(14);
            append_bytes(&mut key, value.as_bytes());
        }
        ValidatedValue::Pattern(value) => {
            key.push(15);
            key.push(value.flags());
            append_bytes(&mut key, value.source().as_bytes());
        }
        ValidatedValue::Nullable(None) => key.push(16),
        ValidatedValue::Nullable(Some(child)) => {
            key.push(17);
            key.extend(canonical_key(state, *child, usage)?);
        }
        ValidatedValue::Sequence(_)
        | ValidatedValue::Tuple(_)
        | ValidatedValue::Mapping(_)
        | ValidatedValue::Set(_)
        | ValidatedValue::FrozenSet(_)
        | ValidatedValue::Model(_) => {
            let (code, message) = match usage {
                KeyUse::Set => ("set_item_unhashable", "Set items must be scalar values"),
                KeyUse::Mapping => (
                    "mapping_key_unhashable",
                    "Mapping keys must be scalar values",
                ),
            };
            return Err(super::scalars::type_error(code, message, "hashable scalar"));
        }
    }
    Ok(key)
}

#[derive(Clone, Copy)]
enum KeyUse {
    Set,
    Mapping,
}

fn append_bytes(target: &mut Vec<u8>, bytes: &[u8]) {
    target.extend(bytes.len().to_be_bytes());
    target.extend(bytes);
}

#[derive(Debug)]
pub struct ValidatedIterator<'a> {
    input: &'a InputArena,
    children: Vec<InputId>,
    item: Schema,
    constraints: CollectionConstraints,
    options: ValidationOptions,
    index: usize,
    finished: bool,
}

pub fn validated_iterator<'a>(
    schema: &Schema,
    input: &'a InputArena,
    options: ValidationOptions,
) -> Result<ValidatedIterator<'a>, ValidationError> {
    validate_options(options)?;
    let Schema::Generator { item, constraints } = schema else {
        return Err(super::scalars::type_error(
            "generator_schema",
            "validated_iterator requires a generator schema",
            "generator schema",
        ));
    };
    let children = sequence_input(
        input,
        input.root(),
        "generator",
        SequenceKind::List,
        ValidationOptions {
            strict: false,
            ..options
        },
    )?;
    if children.len() > options.limits.max_collection_items {
        return Err(super::scalars::type_error(
            "resource_limit",
            "Validation collection item limit exceeded",
            "bounded generator",
        ));
    }
    Ok(ValidatedIterator {
        input,
        children,
        item: item.as_ref().clone(),
        constraints: constraints.clone(),
        options,
        index: 0,
        finished: false,
    })
}

impl Iterator for ValidatedIterator<'_> {
    type Item = Result<ValidatedArena, ValidationError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        if self
            .constraints
            .max_length
            .is_some_and(|maximum| self.index >= maximum && self.index < self.children.len())
        {
            self.finished = true;
            return Some(Err(ValidationError::one(
                ErrorDetail::new("too_long", "generator is too long").context(
                    "maximum",
                    self.constraints.max_length.unwrap_or_default().to_string(),
                ),
            )));
        }
        let Some(child) = self.children.get(self.index).copied() else {
            self.finished = true;
            if self
                .constraints
                .min_length
                .is_some_and(|minimum| self.index < minimum)
            {
                return Some(Err(ValidationError::one(
                    ErrorDetail::new("too_short", "generator is too short").context(
                        "minimum",
                        self.constraints.min_length.unwrap_or_default().to_string(),
                    ),
                )));
            }
            return None;
        };
        let index = self.index;
        self.index += 1;
        Some(
            validate_at(&self.item, self.input, child, self.options)
                .map_err(|error| error.at(LocationItem::Index(index))),
        )
    }
}
