mod collections;
mod construction;
mod control;
mod definitions;
mod error;
mod function_validators;
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
    validate_and_construct, validate_and_construct_with_callbacks, validate_json_and_construct,
    validate_json_and_construct_with_callbacks, validate_json_strings_and_construct,
    validate_json_strings_and_construct_with_callbacks, validate_native_and_construct,
    validate_strings_and_construct, validate_structural_and_construct,
    validate_structural_and_construct_with_callbacks,
};
pub use definitions::{DefinitionSchema, DefinitionsSchema};
pub use error::{ErrorDetail, LocationItem, ValidationError, ValidationLimits};
pub use function_validators::{ValidationCallbacks, validator_callback_error};
pub use schema::{
    AliasPath, AliasSegment, BytesConstraints, BytesJsonMode, ChainSchema, CollectionConstraints,
    ComplexConstraints, DecimalConstraints, ExtraPolicy, FieldDefault, FloatConstraints,
    FractionConstraints, InputProfile, IntegerConstraints, IntegerTarget, JsonOrStructuralSchema,
    LaxOrStrictSchema, ModelField, ModelSchema, PatternCompileError, PatternSchema, PreparedSchema,
    RelativeTimeConstraint, Schema, StringConstraints, StringPattern, TemporalKind, TemporalSchema,
    UrlConstraints,
};
pub(crate) use schema_view::{ExtraRef, FieldRef, ModelRef, StaticSerializer, static_serializers};
pub use schema_view::{SchemaRef, SchemaTag};
pub(crate) use static_sums::declared_values as static_declared_values;
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
    pub strict_override: Option<bool>,
    pub profile: InputProfile,
    pub limits: ValidationLimits,
    pub clock: ClockSnapshot,
}

impl Default for ValidationOptions {
    fn default() -> Self {
        Self {
            strict: false,
            strict_override: None,
            profile: InputProfile::Native,
            limits: ValidationLimits::default(),
            clock: ClockSnapshot::default(),
        }
    }
}

impl ValidationOptions {
    #[must_use]
    pub const fn effective_strict(self) -> bool {
        match self.strict_override {
            Some(strict) => strict,
            None => self.strict,
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

pub(crate) fn validate_ref_with_callbacks<'input>(
    schema: SchemaRef<'_>,
    input: &'input InputArena,
    options: ValidationOptions,
    callbacks: &'input dyn ValidationCallbacks,
) -> Result<ValidatedArena, ValidationError> {
    validate_at_depth_with_context_mode(
        schema,
        input,
        input.root(),
        options,
        0,
        ValidationContext {
            definition_scopes: Arc::new(Vec::new()),
            active_references: Vec::new(),
            enforce_strings_input: true,
            skip_callbacks: false,
        },
        Some(callbacks),
    )
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
        Arc::new(Vec::new()),
        Vec::new(),
    )
}

pub(crate) fn validate_ref_for_serialization(
    schema: SchemaRef<'_>,
    input: &InputArena,
    options: ValidationOptions,
) -> Result<ValidatedArena, ValidationError> {
    validate_at_depth_with_context_mode(
        schema,
        input,
        input.root(),
        options,
        0,
        ValidationContext {
            definition_scopes: Arc::new(Vec::new()),
            active_references: Vec::new(),
            enforce_strings_input: true,
            skip_callbacks: true,
        },
        None,
    )
}

pub(crate) fn validate_at_depth_with_context(
    schema: SchemaRef<'_>,
    input: &InputArena,
    root: InputId,
    options: ValidationOptions,
    start_depth: usize,
    definition_scopes: DefinitionScopes,
    active_references: Vec<(InputId, &'static str)>,
) -> Result<ValidatedArena, ValidationError> {
    validate_at_depth_with_context_mode(
        schema,
        input,
        root,
        options,
        start_depth,
        ValidationContext {
            definition_scopes,
            active_references,
            enforce_strings_input: true,
            skip_callbacks: false,
        },
        None,
    )
}

struct ValidationContext {
    definition_scopes: DefinitionScopes,
    active_references: Vec<(InputId, &'static str)>,
    enforce_strings_input: bool,
    skip_callbacks: bool,
}

fn validate_at_depth_with_context_mode<'input>(
    schema: SchemaRef<'_>,
    input: &'input InputArena,
    root: InputId,
    options: ValidationOptions,
    start_depth: usize,
    context: ValidationContext,
    callbacks: Option<&'input dyn ValidationCallbacks>,
) -> Result<ValidatedArena, ValidationError> {
    validate_options(options)?;
    if context.enforce_strings_input && options.profile == InputProfile::Strings {
        check_strings_profile(input, root)?;
    }
    check_input_limits(input, root, start_depth, options.limits)?;
    let mut state = ValidationState {
        input,
        values: Arena::new(),
        options,
        definition_scopes: context.definition_scopes,
        active_references: context.active_references,
        callbacks,
        skip_callbacks: context.skip_callbacks,
    };
    let root = state.validate_node(schema, root, start_depth)?;
    Ok(ValidatedArena::new(root, state.values))
}

pub(crate) type DefinitionScope = BTreeMap<&'static str, Arc<Schema>>;
pub(crate) type DefinitionScopes = Arc<Vec<DefinitionScope>>;

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
    definition_scopes: DefinitionScopes,
    active_references: Vec<(InputId, &'static str)>,
    callbacks: Option<&'a dyn ValidationCallbacks>,
    skip_callbacks: bool,
}

impl ValidationState<'_> {
    pub(crate) fn validate_node(
        &mut self,
        schema: SchemaRef<'_>,
        input_id: InputId,
        depth: usize,
    ) -> Result<ValueId, ValidationError> {
        let result = self.validate_node_without_override(schema, input_id, depth);
        match (schema, result) {
            (SchemaRef::Static(_), Err(error)) => {
                Err(sums::apply_override(error, schema.static_error()?))
            }
            (_, result) => result,
        }
    }

    fn validate_node_without_override(
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
            schema_view::SchemaTag::LaxOrStrict
                | schema_view::SchemaTag::JsonOrStructural
                | schema_view::SchemaTag::Chain
        ) {
            return control::validate_control(self, schema, input_id, depth);
        }
        if matches!(
            tag,
            schema_view::SchemaTag::FunctionBefore
                | schema_view::SchemaTag::FunctionAfter
                | schema_view::SchemaTag::FunctionPlain
        ) {
            if self.skip_callbacks {
                let child = match tag {
                    schema_view::SchemaTag::FunctionAfter => schema.child(0)?,
                    schema_view::SchemaTag::FunctionBefore
                    | schema_view::SchemaTag::FunctionPlain => schema.child(1)?,
                    _ => schema,
                };
                return self.validate_node(child, input_id, depth + 1);
            }
            return function_validators::validate_function(self, schema, input_id, depth);
        }
        if matches!(
            tag,
            schema_view::SchemaTag::Definitions | schema_view::SchemaTag::DefinitionRef
        ) {
            return definitions::validate_definitions(self, schema, input_id, depth);
        }
        if tag == schema_view::SchemaTag::Model {
            let model = schema.model()?;
            if self.options.strict_override.is_none() && !self.options.strict && model.strict()? {
                let mut options = self.options;
                options.strict = true;
                let branch = self.validate_branch_with_options(schema, input_id, depth, options)?;
                return self.import(branch);
            }
            return models::validate_model(self, model, input_id, depth);
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
                self.options.effective_strict(),
                self.options.profile,
                self.options.clock,
            ) {
                result?
            } else {
                return collections::validate_collection(self, schema, input_id, depth);
            }
        } else {
            if let Some(result) = special::validate_static_special(
                schema,
                input,
                self.options.effective_strict(),
                self.options.profile,
            ) {
                result?
            } else {
                return collections::validate_collection(self, schema, input_id, depth);
            }
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
        &mut self,
        schema: SchemaRef<'_>,
        root: InputId,
        start_depth: usize,
    ) -> Result<ValidatedArena, ValidationError> {
        self.validate_branch_with_options(schema, root, start_depth, self.options)
    }

    pub(crate) fn validate_branch_with_options(
        &mut self,
        schema: SchemaRef<'_>,
        root: InputId,
        start_depth: usize,
        options: ValidationOptions,
    ) -> Result<ValidatedArena, ValidationError> {
        let mut state = ValidationState {
            input: self.input,
            values: Arena::new(),
            options,
            definition_scopes: Arc::clone(&self.definition_scopes),
            active_references: self.active_references.clone(),
            callbacks: self.callbacks,
            skip_callbacks: self.skip_callbacks,
        };
        let root = state.validate_node(schema, root, start_depth)?;
        Ok(ValidatedArena::new(root, state.values))
    }

    pub(crate) fn validate_input(
        &mut self,
        schema: SchemaRef<'_>,
        input: &InputArena,
        root: InputId,
        options: ValidationOptions,
        start_depth: usize,
    ) -> Result<ValidatedArena, ValidationError> {
        validate_at_depth_with_context_mode(
            schema,
            input,
            root,
            options,
            start_depth,
            ValidationContext {
                definition_scopes: Arc::clone(&self.definition_scopes),
                active_references: Vec::new(),
                enforce_strings_input: true,
                skip_callbacks: self.skip_callbacks,
            },
            self.callbacks,
        )
    }

    pub(crate) fn validate_chain_input(
        &mut self,
        schema: SchemaRef<'_>,
        input: &InputArena,
        root: InputId,
        start_depth: usize,
    ) -> Result<ValidatedArena, ValidationError> {
        validate_at_depth_with_context_mode(
            schema,
            input,
            root,
            self.options,
            start_depth,
            ValidationContext {
                definition_scopes: Arc::clone(&self.definition_scopes),
                active_references: Vec::new(),
                enforce_strings_input: false,
                skip_callbacks: self.skip_callbacks,
            },
            self.callbacks,
        )
    }

    pub(crate) fn invoke_validator(
        &mut self,
        slot: usize,
        input: ValidatedArena,
    ) -> Result<InputArena, ValidationError> {
        self.callbacks
            .ok_or_else(|| {
                scalars::type_error(
                    "validator_unavailable",
                    "Validator schema requires a checked callback table",
                    "validator-aware validation entry point",
                )
            })?
            .invoke(slot, input)
    }

    pub(crate) fn push_definition_scope(&mut self, scope: DefinitionScope) {
        Arc::make_mut(&mut self.definition_scopes).push(scope);
    }

    pub(crate) fn pop_definition_scope(&mut self) {
        Arc::make_mut(&mut self.definition_scopes).pop();
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
