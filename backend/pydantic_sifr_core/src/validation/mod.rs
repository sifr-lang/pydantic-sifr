mod collections;
mod construction;
mod definitions;
mod error;
mod models;
mod scalars;
mod schema;
mod schema_view;
mod special;
mod static_sums;
mod structural;
mod sum_schema;
mod sums;
mod textual;
mod value;

pub use collections::{ValidatedIterator, validated_iterator};
pub use construction::{
    validate_and_construct, validate_json_and_construct, validate_native_and_construct,
    validate_strings_and_construct, validate_structural_and_construct,
};
pub use definitions::{DefinitionSchema, DefinitionsSchema};
pub use error::{ErrorDetail, LocationItem, ValidationError, ValidationLimits};
pub use schema::{
    AliasPath, AliasSegment, BytesConstraints, BytesJsonMode, CollectionConstraints,
    ComplexConstraints, DecimalConstraints, ExtraPolicy, FieldDefault, FloatConstraints,
    FractionConstraints, InputProfile, IntegerConstraints, IntegerTarget, ModelField, ModelSchema,
    PatternCompileError, PatternSchema, PreparedSchema, RelativeTimeConstraint, Schema,
    StringConstraints, StringPattern, TemporalKind, TemporalSchema, UrlConstraints,
};
pub use schema_view::SchemaRef;
pub use sum_schema::{
    DiscriminatorPath, EnumSchema, EnumVariant, LiteralSchema, LiteralValue, SchemaErrorOverride,
    TaggedUnionChoice, TaggedUnionSchema, UnionChoice, UnionMode, UnionSchema,
};
pub use value::{
    DateTimeValue, DateValue, DurationValue, EnumValue, ModelValue, PatternValue, TimeValue,
    UnionValue, ValidatedArena, ValidatedValue, ValueId,
};

use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{Arena, InputArena, InputId, InputValue};

const HARD_MAX_DEPTH: usize = 256;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ClockSnapshot {
    pub unix_seconds: i64,
    pub microsecond: u32,
}

impl ClockSnapshot {
    #[must_use]
    pub fn system_utc() -> Self {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => Self {
                unix_seconds: i64::try_from(duration.as_secs()).unwrap_or(i64::MAX),
                microsecond: duration.subsec_micros(),
            },
            Err(error) => {
                let duration = error.duration();
                let seconds = i64::try_from(duration.as_secs()).unwrap_or(i64::MAX);
                let microsecond = duration.subsec_micros();
                if microsecond == 0 {
                    Self {
                        unix_seconds: -seconds,
                        microsecond: 0,
                    }
                } else {
                    Self {
                        unix_seconds: seconds.saturating_neg().saturating_sub(1),
                        microsecond: 1_000_000 - microsecond,
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidationOptions {
    pub strict: bool,
    pub profile: InputProfile,
    pub limits: ValidationLimits,
    pub clock: ClockSnapshot,
}

impl Default for ValidationOptions {
    fn default() -> Self {
        Self {
            strict: false,
            profile: InputProfile::Native,
            limits: ValidationLimits::default(),
            clock: ClockSnapshot::default(),
        }
    }
}

pub fn validate(
    schema: &Schema,
    input: &InputArena,
    options: ValidationOptions,
) -> Result<ValidatedArena, ValidationError> {
    validate_ref(SchemaRef::owned(schema), input, options)
}

pub(crate) fn validate_ref(
    schema: SchemaRef<'_>,
    input: &InputArena,
    options: ValidationOptions,
) -> Result<ValidatedArena, ValidationError> {
    validate_at(schema, input, input.root(), options)
}

fn validate_at(
    schema: SchemaRef<'_>,
    input: &InputArena,
    root: InputId,
    options: ValidationOptions,
) -> Result<ValidatedArena, ValidationError> {
    validate_at_depth(schema, input, root, options, 0)
}

pub(crate) fn validate_at_depth(
    schema: SchemaRef<'_>,
    input: &InputArena,
    root: InputId,
    options: ValidationOptions,
    start_depth: usize,
) -> Result<ValidatedArena, ValidationError> {
    validate_at_depth_with_context(
        schema,
        input,
        root,
        options,
        start_depth,
        Vec::new(),
        Vec::new(),
    )
}

pub(crate) fn validate_at_depth_with_context(
    schema: SchemaRef<'_>,
    input: &InputArena,
    root: InputId,
    options: ValidationOptions,
    start_depth: usize,
    definition_scopes: Vec<DefinitionScope>,
    active_references: Vec<(InputId, &'static str)>,
) -> Result<ValidatedArena, ValidationError> {
    validate_options(options)?;
    if options.profile == InputProfile::Strings {
        check_strings_profile(input, root)?;
    }
    check_input_limits(input, root, start_depth, options.limits)?;
    let mut state = ValidationState {
        input,
        values: Arena::new(),
        options,
        definition_scopes,
        active_references,
    };
    let root = state.validate_node(schema, root, start_depth)?;
    Ok(ValidatedArena::new(root, state.values))
}

pub(crate) type DefinitionScope = BTreeMap<&'static str, Arc<Schema>>;

pub(crate) fn validate_options(options: ValidationOptions) -> Result<(), ValidationError> {
    if options.limits.max_depth == 0
        || options.limits.max_collection_items == 0
        || options.limits.max_string_bytes == 0
        || options.limits.max_numeric_digits == 0
        || options.limits.max_decimal_exponent == 0
        || options.limits.max_errors == 0
        || options.limits.max_depth > HARD_MAX_DEPTH
    {
        Err(scalars::type_error(
            "resource_limit",
            "Validation limits must be greater than zero",
            "positive limits",
        ))
    } else {
        Ok(())
    }
}

pub(crate) struct ValidationState<'a> {
    input: &'a InputArena,
    values: Arena<ValidatedValue>,
    options: ValidationOptions,
    definition_scopes: Vec<DefinitionScope>,
    active_references: Vec<(InputId, &'static str)>,
}

impl ValidationState<'_> {
    pub(crate) fn validate_node(
        &mut self,
        schema: SchemaRef<'_>,
        input_id: InputId,
        depth: usize,
    ) -> Result<ValueId, ValidationError> {
        if depth > self.options.limits.max_depth {
            return Err(scalars::type_error(
                "recursion_limit",
                "Validation recursion limit exceeded",
                "bounded input",
            ));
        }
        let input = self.input.get(input_id).ok_or_else(|| {
            scalars::type_error(
                "internal_input",
                "Input arena index is invalid",
                "valid input arena",
            )
        })?;
        let tag = schema.tag()?;
        if matches!(
            tag,
            schema_view::SchemaTag::Definitions | schema_view::SchemaTag::DefinitionRef
        ) {
            return definitions::validate_definitions(self, schema, input_id, depth);
        }
        if tag == schema_view::SchemaTag::Model {
            return models::validate_model(self, schema.model()?, input_id, depth);
        }
        if matches!(
            tag,
            schema_view::SchemaTag::Literal
                | schema_view::SchemaTag::Enum
                | schema_view::SchemaTag::Nullable
                | schema_view::SchemaTag::Union
                | schema_view::SchemaTag::TaggedUnion
        ) {
            return sums::validate_sum(self, schema, input_id, depth);
        }
        let value = if let Some(result) = scalars::validate_scalar(schema, input, self.options) {
            result?
        } else if let SchemaRef::Owned(owned) = schema {
            if let Some(result) = special::validate_special(
                owned,
                input,
                self.options.strict,
                self.options.profile,
                self.options.clock,
            ) {
                result?
            } else {
                return collections::validate_collection(self, schema, input_id, depth);
            }
        } else {
            return collections::validate_collection(self, schema, input_id, depth);
        };
        value::push_value(&mut self.values, value).map_err(|_| {
            scalars::type_error(
                "resource_limit",
                "Validated arena capacity exceeded",
                "bounded output",
            )
        })
    }

    pub(crate) const fn input(&self) -> &InputArena {
        self.input
    }

    pub(crate) const fn options(&self) -> ValidationOptions {
        self.options
    }

    pub(crate) fn value(&self, id: ValueId) -> Option<&ValidatedValue> {
        self.values.get(id)
    }

    pub(crate) fn push(&mut self, value: ValidatedValue) -> Result<ValueId, ValidationError> {
        value::push_value(&mut self.values, value).map_err(arena_validation_error)
    }

    pub(crate) fn import(&mut self, arena: ValidatedArena) -> Result<ValueId, ValidationError> {
        let root = arena.root();
        self.import_at(arena, root)
    }

    pub(crate) fn import_at(
        &mut self,
        arena: ValidatedArena,
        selected: ValueId,
    ) -> Result<ValueId, ValidationError> {
        let offset = self.values.len();
        let (_, values) = arena.into_parts();
        for mut value in values {
            value.remap_ids(offset).map_err(arena_validation_error)?;
            self.push(value)?;
        }
        let root = usize::try_from(selected.raw())
            .ok()
            .and_then(|raw| raw.checked_add(offset))
            .ok_or_else(arena_capacity_error)?;
        crate::ArenaId::from_usize(root).map_err(arena_validation_error)
    }

    pub(crate) fn validate_branch(
        &self,
        schema: SchemaRef<'_>,
        root: InputId,
        start_depth: usize,
    ) -> Result<ValidatedArena, ValidationError> {
        let mut state = ValidationState {
            input: self.input,
            values: Arena::new(),
            options: self.options,
            definition_scopes: self.definition_scopes.clone(),
            active_references: self.active_references.clone(),
        };
        let root = state.validate_node(schema, root, start_depth)?;
        Ok(ValidatedArena::new(root, state.values))
    }

    pub(crate) fn validate_input(
        &self,
        schema: SchemaRef<'_>,
        input: &InputArena,
        root: InputId,
        options: ValidationOptions,
        start_depth: usize,
    ) -> Result<ValidatedArena, ValidationError> {
        validate_at_depth_with_context(
            schema,
            input,
            root,
            options,
            start_depth,
            self.definition_scopes.clone(),
            Vec::new(),
        )
    }

    pub(crate) fn push_definition_scope(&mut self, scope: DefinitionScope) {
        self.definition_scopes.push(scope);
    }

    pub(crate) fn pop_definition_scope(&mut self) {
        self.definition_scopes.pop();
    }

    pub(crate) fn definition(&self, name: &'static str) -> Option<Arc<Schema>> {
        self.definition_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    pub(crate) fn enter_reference(&mut self, input: InputId, name: &'static str) -> bool {
        if self.active_references.contains(&(input, name)) {
            return false;
        }
        self.active_references.push((input, name));
        true
    }

    pub(crate) fn leave_reference(&mut self, input: InputId, name: &'static str) {
        if let Some(index) = self
            .active_references
            .iter()
            .rposition(|entry| *entry == (input, name))
        {
            self.active_references.remove(index);
        }
    }
}

fn arena_capacity_error() -> ValidationError {
    scalars::type_error(
        "resource_limit",
        "Validated arena capacity exceeded",
        "bounded output",
    )
}

fn arena_validation_error(_error: crate::ArenaError) -> ValidationError {
    arena_capacity_error()
}

fn check_input_limits(
    input: &InputArena,
    root: InputId,
    start_depth: usize,
    limits: ValidationLimits,
) -> Result<(), ValidationError> {
    let mut pending = vec![(root, start_depth)];
    let mut string_bytes = 0_usize;
    while let Some((id, depth)) = pending.pop() {
        if depth > limits.max_depth {
            return Err(scalars::type_error(
                "recursion_limit",
                "Validation recursion limit exceeded",
                "bounded input",
            ));
        }
        let value = input.get(id).ok_or_else(|| {
            scalars::type_error(
                "internal_input",
                "Input arena index is invalid",
                "valid input arena",
            )
        })?;
        let byte_count = match value {
            InputValue::Integer(value) | InputValue::Decimal(value) | InputValue::String(value) => {
                value.len()
            }
            InputValue::Date(value)
            | InputValue::Time(value)
            | InputValue::DateTime(value)
            | InputValue::Duration(value)
            | InputValue::Uuid(value)
            | InputValue::Url(value) => value.len(),
            InputValue::Pattern { source, .. } => source.len(),
            InputValue::Bytes(value) => value.len(),
            InputValue::Fraction {
                numerator,
                denominator,
            } => numerator.len().saturating_add(denominator.len()),
            InputValue::Sequence { items, .. } => {
                check_collection_limit(items.len(), limits)?;
                pending.extend(items.iter().map(|id| (*id, depth + 1)));
                0
            }
            InputValue::Object { entries, .. } => {
                check_collection_limit(entries.len(), limits)?;
                for (key, id) in entries {
                    string_bytes = string_bytes.saturating_add(key.len());
                    pending.push((*id, depth + 1));
                }
                0
            }
            InputValue::Mapping(entries) => {
                check_collection_limit(entries.len(), limits)?;
                for (key, value) in entries {
                    pending.push((*key, depth + 1));
                    pending.push((*value, depth + 1));
                }
                0
            }
            InputValue::Null
            | InputValue::Bool(_)
            | InputValue::Float(_)
            | InputValue::Complex { .. } => 0,
        };
        string_bytes = string_bytes.saturating_add(byte_count);
        if string_bytes > limits.max_string_bytes {
            return Err(scalars::type_error(
                "resource_limit",
                "Validation string byte limit exceeded",
                "bounded input",
            ));
        }
    }
    Ok(())
}

fn check_collection_limit(length: usize, limits: ValidationLimits) -> Result<(), ValidationError> {
    if length > limits.max_collection_items {
        Err(scalars::type_error(
            "resource_limit",
            "Validation collection item limit exceeded",
            "bounded input",
        ))
    } else {
        Ok(())
    }
}

fn check_strings_profile(input: &InputArena, root: InputId) -> Result<(), ValidationError> {
    let mut pending = vec![root];
    while let Some(id) = pending.pop() {
        match input.get(id) {
            Some(InputValue::String(_)) => {}
            Some(InputValue::Sequence { items, .. }) => pending.extend(items),
            Some(InputValue::Object { entries, .. }) => {
                pending.extend(entries.iter().map(|(_, value)| value));
            }
            Some(InputValue::Mapping(entries)) => {
                for (key, value) in entries {
                    pending.push(*key);
                    pending.push(*value);
                }
            }
            Some(_) => {
                return Err(scalars::type_error(
                    "strings_type",
                    "Strings profile requires string scalar leaves",
                    "structural strings",
                ));
            }
            None => {
                return Err(scalars::type_error(
                    "internal_input",
                    "Input arena index is invalid",
                    "valid input arena",
                ));
            }
        }
    }
    Ok(())
}
