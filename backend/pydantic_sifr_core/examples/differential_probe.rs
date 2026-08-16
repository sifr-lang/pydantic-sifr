use std::error::Error;
use std::io::{self, Write};

use pydantic_sifr_core::{
    CollectionConstraints, ErrorDetail, InputProfile, JsonLimits, LocationItem, Schema,
    StringConstraints, StringPattern, ValidatedArena, ValidatedValue, ValidationError,
    ValidationOptions, parse_json, validate,
};
use serde_json::{Value, json};

fn location_value(item: &LocationItem) -> Value {
    match item {
        LocationItem::Field(value) | LocationItem::Branch(value) => json!(value),
        LocationItem::Index(value) | LocationItem::MappingKey(value) => json!(value),
    }
}

fn validated_value(arena: &ValidatedArena, value: &ValidatedValue) -> Result<Value, io::Error> {
    match value {
        ValidatedValue::ExactInt(value) => value
            .to_string()
            .parse::<i64>()
            .map(Value::from)
            .map_err(io::Error::other),
        ValidatedValue::String(value) => Ok(json!(value)),
        ValidatedValue::Sequence(items) => items
            .iter()
            .map(|id| {
                arena
                    .get(*id)
                    .ok_or_else(|| io::Error::other("validated item is missing"))
                    .and_then(|value| validated_value(arena, value))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        other => Err(io::Error::other(format!(
            "unsupported differential value: {other:?}"
        ))),
    }
}

fn outcome(
    name: &'static str,
    result: Result<ValidatedArena, ValidationError>,
) -> Result<Value, io::Error> {
    match result {
        Ok(arena) => {
            let root = arena
                .get(arena.root())
                .ok_or_else(|| io::Error::other("validated root is missing"))?;
            Ok(json!({"name": name, "outcome": {"ok": validated_value(&arena, root)?}}))
        }
        Err(error) => {
            let first = error
                .details()
                .first()
                .ok_or_else(|| io::Error::other("validation error has no details"))?;
            let location = first
                .location
                .iter()
                .map(location_value)
                .collect::<Vec<_>>();
            Ok(json!({
                "name": name,
                "outcome": {"error": {"code": first.code, "location": location}}
            }))
        }
    }
}

fn validate_json(
    schema: &Schema,
    payload: &[u8],
    strict: bool,
) -> Result<ValidatedArena, ValidationError> {
    let input = parse_json(payload, JsonLimits::default()).map_err(|error| {
        ValidationError::one(ErrorDetail {
            code: "json_invalid",
            location: Vec::new(),
            message: error.to_string(),
            expected: "valid JSON".to_owned(),
            context: Default::default(),
        })
    })?;
    validate(
        schema,
        &input,
        ValidationOptions {
            strict,
            profile: InputProfile::Json,
            ..ValidationOptions::default()
        },
    )
}

fn main() -> Result<(), Box<dyn Error>> {
    let string_pattern = StringPattern::compile("^[a-z]{3}$")?;
    let string_schema = Schema::String(StringConstraints {
        strip_whitespace: true,
        to_upper: true,
        min_length: Some(3),
        max_length: Some(3),
        pattern: Some(string_pattern),
        ..StringConstraints::default()
    });
    let list_schema = Schema::List {
        item: Box::new(Schema::exact_integer()),
        constraints: CollectionConstraints::default(),
    };
    let results = vec![
        outcome(
            "lax_int",
            validate_json(&Schema::exact_integer(), b"\"42\"", false),
        )?,
        outcome(
            "strict_int_error",
            validate_json(&Schema::exact_integer(), b"\"42\"", true),
        )?,
        outcome(
            "string_pipeline",
            validate_json(&string_schema, b"\"  abc  \"", false),
        )?,
        outcome("list_int", validate_json(&list_schema, b"[1,\"2\"]", false))?,
        outcome(
            "list_error",
            validate_json(&list_schema, b"[1,\"x\"]", false),
        )?,
    ];
    let mut output = io::stdout().lock();
    serde_json::to_writer(&mut output, &results)?;
    output.write_all(b"\n")?;
    Ok(())
}
