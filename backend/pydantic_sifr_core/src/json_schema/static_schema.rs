use serde_json::{Map, Value, json};
use sifr_runtime::json::JsonIntegerProfile;

use crate::validation::{AliasSegment, ExtraRef, SchemaRef, SchemaTag, static_declared_values};

use super::{
    JSON_SCHEMA_DIALECT, JsonSchemaError, JsonSchemaErrorKind, JsonSchemaMode, JsonSchemaOptions,
    MAX_JSON_SCHEMA_DEPTH, escape_json_pointer, integer_bounds, integer_schema, literal_schema,
    typed, unsupported,
};

const MAX_STATIC_JSON_SCHEMA_NODES: usize = 4096;

pub(super) fn generate(
    schema: SchemaRef<'_>,
    options: JsonSchemaOptions,
    integer_profile: JsonIntegerProfile,
) -> Result<Value, JsonSchemaError> {
    let mut generated_nodes = 0;
    let mut definitions = Vec::new();
    let mut document = generate_node(
        schema,
        options,
        integer_profile,
        0,
        &mut generated_nodes,
        &mut definitions,
    )?;
    let Some(document) = document.as_object_mut() else {
        return Err(unsupported("JSON Schema document root is not an object"));
    };
    let mut generated_definitions = Map::new();
    let mut definition_index = 0;
    while definition_index < definitions.len() {
        let (name, definition) = definitions[definition_index].clone();
        let value = generate_node(
            definition,
            options,
            integer_profile,
            1,
            &mut generated_nodes,
            &mut definitions,
        )?;
        generated_definitions.insert(name, value);
        definition_index += 1;
    }
    if !generated_definitions.is_empty() {
        document.insert("$defs".to_owned(), Value::Object(generated_definitions));
    }
    document.insert(
        "$schema".to_owned(),
        Value::String(JSON_SCHEMA_DIALECT.to_owned()),
    );
    Ok(Value::Object(core::mem::take(document)))
}

fn generate_node<'schema>(
    schema: SchemaRef<'schema>,
    options: JsonSchemaOptions,
    integer_profile: JsonIntegerProfile,
    depth: usize,
    generated_nodes: &mut usize,
    definitions: &mut Vec<(String, SchemaRef<'schema>)>,
) -> Result<Value, JsonSchemaError> {
    if depth > MAX_JSON_SCHEMA_DEPTH {
        return Err(JsonSchemaError::new(
            JsonSchemaErrorKind::DepthLimit,
            "JSON Schema generation exceeded the static schema depth limit",
        ));
    }
    *generated_nodes = generated_nodes.saturating_add(1);
    if *generated_nodes > MAX_STATIC_JSON_SCHEMA_NODES {
        return Err(JsonSchemaError::new(
            JsonSchemaErrorKind::DepthLimit,
            "JSON Schema generation exceeded the static node limit",
        ));
    }
    let mut child = |index| {
        let child = schema.child(index).map_err(schema_error)?;
        generate_node(
            child,
            options,
            integer_profile,
            depth + 1,
            generated_nodes,
            definitions,
        )
    };
    let output = match schema.tag().map_err(schema_error)? {
        SchemaTag::None => Ok(json!({"type": "null"})),
        SchemaTag::Bool => Ok(json!({"type": "boolean"})),
        SchemaTag::Integer => {
            let (target, constraints) = schema.integer().map_err(schema_error)?;
            if constraints
                .multiple_of
                .as_ref()
                .is_some_and(|value| value <= &num_bigint::BigInt::from(0_u8))
            {
                return Err(JsonSchemaError::new(
                    JsonSchemaErrorKind::InvalidNumber,
                    "integer multipleOf must be greater than zero",
                ));
            }
            let (minimum, maximum) = integer_bounds(target, &constraints);
            integer_schema(
                options.mode,
                integer_profile,
                minimum,
                maximum,
                &constraints.multiple_of,
            )
        }
        SchemaTag::Float => {
            let constraints = schema.float().map_err(schema_error)?;
            if constraints.multiple_of.is_some_and(|value| value <= 0.0) {
                return Err(JsonSchemaError::new(
                    JsonSchemaErrorKind::InvalidNumber,
                    "float multipleOf must be greater than zero",
                ));
            }
            let mut output = typed("number");
            insert_float_constraint(&mut output, "exclusiveMinimum", constraints.greater_than);
            insert_float_constraint(&mut output, "minimum", constraints.greater_or_equal);
            insert_float_constraint(&mut output, "exclusiveMaximum", constraints.less_than);
            insert_float_constraint(&mut output, "maximum", constraints.less_or_equal);
            insert_float_constraint(&mut output, "multipleOf", constraints.multiple_of);
            Ok(Value::Object(output))
        }
        SchemaTag::String => {
            let constraints = schema.string().map_err(schema_error)?;
            let mut output = typed("string");
            if let Some(value) = constraints.min_length {
                output.insert("minLength".to_owned(), json!(value));
            }
            if let Some(value) = constraints.max_length {
                output.insert("maxLength".to_owned(), json!(value));
            }
            if let Some(pattern) = constraints.pattern {
                output.insert(
                    "pattern".to_owned(),
                    Value::String(pattern.source().to_owned()),
                );
            }
            Ok(Value::Object(output))
        }
        SchemaTag::Literal | SchemaTag::Enum => literal_schema(
            &static_declared_values(schema).map_err(schema_error)?,
            options.mode,
            integer_profile,
        ),
        SchemaTag::Nullable => Ok(json!({"anyOf": [child(0)?, {"type": "null"}]})),
        SchemaTag::Union => Ok(json!({
            "anyOf": children(
                schema,
                options,
                integer_profile,
                depth + 1,
                generated_nodes,
                definitions,
            )?
        })),
        SchemaTag::TaggedUnion => Ok(json!({
            "oneOf": children(
                schema,
                options,
                integer_profile,
                depth + 1,
                generated_nodes,
                definitions,
            )?
        })),
        SchemaTag::DefinitionRef => {
            let name = schema.static_reference().map_err(schema_error)?;
            if !definitions.iter().any(|(existing, _)| existing == name) {
                definitions.push((
                    name.to_owned(),
                    schema.static_definition_target().map_err(schema_error)?,
                ));
            }
            Ok(json!({
                "$ref": format!("#/$defs/{}", escape_json_pointer(name))
            }))
        }
        SchemaTag::Model => model_schema(
            schema,
            options,
            integer_profile,
            depth,
            generated_nodes,
            definitions,
        ),
        SchemaTag::List | SchemaTag::Set => {
            let mut output = typed("array");
            output.insert("items".to_owned(), child(0)?);
            let constraints = schema.collection().map_err(schema_error)?;
            if let Some(value) = constraints.min_length {
                output.insert("minItems".to_owned(), json!(value));
            }
            if let Some(value) = constraints.max_length {
                output.insert("maxItems".to_owned(), json!(value));
            }
            if schema.tag().map_err(schema_error)? == SchemaTag::Set {
                output.insert("uniqueItems".to_owned(), Value::Bool(true));
            }
            Ok(Value::Object(output))
        }
        SchemaTag::Tuple => {
            let items = children(
                schema,
                options,
                integer_profile,
                depth + 1,
                generated_nodes,
                definitions,
            )?;
            let count = items.len();
            Ok(json!({
                "type": "array",
                "prefixItems": items,
                "minItems": count,
                "maxItems": count
            }))
        }
        SchemaTag::Mapping => {
            if schema
                .child(0)
                .map_err(schema_error)?
                .tag()
                .map_err(schema_error)?
                != SchemaTag::String
            {
                return Err(unsupported(
                    "non-string JSON object keys need an exact property-name representation",
                ));
            }
            let constraints = schema.collection().map_err(schema_error)?;
            let mut output = typed("object");
            output.insert("propertyNames".to_owned(), child(0)?);
            output.insert("additionalProperties".to_owned(), child(1)?);
            if let Some(value) = constraints.min_length {
                output.insert("minProperties".to_owned(), json!(value));
            }
            if let Some(value) = constraints.max_length {
                output.insert("maxProperties".to_owned(), json!(value));
            }
            Ok(Value::Object(output))
        }
        SchemaTag::Bytes => {
            let constraints = schema.bytes().map_err(schema_error)?;
            let mut output = typed("string");
            if let Some(value) = constraints.min_length {
                output.insert("minLength".to_owned(), json!(value));
            }
            if let Some(value) = constraints.max_length {
                output.insert("maxLength".to_owned(), json!(value));
            }
            Ok(Value::Object(output))
        }
        SchemaTag::Url | SchemaTag::MultiHostUrl => Ok(json!({"type": "string", "format": "uri"})),
        SchemaTag::Pattern => Ok(json!({"type": "string", "format": "regex"})),
        SchemaTag::Decimal => Err(unsupported(
            "static schema kind has no exact JSON Schema representation",
        )),
        _ => Err(unsupported(
            "static schema kind is not supported by JSON Schema generation",
        )),
    }?;
    apply_metadata(schema, output)
}

fn insert_float_constraint(output: &mut Map<String, Value>, key: &str, value: Option<f64>) {
    if let Some(value) = value {
        output.insert(key.to_owned(), json!(value));
    }
}

fn apply_metadata(schema: SchemaRef<'_>, mut output: Value) -> Result<Value, JsonSchemaError> {
    if !matches!(schema, SchemaRef::Static(_)) {
        return Ok(output);
    }
    let Some(object) = output.as_object_mut() else {
        return Ok(output);
    };
    let mut examples = Vec::new();
    for item in schema.static_metadata().map_err(schema_error)? {
        match item.key {
            "pydantic.title" => {
                object.insert("title".to_owned(), Value::String(item.value.to_owned()));
            }
            "pydantic.description" => {
                object.insert(
                    "description".to_owned(),
                    Value::String(item.value.to_owned()),
                );
            }
            "pydantic.example" => examples.push(Value::String(item.value.to_owned())),
            key if key.starts_with("pydantic.json_schema_extra.") => {
                let name = key.trim_start_matches("pydantic.json_schema_extra.");
                if name.is_empty() || reserved_extra_key(name) {
                    return Err(unsupported(
                        "json_schema_extra cannot replace structural JSON Schema keys",
                    ));
                }
                let value = match item.value {
                    "true" => Value::Bool(true),
                    "false" => Value::Bool(false),
                    value => Value::String(value.to_owned()),
                };
                object.insert(name.to_owned(), value);
            }
            _ => {}
        }
    }
    if !examples.is_empty() {
        object.insert("examples".to_owned(), Value::Array(examples));
    }
    Ok(output)
}

fn reserved_extra_key(key: &str) -> bool {
    matches!(
        key,
        "$schema"
            | "$id"
            | "$anchor"
            | "$dynamicAnchor"
            | "$dynamicRef"
            | "$defs"
            | "$ref"
            | "type"
            | "const"
            | "enum"
            | "not"
            | "if"
            | "then"
            | "else"
            | "format"
            | "pattern"
            | "minimum"
            | "maximum"
            | "exclusiveMinimum"
            | "exclusiveMaximum"
            | "multipleOf"
            | "minLength"
            | "maxLength"
            | "minItems"
            | "maxItems"
            | "uniqueItems"
            | "prefixItems"
            | "items"
            | "contains"
            | "minContains"
            | "maxContains"
            | "properties"
            | "patternProperties"
            | "propertyNames"
            | "minProperties"
            | "maxProperties"
            | "required"
            | "additionalProperties"
            | "dependentSchemas"
            | "unevaluatedProperties"
            | "unevaluatedItems"
            | "anyOf"
            | "oneOf"
            | "allOf"
            | "title"
            | "description"
            | "default"
            | "examples"
    )
}

fn children<'schema>(
    schema: SchemaRef<'schema>,
    options: JsonSchemaOptions,
    integer_profile: JsonIntegerProfile,
    depth: usize,
    generated_nodes: &mut usize,
    definitions: &mut Vec<(String, SchemaRef<'schema>)>,
) -> Result<Vec<Value>, JsonSchemaError> {
    (0..schema.child_count().map_err(schema_error)?)
        .map(|index| {
            generate_node(
                schema.child(index).map_err(schema_error)?,
                options,
                integer_profile,
                depth,
                generated_nodes,
                definitions,
            )
        })
        .collect()
}

fn model_schema<'schema>(
    schema: SchemaRef<'schema>,
    options: JsonSchemaOptions,
    integer_profile: JsonIntegerProfile,
    depth: usize,
    generated_nodes: &mut usize,
    definitions: &mut Vec<(String, SchemaRef<'schema>)>,
) -> Result<Value, JsonSchemaError> {
    let model = schema.model().map_err(schema_error)?;
    let mut properties = Map::new();
    let mut required = Vec::new();
    for field in model.fields().map_err(schema_error)? {
        if options.mode == JsonSchemaMode::Validation && !field.input() {
            continue;
        }
        if options.mode == JsonSchemaMode::Serialization
            && field.excluded().map_err(schema_error)?
        {
            continue;
        }
        let name = field_name(model, field, options)?;
        if properties.contains_key(&name) {
            return Err(unsupported(
                "model aliases produce duplicate JSON Schema property names",
            ));
        }
        properties.insert(
            name.clone(),
            generate_node(
                field.schema().map_err(schema_error)?,
                options,
                integer_profile,
                depth + 1,
                generated_nodes,
                definitions,
            )?,
        );
        if options.mode == JsonSchemaMode::Serialization
            || field.is_required().map_err(schema_error)?
        {
            required.push(Value::String(name));
        }
    }
    let additional = match (options.mode, model.extra().map_err(schema_error)?) {
        (JsonSchemaMode::Serialization, _) | (_, ExtraRef::Forbid) => Value::Bool(false),
        (_, ExtraRef::Ignore) => Value::Bool(true),
        (_, ExtraRef::Allow { .. }) => {
            return Err(unsupported(
                "static models do not support typed extra fields",
            ));
        }
    };
    let mut output = typed("object");
    output.insert(
        "title".to_owned(),
        Value::String(model.name().map_err(schema_error)?.to_owned()),
    );
    output.insert("properties".to_owned(), Value::Object(properties));
    output.insert("required".to_owned(), Value::Array(required));
    output.insert("additionalProperties".to_owned(), additional);
    Ok(Value::Object(output))
}

fn field_name(
    model: crate::validation::ModelRef<'_>,
    field: crate::validation::FieldRef<'_>,
    options: JsonSchemaOptions,
) -> Result<String, JsonSchemaError> {
    let name = field.name().map_err(schema_error)?;
    if options.mode == JsonSchemaMode::Serialization {
        if options.by_alias {
            return Ok(field
                .serialization_alias()
                .unwrap_or_else(|| name.to_owned()));
        }
        return Ok(name.to_owned());
    }
    match field.aliases().map_err(schema_error)?.as_slice() {
        [] => Ok(name.to_owned()),
        [path]
            if !model.populate_by_name().map_err(schema_error)?
                && matches!(path.segments.as_slice(), [AliasSegment::Field(_)]) =>
        {
            let [AliasSegment::Field(alias)] = path.segments.as_slice() else {
                return Err(unsupported("validation alias path is not a field"));
            };
            if alias.is_empty() {
                Err(unsupported("validation alias must not be empty"))
            } else {
                Ok((*alias).to_owned())
            }
        }
        _ => Err(unsupported(
            "validation aliases need one field alias with populate_by_name disabled",
        )),
    }
}

fn schema_error(error: crate::ValidationError) -> JsonSchemaError {
    JsonSchemaError::new(JsonSchemaErrorKind::UnsupportedSchema, error.to_string())
}

#[cfg(test)]
mod tests {
    use sifr_runtime::interop::structural::StaticProgramValue;

    use super::*;

    fn record(fields: Vec<(&'static str, StaticProgramValue)>) -> StaticProgramValue {
        StaticProgramValue::Record(Box::leak(fields.into_boxed_slice()))
    }

    fn list(values: Vec<StaticProgramValue>) -> StaticProgramValue {
        StaticProgramValue::List(Box::leak(values.into_boxed_slice()))
    }

    fn field(node: &'static str, required: bool) -> StaticProgramValue {
        record(vec![
            ("name", StaticProgramValue::String("next")),
            ("node", StaticProgramValue::Integer(node)),
            ("required", StaticProgramValue::Bool(required)),
            (
                "default_kind",
                StaticProgramValue::String(if required { "required" } else { "const" }),
            ),
            ("default", StaticProgramValue::None),
            ("validation_aliases", list(Vec::new())),
            ("serialization_alias", StaticProgramValue::None),
            ("strict", StaticProgramValue::None),
            ("exclude", StaticProgramValue::Bool(false)),
        ])
    }

    fn model_node(
        definition: &'static str,
        child: &'static str,
        required: bool,
    ) -> StaticProgramValue {
        record(vec![
            ("kind", StaticProgramValue::String("model")),
            ("children", list(vec![StaticProgramValue::Integer(child)])),
            ("definition", StaticProgramValue::String(definition)),
            ("reference", StaticProgramValue::None),
            (
                "model",
                record(vec![
                    ("name", StaticProgramValue::String(definition)),
                    ("fields", list(vec![field(child, required)])),
                    ("extra", StaticProgramValue::String("forbid")),
                    ("populate_by_name", StaticProgramValue::Bool(false)),
                    ("location_by_alias", StaticProgramValue::Bool(true)),
                    ("strict", StaticProgramValue::Bool(false)),
                ]),
            ),
            ("metadata", list(Vec::new())),
        ])
    }

    fn schema_program(nodes: Vec<StaticProgramValue>) -> &'static StaticProgramValue {
        Box::leak(Box::new(record(vec![
            ("nodes", list(nodes)),
            ("root", StaticProgramValue::Integer("0")),
        ])))
    }

    #[test]
    fn unreferenced_root_definition_is_not_duplicated_in_defs() {
        let scalar = record(vec![
            ("kind", StaticProgramValue::String("str")),
            ("children", list(Vec::new())),
            ("definition", StaticProgramValue::None),
            ("reference", StaticProgramValue::None),
            ("string_constraints", StaticProgramValue::None),
            ("metadata", list(Vec::new())),
        ]);
        let unreachable_reference = record(vec![
            ("kind", StaticProgramValue::String("definition-ref")),
            ("children", list(Vec::new())),
            ("definition", StaticProgramValue::None),
            ("reference", StaticProgramValue::String("Orphan")),
            ("metadata", list(Vec::new())),
        ]);
        let schema = SchemaRef::from_static_program(schema_program(vec![
            model_node("User", "1", true),
            scalar,
            unreachable_reference,
            model_node("Orphan", "1", true),
        ]))
        .unwrap_or_else(|error| panic!("static schema failed: {error}"));
        let document = generate(
            schema,
            JsonSchemaOptions::new(JsonSchemaMode::Validation, true),
            JsonIntegerProfile::Exact,
        )
        .unwrap_or_else(|error| panic!("JSON Schema failed: {error}"));
        assert!(document.get("$defs").is_none());
        assert_eq!(document["properties"]["next"]["type"], "string");
    }

    #[test]
    fn recursive_reference_emits_one_reachable_definition() {
        let nullable = record(vec![
            ("kind", StaticProgramValue::String("nullable")),
            ("children", list(vec![StaticProgramValue::Integer("2")])),
            ("definition", StaticProgramValue::None),
            ("reference", StaticProgramValue::None),
            ("metadata", list(Vec::new())),
        ]);
        let reference = record(vec![
            ("kind", StaticProgramValue::String("definition-ref")),
            ("children", list(Vec::new())),
            ("definition", StaticProgramValue::None),
            ("reference", StaticProgramValue::String("Node")),
            ("metadata", list(Vec::new())),
        ]);
        let schema = SchemaRef::from_static_program(schema_program(vec![
            model_node("Node", "1", true),
            nullable,
            reference,
        ]))
        .unwrap_or_else(|error| panic!("static schema failed: {error}"));
        let document = generate(
            schema,
            JsonSchemaOptions::new(JsonSchemaMode::Validation, true),
            JsonIntegerProfile::Exact,
        )
        .unwrap_or_else(|error| panic!("JSON Schema failed: {error}"));
        let definitions = document["$defs"]
            .as_object()
            .unwrap_or_else(|| panic!("recursive schema has no definitions"));
        assert_eq!(definitions.len(), 1);
        assert_eq!(
            definitions["Node"]["properties"]["next"]["anyOf"][0]["$ref"],
            "#/$defs/Node"
        );
    }

    #[test]
    fn none_default_uses_required_flag_in_static_schema() {
        let nullable = record(vec![
            ("kind", StaticProgramValue::String("nullable")),
            ("children", list(vec![StaticProgramValue::Integer("2")])),
            ("definition", StaticProgramValue::None),
            ("reference", StaticProgramValue::None),
            ("metadata", list(Vec::new())),
        ]);
        let scalar = record(vec![
            ("kind", StaticProgramValue::String("str")),
            ("children", list(Vec::new())),
            ("definition", StaticProgramValue::None),
            ("reference", StaticProgramValue::None),
            ("string_constraints", StaticProgramValue::None),
            ("metadata", list(Vec::new())),
        ]);
        let schema = SchemaRef::from_static_program(schema_program(vec![
            model_node("OptionalModel", "1", false),
            nullable,
            scalar,
        ]))
        .unwrap_or_else(|error| panic!("static schema failed: {error}"));
        let document = generate(
            schema,
            JsonSchemaOptions::new(JsonSchemaMode::Validation, true),
            JsonIntegerProfile::Exact,
        )
        .unwrap_or_else(|error| panic!("JSON Schema failed: {error}"));
        assert_eq!(document["required"], json!([]));
    }

    #[test]
    fn schema_extras_cannot_replace_structural_keywords() {
        for key in ["items", "pattern", "enum", "minimum", "properties"] {
            assert!(super::reserved_extra_key(key));
        }
        assert!(!super::reserved_extra_key("x-package-note"));
    }
}
