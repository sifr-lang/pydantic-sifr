use core::fmt;

use serde_json::{Map, Number, Value, json};
use sifr_runtime::json::JsonIntegerProfile;

use crate::{ExtraPolicy, LiteralValue, Schema, validation::TemporalKind};

const MAX_JSON_SCHEMA_DEPTH: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JsonSchemaMode {
    Validation,
    Serialization,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JsonSchemaErrorKind {
    DepthLimit,
    UnsupportedSchema,
    InvalidNumber,
    IntegerPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonSchemaError {
    kind: JsonSchemaErrorKind,
    message: String,
}

impl JsonSchemaError {
    #[must_use]
    pub const fn kind(&self) -> JsonSchemaErrorKind {
        self.kind
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub const fn diagnostic_code(&self) -> Option<&'static str> {
        match self.kind {
            JsonSchemaErrorKind::IntegerPolicy => Some("SIFR-INT-0009"),
            JsonSchemaErrorKind::DepthLimit
            | JsonSchemaErrorKind::UnsupportedSchema
            | JsonSchemaErrorKind::InvalidNumber => None,
        }
    }

    fn new(kind: JsonSchemaErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for JsonSchemaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for JsonSchemaError {}

pub fn generate_json_schema(
    schema: &Schema,
    mode: JsonSchemaMode,
    integer_profile: JsonIntegerProfile,
) -> Result<Value, JsonSchemaError> {
    generate(schema, mode, integer_profile, 0)
}

fn generate(
    schema: &Schema,
    mode: JsonSchemaMode,
    integer_profile: JsonIntegerProfile,
    depth: usize,
) -> Result<Value, JsonSchemaError> {
    if depth > MAX_JSON_SCHEMA_DEPTH {
        return Err(JsonSchemaError::new(
            JsonSchemaErrorKind::DepthLimit,
            "JSON Schema generation exceeded the static schema depth limit",
        ));
    }
    let child = |schema| generate(schema, mode, integer_profile, depth + 1);
    match schema {
        Schema::None => Ok(json!({"type": "null"})),
        Schema::Bool => Ok(json!({"type": "boolean"})),
        Schema::Integer {
            target,
            constraints,
        } => {
            if constraints
                .multiple_of
                .as_ref()
                .is_some_and(|value| value <= &num_bigint::BigInt::from(0_u8))
            {
                return Err(invalid_number(
                    "integer multipleOf must be greater than zero",
                ));
            }
            let (minimum, maximum) = integer_bounds(*target, constraints);
            integer_schema(
                mode,
                integer_profile,
                minimum,
                maximum,
                &constraints.multiple_of,
            )
        }
        Schema::Float(constraints) => {
            let mut output = typed("number");
            insert_optional_float(&mut output, "exclusiveMinimum", constraints.greater_than)?;
            insert_optional_float(&mut output, "minimum", constraints.greater_or_equal)?;
            insert_optional_float(&mut output, "exclusiveMaximum", constraints.less_than)?;
            insert_optional_float(&mut output, "maximum", constraints.less_or_equal)?;
            if constraints
                .multiple_of
                .is_some_and(|value| !value.is_finite() || value <= 0.0)
            {
                return Err(invalid_number(
                    "floating-point multipleOf must be finite and greater than zero",
                ));
            }
            insert_optional_float(&mut output, "multipleOf", constraints.multiple_of)?;
            Ok(Value::Object(output))
        }
        Schema::Decimal(_) => Err(unsupported(
            "decimal JSON Schema representation is not implemented",
        )),
        Schema::Fraction(_) | Schema::Complex(_) => Err(unsupported(
            "specialized numeric JSON Schema representation is not implemented",
        )),
        Schema::String(constraints) => {
            let mut output = typed("string");
            insert_optional_usize(&mut output, "minLength", constraints.min_length);
            insert_optional_usize(&mut output, "maxLength", constraints.max_length);
            if let Some(pattern) = &constraints.pattern {
                output.insert(
                    "pattern".to_owned(),
                    Value::String(pattern.source().to_owned()),
                );
            }
            Ok(Value::Object(output))
        }
        Schema::Bytes(_) => Err(unsupported(
            "byte JSON Schema representation is not implemented",
        )),
        Schema::Temporal(schema) => Ok(json!({
            "type": "string",
            "format": match schema.kind {
                TemporalKind::Date => "date",
                TemporalKind::Time => "time",
                TemporalKind::DateTime => "date-time",
                TemporalKind::Duration => "duration",
            }
        })),
        Schema::Uuid { .. } => Ok(json!({"type": "string", "format": "uuid"})),
        Schema::Url(constraints) => {
            let mut output = typed("string");
            output.insert("format".to_owned(), Value::String("uri".to_owned()));
            insert_optional_usize(&mut output, "maxLength", constraints.max_length);
            Ok(Value::Object(output))
        }
        Schema::Pattern(_) => Ok(json!({"type": "string", "format": "regex"})),
        Schema::Literal(schema) => literal_schema(schema.values(), mode, integer_profile),
        Schema::Enum(schema) => literal_schema(
            &schema
                .variants()
                .iter()
                .map(|variant| variant.input.clone())
                .collect::<Vec<_>>(),
            mode,
            integer_profile,
        ),
        Schema::Nullable(inner) => Ok(json!({"anyOf": [child(inner)?, {"type": "null"}]})),
        Schema::Union(schema) => Ok(json!({
            "anyOf": schema
                .choices()
                .iter()
                .map(|choice| child(&choice.schema))
                .collect::<Result<Vec<_>, _>>()?
        })),
        Schema::TaggedUnion(schema) => Ok(json!({
            "oneOf": schema
                .choices()
                .iter()
                .map(|choice| child(&choice.schema))
                .collect::<Result<Vec<_>, _>>()?
        })),
        Schema::Definitions(_) | Schema::DefinitionRef { .. } => Err(unsupported(
            "definition and recursive JSON Schema generation is not implemented",
        )),
        Schema::Model(model) => {
            let mut properties = Map::new();
            let mut required = Vec::new();
            for field in &model.fields {
                if mode == JsonSchemaMode::Validation && !field.input {
                    continue;
                }
                properties.insert(field.name.to_owned(), child(&field.schema)?);
                if mode == JsonSchemaMode::Serialization || field.default.is_none() {
                    required.push(Value::String(field.name.to_owned()));
                }
            }
            let additional = match &model.extra {
                ExtraPolicy::Forbid => Value::Bool(false),
                ExtraPolicy::Ignore => Value::Bool(true),
                ExtraPolicy::Allow { value_schema, .. } => child(value_schema)?,
            };
            let mut output = typed("object");
            output.insert("title".to_owned(), Value::String(model.name.to_owned()));
            output.insert("properties".to_owned(), Value::Object(properties));
            output.insert("required".to_owned(), Value::Array(required));
            output.insert("additionalProperties".to_owned(), additional);
            Ok(Value::Object(output))
        }
        Schema::List { item, constraints }
        | Schema::Set { item, constraints }
        | Schema::FrozenSet { item, constraints }
        | Schema::Generator { item, constraints } => {
            let mut output = typed("array");
            output.insert("items".to_owned(), child(item)?);
            insert_optional_usize(&mut output, "minItems", constraints.min_length);
            insert_optional_usize(&mut output, "maxItems", constraints.max_length);
            if matches!(schema, Schema::Set { .. } | Schema::FrozenSet { .. }) {
                output.insert("uniqueItems".to_owned(), Value::Bool(true));
            }
            Ok(Value::Object(output))
        }
        Schema::Tuple(items) => Ok(json!({
            "type": "array",
            "prefixItems": items
                .iter()
                .map(&child)
                .collect::<Result<Vec<_>, _>>()?,
            "minItems": items.len(),
            "maxItems": items.len()
        })),
        Schema::Mapping {
            value, constraints, ..
        } => {
            let mut output = typed("object");
            output.insert("additionalProperties".to_owned(), child(value)?);
            insert_optional_usize(&mut output, "minProperties", constraints.min_length);
            insert_optional_usize(&mut output, "maxProperties", constraints.max_length);
            Ok(Value::Object(output))
        }
        Schema::EmbeddedJson(inner) => match mode {
            JsonSchemaMode::Validation => Ok(json!({
                "type": "string",
                "contentMediaType": "application/json",
                "contentSchema": child(inner)?
            })),
            JsonSchemaMode::Serialization => child(inner),
        },
        Schema::LaxOrStrict(schema) => child(match mode {
            JsonSchemaMode::Validation => schema.lax(),
            JsonSchemaMode::Serialization => schema.strict(),
        }),
        Schema::JsonOrStructural(schema) => child(match mode {
            JsonSchemaMode::Validation => schema.json(),
            JsonSchemaMode::Serialization => schema.structural(),
        }),
        Schema::Chain(schema) => {
            let selected = match mode {
                JsonSchemaMode::Validation => schema.steps().first(),
                JsonSchemaMode::Serialization => schema.steps().last(),
            }
            .ok_or_else(|| unsupported("empty typed chain has no JSON Schema representation"))?;
            child(selected)
        }
    }
}

fn typed(name: &str) -> Map<String, Value> {
    let mut output = Map::new();
    output.insert("type".to_owned(), Value::String(name.to_owned()));
    output
}

fn literal_schema(
    values: &[LiteralValue],
    mode: JsonSchemaMode,
    profile: JsonIntegerProfile,
) -> Result<Value, JsonSchemaError> {
    let integer_values = values
        .iter()
        .filter_map(|value| match value {
            LiteralValue::Integer(value) => Some(value),
            LiteralValue::None
            | LiteralValue::Bool(_)
            | LiteralValue::String(_)
            | LiteralValue::Bytes(_) => None,
        })
        .collect::<Vec<_>>();
    if profile == JsonIntegerProfile::Web
        && integer_values
            .iter()
            .any(|value| !is_javascript_safe_integer(value))
    {
        return Err(JsonSchemaError::new(
            JsonSchemaErrorKind::IntegerPolicy,
            "SIFR-INT-0009: json.web integer literal is outside the JavaScript-safe range",
        ));
    }
    let values = values
        .iter()
        .map(|value| literal_value(value, mode, profile))
        .collect::<Result<Vec<_>, _>>()?;
    let value_count = values.len();
    let mut output = Map::new();
    if let [value] = values.as_slice() {
        output.insert("const".to_owned(), value.clone());
    } else {
        output.insert("enum".to_owned(), Value::Array(values));
    }
    if !integer_values.is_empty() {
        let profile_name = match profile {
            JsonIntegerProfile::Exact => "exact",
            JsonIntegerProfile::Web => "web",
            JsonIntegerProfile::StringInts => "string_ints",
        };
        output.insert(
            "x-sifr-integer-profile".to_owned(),
            Value::String(profile_name.to_owned()),
        );
        let minimum = integer_values.iter().copied().min().cloned();
        let maximum = integer_values.iter().copied().max().cloned();
        insert_optional_big_integer(&mut output, "x-sifr-minimum", &minimum)?;
        insert_optional_big_integer(&mut output, "x-sifr-maximum", &maximum)?;
        if profile == JsonIntegerProfile::Exact {
            output.insert(
                "x-sifr-generated-client-warning".to_owned(),
                Value::String(
                    "client must use an exact integer JSON parser for this field".to_owned(),
                ),
            );
        }
        if mode == JsonSchemaMode::Serialization
            && profile == JsonIntegerProfile::StringInts
            && integer_values.len() == value_count
        {
            output.insert(
                "x-sifr-format".to_owned(),
                Value::String("integer-decimal-string".to_owned()),
            );
        }
    }
    Ok(Value::Object(output))
}

fn literal_value(
    value: &LiteralValue,
    mode: JsonSchemaMode,
    profile: JsonIntegerProfile,
) -> Result<Value, JsonSchemaError> {
    match value {
        LiteralValue::None => Ok(Value::Null),
        LiteralValue::Bool(value) => Ok(Value::Bool(*value)),
        LiteralValue::Integer(value)
            if mode == JsonSchemaMode::Serialization
                && profile == JsonIntegerProfile::StringInts =>
        {
            Ok(Value::String(value.to_string()))
        }
        LiteralValue::Integer(value) => big_integer_value(value),
        LiteralValue::String(value) => Ok(Value::String(value.clone())),
        LiteralValue::Bytes(_) => Err(unsupported(
            "byte literal JSON Schema needs an explicit byte representation",
        )),
    }
}

fn insert_optional_usize(output: &mut Map<String, Value>, key: &str, value: Option<usize>) {
    if let Some(value) = value {
        output.insert(key.to_owned(), json!(value));
    }
}

fn insert_optional_big_integer(
    output: &mut Map<String, Value>,
    key: &str,
    value: &Option<num_bigint::BigInt>,
) -> Result<(), JsonSchemaError> {
    if let Some(value) = value {
        insert_big_integer(output, key, value)?;
    }
    Ok(())
}

fn insert_big_integer(
    output: &mut Map<String, Value>,
    key: &str,
    value: &num_bigint::BigInt,
) -> Result<(), JsonSchemaError> {
    output.insert(key.to_owned(), big_integer_value(value)?);
    Ok(())
}

fn big_integer_value(value: &num_bigint::BigInt) -> Result<Value, JsonSchemaError> {
    serde_json::from_str(&value.to_string()).map_err(|_| {
        JsonSchemaError::new(
            JsonSchemaErrorKind::InvalidNumber,
            "integer constraint cannot be represented exactly in JSON Schema",
        )
    })
}

fn insert_optional_float(
    output: &mut Map<String, Value>,
    key: &str,
    value: Option<f64>,
) -> Result<(), JsonSchemaError> {
    if let Some(value) = value {
        let value = Number::from_f64(value).ok_or_else(|| {
            JsonSchemaError::new(
                JsonSchemaErrorKind::InvalidNumber,
                "non-finite constraint cannot be represented in JSON Schema",
            )
        })?;
        output.insert(key.to_owned(), Value::Number(value));
    }
    Ok(())
}

fn integer_schema(
    mode: JsonSchemaMode,
    profile: JsonIntegerProfile,
    minimum: Option<num_bigint::BigInt>,
    maximum: Option<num_bigint::BigInt>,
    multiple_of: &Option<num_bigint::BigInt>,
) -> Result<Value, JsonSchemaError> {
    if profile == JsonIntegerProfile::Web && !is_javascript_safe_range(&minimum, &maximum) {
        return Err(JsonSchemaError::new(
            JsonSchemaErrorKind::IntegerPolicy,
            "SIFR-INT-0009: json.web integer schema needs a statically JavaScript-safe range or an explicit string integer profile",
        ));
    }

    if mode == JsonSchemaMode::Serialization && profile == JsonIntegerProfile::StringInts {
        let mut output = typed("string");
        output.insert("pattern".to_owned(), Value::String("^-?[0-9]+$".to_owned()));
        output.insert(
            "x-sifr-format".to_owned(),
            Value::String("integer-decimal-string".to_owned()),
        );
        output.insert(
            "x-sifr-integer-profile".to_owned(),
            Value::String("string_ints".to_owned()),
        );
        insert_optional_big_integer(&mut output, "x-sifr-minimum", &minimum)?;
        insert_optional_big_integer(&mut output, "x-sifr-maximum", &maximum)?;
        insert_optional_big_integer(&mut output, "x-sifr-multiple-of", multiple_of)?;
        return Ok(Value::Object(output));
    }

    let mut output = typed("integer");
    insert_optional_big_integer(&mut output, "minimum", &minimum)?;
    insert_optional_big_integer(&mut output, "maximum", &maximum)?;
    insert_optional_big_integer(&mut output, "multipleOf", multiple_of)?;
    let profile_name = match profile {
        JsonIntegerProfile::Exact => "exact",
        JsonIntegerProfile::Web => "web",
        JsonIntegerProfile::StringInts => "string_ints",
    };
    output.insert(
        "x-sifr-integer-profile".to_owned(),
        Value::String(profile_name.to_owned()),
    );
    if profile == JsonIntegerProfile::Exact {
        output.insert(
            "x-sifr-generated-client-warning".to_owned(),
            Value::String("client must use an exact integer JSON parser for this field".to_owned()),
        );
    }
    Ok(Value::Object(output))
}

fn integer_bounds(
    target: crate::IntegerTarget,
    constraints: &crate::IntegerConstraints,
) -> (Option<num_bigint::BigInt>, Option<num_bigint::BigInt>) {
    let (target_minimum, target_maximum) =
        target.bounds().map_or((None, None), |(minimum, maximum)| {
            (Some(minimum), Some(maximum))
        });
    let exclusive_minimum = constraints
        .greater_than
        .as_ref()
        .map(|value| value + num_bigint::BigInt::from(1_u8));
    let exclusive_maximum = constraints
        .less_than
        .as_ref()
        .map(|value| value - num_bigint::BigInt::from(1_u8));
    (
        maximum_big_integer(
            maximum_big_integer(target_minimum, constraints.greater_or_equal.clone()),
            exclusive_minimum,
        ),
        minimum_big_integer(
            minimum_big_integer(target_maximum, constraints.less_or_equal.clone()),
            exclusive_maximum,
        ),
    )
}

fn is_javascript_safe_range(
    minimum: &Option<num_bigint::BigInt>,
    maximum: &Option<num_bigint::BigInt>,
) -> bool {
    let safe = num_bigint::BigInt::from(9_007_199_254_740_991_i64);
    matches!((minimum, maximum), (Some(minimum), Some(maximum)) if minimum >= &-safe.clone() && maximum <= &safe)
}

fn is_javascript_safe_integer(value: &num_bigint::BigInt) -> bool {
    let safe = num_bigint::BigInt::from(9_007_199_254_740_991_i64);
    value >= &-safe.clone() && value <= &safe
}

fn maximum_big_integer(
    left: Option<num_bigint::BigInt>,
    right: Option<num_bigint::BigInt>,
) -> Option<num_bigint::BigInt> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (left, right) => left.or(right),
    }
}

fn minimum_big_integer(
    left: Option<num_bigint::BigInt>,
    right: Option<num_bigint::BigInt>,
) -> Option<num_bigint::BigInt> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (left, right) => left.or(right),
    }
}

fn invalid_number(message: &'static str) -> JsonSchemaError {
    JsonSchemaError::new(JsonSchemaErrorKind::InvalidNumber, message)
}

fn unsupported(message: &'static str) -> JsonSchemaError {
    JsonSchemaError::new(JsonSchemaErrorKind::UnsupportedSchema, message)
}
