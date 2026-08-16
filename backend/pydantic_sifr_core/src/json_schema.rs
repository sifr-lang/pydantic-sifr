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
            if integer_profile != JsonIntegerProfile::Exact {
                return Err(unsupported(
                    "non-exact integer JSON profiles need an explicit schema representation",
                ));
            }
            let mut output = typed("integer");
            let (target_minimum, target_maximum) =
                target.bounds().map_or((None, None), |(minimum, maximum)| {
                    (Some(minimum), Some(maximum))
                });
            let minimum = maximum_big_integer(target_minimum, constraints.greater_or_equal.clone());
            let maximum = minimum_big_integer(target_maximum, constraints.less_or_equal.clone());
            insert_optional_big_integer(&mut output, "minimum", &minimum)?;
            insert_optional_big_integer(&mut output, "maximum", &maximum)?;
            insert_optional_big_integer(
                &mut output,
                "exclusiveMinimum",
                &constraints.greater_than,
            )?;
            insert_optional_big_integer(&mut output, "exclusiveMaximum", &constraints.less_than)?;
            if constraints
                .multiple_of
                .as_ref()
                .is_some_and(|value| value <= &num_bigint::BigInt::from(0_u8))
            {
                return Err(invalid_number(
                    "integer multipleOf must be greater than zero",
                ));
            }
            insert_optional_big_integer(&mut output, "multipleOf", &constraints.multiple_of)?;
            Ok(Value::Object(output))
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
        Schema::Literal(schema) => literal_schema(schema.values()),
        Schema::Enum(schema) => literal_schema(
            &schema
                .variants()
                .iter()
                .map(|variant| variant.input.clone())
                .collect::<Vec<_>>(),
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

fn literal_schema(values: &[LiteralValue]) -> Result<Value, JsonSchemaError> {
    let values = values
        .iter()
        .map(literal_value)
        .collect::<Result<Vec<_>, _>>()?;
    if let [value] = values.as_slice() {
        Ok(json!({"const": value}))
    } else {
        Ok(json!({"enum": values}))
    }
}

fn literal_value(value: &LiteralValue) -> Result<Value, JsonSchemaError> {
    match value {
        LiteralValue::None => Ok(Value::Null),
        LiteralValue::Bool(value) => Ok(Value::Bool(*value)),
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
