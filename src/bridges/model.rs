use core::{cell::RefCell, fmt, marker::PhantomData};

use pydantic_sifr_core::{
    CallbackOutputSink, JsonIntegerProfile, JsonLimits, JsonSchemaError, JsonSchemaMode,
    JsonSchemaOptions, PreparedSchema, SelectionPath, SerializationCallbacks, SerializationOptions,
    SerializationPlan, ValidationCallbacks, ValidationError, ValidationOptions,
    generate_prepared_json_schema_bytes, serialize_json, serialize_json_with_callbacks,
    serialize_structural, serialize_structural_with_callbacks, serializer_callback_error,
    validate_json_and_construct, validate_json_and_construct_with_callbacks,
    validate_structural_and_construct, validate_structural_and_construct_with_callbacks,
    validate_structural_strings_and_construct,
    validate_structural_strings_and_construct_with_callbacks, validator_callback_error,
};
use sifr_runtime::interop::structural::{
    MethodSlotTable, StaticProgramType, StructuralConstruct, StructuralProject, StructuralType,
};

#[derive(Debug)]
pub struct ModelValidationError {
    errors_json: String,
}

impl ModelValidationError {
    fn from_validation(error: &ValidationError) -> Self {
        Self {
            errors_json: validation_error_json(error),
        }
    }
}

impl fmt::Display for ModelValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.errors_json)
    }
}

impl std::error::Error for ModelValidationError {}

#[derive(Debug)]
pub struct ModelSerializationError {
    details_json: String,
}

impl fmt::Display for ModelSerializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.details_json)
    }
}

impl std::error::Error for ModelSerializationError {}

#[derive(Debug)]
pub struct ModelJsonSchemaError {
    details_json: String,
}

impl fmt::Display for ModelJsonSchemaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.details_json)
    }
}

impl std::error::Error for ModelJsonSchemaError {}

pub fn validate_json<T>(payload: &[u8], strict: Option<bool>) -> Result<T, ModelValidationError>
where
    T: StructuralConstruct + StaticProgramType,
{
    let schema = PreparedSchema::from_static::<T>()
        .map_err(|error| ModelValidationError::from_validation(&error))?;
    validate_json_and_construct(
        &schema,
        payload,
        JsonLimits::default(),
        validation_options(strict),
    )
    .map_err(|error| ModelValidationError::from_validation(&error))
}

pub fn validate_strings<Input, T>(
    payload: &Input,
    strict: Option<bool>,
) -> Result<T, ModelValidationError>
where
    Input: StructuralProject,
    T: StructuralConstruct + StaticProgramType,
{
    let schema = PreparedSchema::from_static::<T>()
        .map_err(|error| ModelValidationError::from_validation(&error))?;
    validate_structural_strings_and_construct(
        &schema,
        payload,
        JsonLimits::default(),
        validation_options(strict),
    )
    .map_err(|error| ModelValidationError::from_validation(&error))
}

struct SlotCallbacks<'context, Context, T> {
    context: RefCell<&'context mut Context>,
    marker: PhantomData<fn() -> T>,
}

impl<'context, Context, T> SlotCallbacks<'context, Context, T> {
    fn new(context: &'context mut Context) -> Self {
        Self {
            context: RefCell::new(context),
            marker: PhantomData,
        }
    }
}

impl<Context, T> ValidationCallbacks for SlotCallbacks<'_, Context, T>
where
    Context: StructuralType,
    T: MethodSlotTable<Context>,
{
    fn invoke(
        &self,
        slot: usize,
        input: pydantic_sifr_core::ValidatedArena,
    ) -> Result<pydantic_sifr_core::InputArena, ValidationError> {
        let signature = T::slot_signatures()
            .get(slot)
            .ok_or_else(|| callback_error("Validator slot is out of range"))?;
        let input = input
            .into_structural_arena(signature.input())
            .map_err(|error| callback_error(&error.to_string()))?;
        let mut sink = CallbackOutputSink::new(JsonLimits::default())
            .map_err(|error| callback_error(&error.to_string()))?;
        let mut context = self.context.try_borrow_mut().map_err(|_| {
            callback_error("Validator context is already borrowed by an active callback")
        })?;
        T::invoke_slot(slot, input, &mut **context, None, &mut sink)
            .map_err(|error| callback_error(&error.to_string()))?;
        sink.finish()
            .map_err(|error| callback_error(&error.to_string()))
    }
}

impl<Context, T> SerializationCallbacks for SlotCallbacks<'_, Context, T>
where
    Context: StructuralType,
    T: MethodSlotTable<Context>,
{
    fn invoke(
        &self,
        slot: usize,
        input: pydantic_sifr_core::ValidatedArena,
    ) -> Result<pydantic_sifr_core::InputArena, pydantic_sifr_core::SerializationError> {
        let signature = T::slot_signatures()
            .get(slot)
            .ok_or_else(|| serializer_callback_error("Serializer slot is out of range"))?;
        let input = input
            .into_structural_arena(signature.input())
            .map_err(|error| serializer_callback_error(error.to_string()))?;
        let mut sink = CallbackOutputSink::new(JsonLimits::default())
            .map_err(|error| serializer_callback_error(error.to_string()))?;
        let mut context = self.context.try_borrow_mut().map_err(|_| {
            serializer_callback_error(
                "Serializer context is already borrowed by an active callback",
            )
        })?;
        T::invoke_slot(slot, input, &mut **context, None, &mut sink)
            .map_err(|error| serializer_callback_error(error.to_string()))?;
        sink.finish()
            .map_err(|error| serializer_callback_error(error.to_string()))
    }
}

pub fn validate_with_validators<Context, Input, T>(
    payload: &Input,
    context: &mut Context,
    strict: Option<bool>,
) -> Result<T, ModelValidationError>
where
    Context: StructuralType,
    Input: StructuralProject,
    T: StructuralConstruct + StaticProgramType + MethodSlotTable<Context>,
{
    let schema = validator_schema::<Context, T>()?;
    let callbacks = SlotCallbacks::<Context, T>::new(context);
    validate_structural_and_construct_with_callbacks(
        &schema,
        payload,
        JsonLimits::default(),
        validation_options(strict),
        &callbacks,
    )
    .map_err(|error| ModelValidationError::from_validation(&error))
}

pub fn validate_json_with_validators<Context, T>(
    payload: &[u8],
    context: &mut Context,
    strict: Option<bool>,
) -> Result<T, ModelValidationError>
where
    Context: StructuralType,
    T: StructuralConstruct + StaticProgramType + MethodSlotTable<Context>,
{
    let schema = validator_schema::<Context, T>()?;
    let callbacks = SlotCallbacks::<Context, T>::new(context);
    validate_json_and_construct_with_callbacks(
        &schema,
        payload,
        JsonLimits::default(),
        validation_options(strict),
        &callbacks,
    )
    .map_err(|error| ModelValidationError::from_validation(&error))
}

pub fn validate_strings_with_validators<Context, Input, T>(
    payload: &Input,
    context: &mut Context,
    strict: Option<bool>,
) -> Result<T, ModelValidationError>
where
    Context: StructuralType,
    Input: StructuralProject,
    T: StructuralConstruct + StaticProgramType + MethodSlotTable<Context>,
{
    let schema = validator_schema::<Context, T>()?;
    let callbacks = SlotCallbacks::<Context, T>::new(context);
    validate_structural_strings_and_construct_with_callbacks(
        &schema,
        payload,
        JsonLimits::default(),
        validation_options(strict),
        &callbacks,
    )
    .map_err(|error| ModelValidationError::from_validation(&error))
}

fn validator_schema<Context, T>() -> Result<PreparedSchema<'static>, ModelValidationError>
where
    Context: StructuralType,
    T: StructuralConstruct + StaticProgramType + MethodSlotTable<Context>,
{
    let program = T::static_program();
    if program.header().slot_table_identity() != Some(T::slot_table_identity()) {
        return Err(ModelValidationError::from_validation(&callback_error(
            "Validator slot table does not match the static schema program",
        )));
    }
    PreparedSchema::from_static::<T>()
        .map_err(|error| ModelValidationError::from_validation(&error))
}

fn callback_error(message: &str) -> ValidationError {
    validator_callback_error(message)
}

fn validation_options(strict: Option<bool>) -> ValidationOptions {
    ValidationOptions {
        strict_override: strict,
        ..ValidationOptions::default()
    }
}

pub fn validate<Input, T>(payload: &Input, strict: Option<bool>) -> Result<T, ModelValidationError>
where
    T: StructuralConstruct + StaticProgramType,
    Input: StructuralProject,
{
    let schema = PreparedSchema::from_static::<T>()
        .map_err(|error| ModelValidationError::from_validation(&error))?;
    validate_structural_and_construct(
        &schema,
        payload,
        JsonLimits::default(),
        validation_options(strict),
    )
    .map_err(|error| ModelValidationError::from_validation(&error))
}

pub fn dump_json<T>(
    value: &T,
    include: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
    by_alias: bool,
    exclude_none: bool,
    exclude_defaults: bool,
    integer_profile: &str,
) -> Result<Vec<u8>, ModelSerializationError>
where
    T: StructuralProject + StaticProgramType,
{
    let schema = PreparedSchema::from_static::<T>().map_err(serialization_setup_error)?;
    let plan = SerializationPlan::from_prepared(schema, parse_integer_profile(integer_profile)?)
        .map_err(serialization_plan_error)?;
    ensure_no_serializer_callbacks(&plan)?;
    let options = serialization_options(include, exclude, by_alias, exclude_none, exclude_defaults);
    serialize_json(&plan, value, &options).map_err(serialization_error)
}

#[allow(
    clippy::too_many_arguments,
    reason = "the compiler-bound bridge mirrors the checked Sifr API"
)]
pub fn dump_json_with_serializers<Context, T>(
    value: &T,
    context: &mut Context,
    include: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
    by_alias: bool,
    exclude_none: bool,
    exclude_defaults: bool,
    integer_profile: &str,
) -> Result<Vec<u8>, ModelSerializationError>
where
    Context: StructuralType,
    T: StructuralProject + StaticProgramType + MethodSlotTable<Context>,
{
    let schema = serializer_schema::<Context, T>()?;
    let plan = SerializationPlan::from_prepared(schema, parse_integer_profile(integer_profile)?)
        .map_err(serialization_plan_error)?;
    let options = serialization_options(include, exclude, by_alias, exclude_none, exclude_defaults);
    let callbacks = SlotCallbacks::<Context, T>::new(context);
    serialize_json_with_callbacks(schema, &plan, value, &options, &callbacks)
        .map_err(serialization_error)
}

pub fn dump<Output, T>(
    value: &T,
    include: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
    by_alias: bool,
    exclude_none: bool,
    exclude_defaults: bool,
    integer_profile: &str,
) -> Result<Output, ModelSerializationError>
where
    T: StructuralProject + StaticProgramType,
    Output: StructuralConstruct + StructuralType,
{
    let schema = PreparedSchema::from_static::<T>().map_err(serialization_setup_error)?;
    let plan = SerializationPlan::from_prepared(schema, parse_integer_profile(integer_profile)?)
        .map_err(serialization_plan_error)?;
    ensure_no_serializer_callbacks(&plan)?;
    let options = serialization_options(include, exclude, by_alias, exclude_none, exclude_defaults);
    let value = serialize_structural(&plan, value, &options).map_err(serialization_error)?;
    super::json_value::construct_json_object(&value).map_err(serialization_message)
}

#[allow(
    clippy::too_many_arguments,
    reason = "the compiler-bound bridge mirrors the checked Sifr API"
)]
pub fn dump_with_serializers<Context, Output, T>(
    value: &T,
    context: &mut Context,
    include: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
    by_alias: bool,
    exclude_none: bool,
    exclude_defaults: bool,
    integer_profile: &str,
) -> Result<Output, ModelSerializationError>
where
    Context: StructuralType,
    T: StructuralProject + StaticProgramType + MethodSlotTable<Context>,
    Output: StructuralConstruct + StructuralType,
{
    let schema = serializer_schema::<Context, T>()?;
    let plan = SerializationPlan::from_prepared(schema, parse_integer_profile(integer_profile)?)
        .map_err(serialization_plan_error)?;
    let options = serialization_options(include, exclude, by_alias, exclude_none, exclude_defaults);
    let callbacks = SlotCallbacks::<Context, T>::new(context);
    let value = serialize_structural_with_callbacks(schema, &plan, value, &options, &callbacks)
        .map_err(serialization_error)?;
    super::json_value::construct_json_object(&value).map_err(serialization_message)
}

fn serializer_schema<Context, T>() -> Result<PreparedSchema<'static>, ModelSerializationError>
where
    Context: StructuralType,
    T: StructuralProject + StaticProgramType + MethodSlotTable<Context>,
{
    let program = T::static_program();
    if program.header().slot_table_identity() != Some(T::slot_table_identity()) {
        return Err(serialization_message(
            "Serializer slot table does not match the static schema program",
        ));
    }
    PreparedSchema::from_static::<T>().map_err(serialization_setup_error)
}

fn serialization_options(
    include: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
    by_alias: bool,
    exclude_none: bool,
    exclude_defaults: bool,
) -> SerializationOptions {
    SerializationOptions {
        by_alias,
        exclude_none,
        exclude_defaults,
        include: include
            .unwrap_or_default()
            .into_iter()
            .map(SelectionPath::field)
            .collect(),
        exclude: exclude
            .unwrap_or_default()
            .into_iter()
            .map(SelectionPath::field)
            .collect(),
        ..SerializationOptions::default()
    }
}

fn serialization_plan_error(
    error: pydantic_sifr_core::SerializationPlanError,
) -> ModelSerializationError {
    serialization_detail("serialization.plan", error.message(), &[], &[])
}

fn parse_integer_profile(
    integer_profile: &str,
) -> Result<JsonIntegerProfile, ModelSerializationError> {
    JsonIntegerProfile::from_name(integer_profile).ok_or_else(|| {
        serialization_detail(
            "serialization.configuration",
            "integer_profile must be exact, web, or string_ints",
            &[],
            &[("integer_profile", integer_profile)],
        )
    })
}

fn ensure_no_serializer_callbacks(plan: &SerializationPlan) -> Result<(), ModelSerializationError> {
    if plan.callbacks().is_empty() {
        Ok(())
    } else {
        Err(serialization_message(
            "Serializer handlers require model_dump_with_serializers or model_dump_json_with_serializers",
        ))
    }
}

fn serialization_error(error: pydantic_sifr_core::SerializationError) -> ModelSerializationError {
    let kind = format!("{:?}", error.kind());
    if let Some(integer) = error.integer_range_error() {
        return serialization_detail(
            "serialization.integer_range",
            error.message(),
            &[integer.path()],
            &[("kind", kind.as_str()), ("profile", integer.profile())],
        );
    }
    serialization_detail(
        serialization_code(error.kind()),
        error.message(),
        &[],
        &[("kind", kind.as_str())],
    )
}

fn serialization_message(message: impl Into<String>) -> ModelSerializationError {
    let message = message.into();
    serialization_detail("serialization.configuration", &message, &[], &[])
}

pub fn json_schema<T>(
    serialization: bool,
    by_alias: bool,
    integer_profile: &str,
) -> Result<Vec<u8>, ModelJsonSchemaError>
where
    T: StaticProgramType,
{
    let schema = PreparedSchema::from_static::<T>().map_err(json_schema_setup_error)?;
    let mode = if serialization {
        JsonSchemaMode::Serialization
    } else {
        JsonSchemaMode::Validation
    };
    let profile = JsonIntegerProfile::from_name(integer_profile).ok_or_else(|| {
        json_schema_detail(
            "json_schema.configuration",
            "integer_profile must be exact, web, or string_ints",
            &[("integer_profile", integer_profile)],
        )
    })?;
    generate_prepared_json_schema_bytes(&schema, JsonSchemaOptions::new(mode, by_alias), profile)
        .map_err(json_schema_error)
}

fn serialization_setup_error(error: ValidationError) -> ModelSerializationError {
    serialization_detail("serialization.schema", &error.to_string(), &[], &[])
}

fn serialization_code(kind: pydantic_sifr_core::SerializationErrorKind) -> &'static str {
    use pydantic_sifr_core::SerializationErrorKind;

    match kind {
        SerializationErrorKind::ShapeMismatch => "serialization.shape_mismatch",
        SerializationErrorKind::InvalidProjection => "serialization.invalid_projection",
        SerializationErrorKind::Limit => "serialization.limit",
        SerializationErrorKind::UnsupportedJsonValue => "serialization.unsupported_json_value",
        SerializationErrorKind::IntegerRange => "serialization.integer_range",
        SerializationErrorKind::Callback => "serialization.callback",
    }
}

fn serialization_detail(
    code: &str,
    message: &str,
    location: &[&str],
    context: &[(&str, &str)],
) -> ModelSerializationError {
    ModelSerializationError {
        details_json: structured_error_json(code, message, location, context),
    }
}

fn json_schema_setup_error(error: ValidationError) -> ModelJsonSchemaError {
    json_schema_detail("json_schema.schema", &error.to_string(), &[])
}

fn json_schema_error(error: JsonSchemaError) -> ModelJsonSchemaError {
    let kind = format!("{:?}", error.kind());
    let mut context = vec![("kind", kind.as_str())];
    if let Some(diagnostic) = error.diagnostic_code() {
        context.push(("diagnostic", diagnostic));
    }
    json_schema_detail(json_schema_code(error.kind()), error.message(), &context)
}

fn json_schema_code(kind: pydantic_sifr_core::JsonSchemaErrorKind) -> &'static str {
    use pydantic_sifr_core::JsonSchemaErrorKind;

    match kind {
        JsonSchemaErrorKind::DepthLimit => "json_schema.depth_limit",
        JsonSchemaErrorKind::UnsupportedSchema => "json_schema.unsupported_schema",
        JsonSchemaErrorKind::InvalidNumber => "json_schema.invalid_number",
        JsonSchemaErrorKind::IntegerPolicy => "json_schema.integer_policy",
    }
}

fn json_schema_detail(code: &str, message: &str, context: &[(&str, &str)]) -> ModelJsonSchemaError {
    ModelJsonSchemaError {
        details_json: structured_error_json(code, message, &[], context),
    }
}

fn structured_error_json(
    code: &str,
    message: &str,
    location: &[&str],
    context: &[(&str, &str)],
) -> String {
    let location = location
        .iter()
        .map(|item| format!("\"{}\"", escape_json(item)))
        .collect::<Vec<_>>()
        .join(",");
    let context = context
        .iter()
        .map(|(key, value)| format!("\"{}\":\"{}\"", escape_json(key), escape_json(value)))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"code\":\"{}\",\"message\":\"{}\",\"location\":[{}],\"context\":{{{}}}}}",
        escape_json(code),
        escape_json(message),
        location,
        context,
    )
}

fn validation_error_json(error: &ValidationError) -> String {
    let details = error
        .details()
        .iter()
        .map(|detail| {
            let location = detail
                .location
                .iter()
                .map(location_item_json)
                .collect::<Vec<_>>()
                .join(",");
            let context = detail
                .context
                .iter()
                .map(|(key, value)| format!("\"{}\":\"{}\"", escape_json(key), escape_json(value)))
                .collect::<Vec<_>>()
                .join(",");
            let context = if context.is_empty() {
                String::new()
            } else {
                format!(",\"context\":{{{context}}}")
            };
            format!(
                "{{\"code\":\"{}\",\"message\":\"{}\",\"expected\":\"{}\",\"location\":[{}]{}}}",
                escape_json(detail.code),
                escape_json(&detail.message),
                escape_json(&detail.expected),
                location,
                context,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"errors\":[{}],\"truncated\":{}}}",
        details,
        error.is_truncated(),
    )
}

fn location_item_json(item: &pydantic_sifr_core::LocationItem) -> String {
    match item {
        pydantic_sifr_core::LocationItem::Field(value)
        | pydantic_sifr_core::LocationItem::Branch(value) => {
            format!("\"{}\"", escape_json(value))
        }
        pydantic_sifr_core::LocationItem::Index(value)
        | pydantic_sifr_core::LocationItem::MappingKey(value) => value.to_string(),
    }
}

fn escape_json(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\u{08}' => escaped.push_str("\\b"),
            '\u{0c}' => escaped.push_str("\\f"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            control if control <= '\u{1f}' => {
                let value = u32::from(control);
                escaped.push_str("\\u00");
                escaped.push(hex_digit(value >> 4));
                escaped.push(hex_digit(value & 0x0f));
            }
            character => escaped.push(character),
        }
    }
    escaped
}

const fn hex_digit(value: u32) -> char {
    match value {
        0 => '0',
        1 => '1',
        2 => '2',
        3 => '3',
        4 => '4',
        5 => '5',
        6 => '6',
        7 => '7',
        8 => '8',
        9 => '9',
        10 => 'a',
        11 => 'b',
        12 => 'c',
        13 => 'd',
        14 => 'e',
        15 => 'f',
        _ => '0',
    }
}

#[cfg(test)]
mod tests {
    use pydantic_sifr_core::{LocationItem, validator_callback_error};

    use super::{
        escape_json, json_schema_detail, location_item_json, serialization_detail,
        validation_error_json,
    };

    #[test]
    fn json_escape_covers_every_control_character() {
        let controls = (0_u8..=0x1f).map(char::from).collect::<String>();
        let escaped = escape_json(&controls);
        assert_eq!(
            escaped,
            "\\u0000\\u0001\\u0002\\u0003\\u0004\\u0005\\u0006\\u0007\\b\\t\\n\\u000b\\f\\r\\u000e\\u000f\\u0010\\u0011\\u0012\\u0013\\u0014\\u0015\\u0016\\u0017\\u0018\\u0019\\u001a\\u001b\\u001c\\u001d\\u001e\\u001f"
        );
    }

    #[test]
    fn branch_locations_use_the_stable_string_representation() {
        assert_eq!(
            location_item_json(&LocationItem::Branch("int\"choice".to_owned())),
            "\"int\\\"choice\""
        );
    }

    #[test]
    fn callback_context_is_exposed_in_public_error_json() {
        let error = validator_callback_error("code is rejected");
        assert_eq!(
            validation_error_json(&error),
            "{\"errors\":[{\"code\":\"validator_error\",\"message\":\"Validator callback failed: code is rejected\",\"expected\":\"successful checked validator\",\"location\":[],\"context\":{\"error\":\"code is rejected\"}}],\"truncated\":false}"
        );
    }

    #[test]
    fn serialization_errors_expose_code_message_location_and_context() {
        let error = serialization_detail(
            "serialization.integer_range",
            "integer is outside the selected profile",
            &["$.amount"],
            &[("profile", "json.web")],
        );
        assert_eq!(
            error.to_string(),
            "{\"code\":\"serialization.integer_range\",\"message\":\"integer is outside the selected profile\",\"location\":[\"$.amount\"],\"context\":{\"profile\":\"json.web\"}}"
        );
    }

    #[test]
    fn json_schema_errors_expose_code_message_location_and_context() {
        let error = json_schema_detail(
            "json_schema.configuration",
            "invalid profile",
            &[("integer_profile", "unknown")],
        );
        assert_eq!(
            error.to_string(),
            "{\"code\":\"json_schema.configuration\",\"message\":\"invalid profile\",\"location\":[],\"context\":{\"integer_profile\":\"unknown\"}}"
        );
    }
}
