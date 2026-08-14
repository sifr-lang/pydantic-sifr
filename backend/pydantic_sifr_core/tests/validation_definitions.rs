use pydantic_sifr_core::{
    DefinitionSchema, DefinitionsSchema, ExtraPolicy, JsonLimits, ModelField, ModelSchema,
    PreparedSchema, Schema, StringConstraints, ValidatedArena, ValidatedValue, ValidationLimits,
    ValidationOptions, parse_json, validate,
};
use sifr_runtime::interop::structural::{ShapeIdentity, primitive};

fn required(name: &'static str, schema: Schema) -> ModelField {
    ModelField::required(name, schema)
}

fn model(name: &'static str, identity: ShapeIdentity, fields: Vec<ModelField>) -> Schema {
    Schema::Model(
        ModelSchema::new(name, identity, fields, ExtraPolicy::Ignore, false, true)
            .unwrap_or_else(|error| panic!("model schema failed: {error}")),
    )
}

fn node_reference(identity: ShapeIdentity) -> Schema {
    Schema::model_reference("tests.Node", identity, "tests.Node")
}

fn node_definition(identity: ShapeIdentity) -> DefinitionSchema {
    DefinitionSchema {
        name: "tests.Node",
        schema: model(
            "tests.Node",
            identity,
            vec![
                required("value", Schema::exact_integer()),
                required("next", Schema::Nullable(Box::new(node_reference(identity)))),
            ],
        ),
    }
}

fn recursive_node_schema() -> Schema {
    let identity = primitive("tests.Node");
    Schema::Definitions(
        DefinitionsSchema::new(node_reference(identity), vec![node_definition(identity)])
            .unwrap_or_else(|error| panic!("definition schema failed: {error}")),
    )
}

fn json(
    schema: &Schema,
    input: &[u8],
) -> Result<ValidatedArena, pydantic_sifr_core::ValidationError> {
    let input = parse_json(input, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON input failed: {error}"));
    validate(
        schema,
        &input,
        ValidationOptions {
            profile: pydantic_sifr_core::InputProfile::Json,
            ..ValidationOptions::default()
        },
    )
}

fn require_validation_error(
    result: Result<ValidatedArena, pydantic_sifr_core::ValidationError>,
) -> pydantic_sifr_core::ValidationError {
    match result {
        Ok(_) => panic!("expected validation error"),
        Err(error) => error,
    }
}

fn require_schema_error(
    result: Result<DefinitionsSchema, pydantic_sifr_core::ValidationError>,
) -> pydantic_sifr_core::ValidationError {
    match result {
        Ok(_) => panic!("expected schema error"),
        Err(error) => error,
    }
}

#[test]
fn recursive_definition_validates_finite_nullable_branches() {
    let arena = json(
        &recursive_node_schema(),
        br#"{"value":1,"next":{"value":2,"next":null}}"#,
    )
    .unwrap_or_else(|error| panic!("recursive validation failed: {error}"));
    let Some(ValidatedValue::Model(root)) = arena.get(arena.root()) else {
        panic!("expected root model");
    };
    assert_eq!(root.name(), "tests.Node");
    let next = root.fields()[1].1;
    let Some(ValidatedValue::Nullable(Some(child))) = arena.get(next) else {
        panic!("expected populated recursive field");
    };
    let Some(ValidatedValue::Model(child)) = arena.get(*child) else {
        panic!("expected child model");
    };
    assert_eq!(child.name(), "tests.Node");
    assert!(matches!(
        arena.get(child.fields()[1].1),
        Some(ValidatedValue::Nullable(None))
    ));
}

#[test]
fn repeated_references_share_one_definition_scope() {
    let node_identity = primitive("tests.Node");
    let pair = model(
        "tests.Pair",
        primitive("tests.Pair"),
        vec![
            required("left", node_reference(node_identity)),
            required("right", node_reference(node_identity)),
        ],
    );
    let schema = Schema::Definitions(
        DefinitionsSchema::new(pair, vec![node_definition(node_identity)])
            .unwrap_or_else(|error| panic!("definition schema failed: {error}")),
    );
    let arena = json(
        &schema,
        br#"{"left":{"value":1,"next":null},"right":{"value":2,"next":null}}"#,
    )
    .unwrap_or_else(|error| panic!("repeated reference validation failed: {error}"));
    let Some(ValidatedValue::Model(pair)) = arena.get(arena.root()) else {
        panic!("expected pair model");
    };
    for (_, field) in pair.fields() {
        assert!(matches!(arena.get(*field), Some(ValidatedValue::Model(_))));
    }
}

#[test]
fn recursive_input_stops_at_the_configured_depth() {
    let input = br#"{"value":0,"next":{"value":1,"next":{"value":2,"next":null}}}"#;
    let input = parse_json(input, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON input failed: {error}"));
    let error = require_validation_error(validate(
        &recursive_node_schema(),
        &input,
        ValidationOptions {
            profile: pydantic_sifr_core::InputProfile::Json,
            limits: ValidationLimits {
                max_depth: 2,
                ..ValidationLimits::default()
            },
            ..ValidationOptions::default()
        },
    ));
    assert_eq!(error.details()[0].code, "recursion_limit");
}

#[test]
fn definition_reference_identity_must_match_its_target() {
    let declared = primitive("tests.Node");
    let wrong = primitive("tests.Other");
    let schema = require_schema_error(DefinitionsSchema::new(
        Schema::model_reference("tests.Node", wrong, "tests.Node"),
        vec![node_definition(declared)],
    ));
    assert_eq!(schema.details()[0].code, "schema_invalid");
}

#[test]
fn prepared_recursive_schema_uses_the_definition_root_identity() {
    let schema = recursive_node_schema();
    let prepared = PreparedSchema::new(&schema)
        .unwrap_or_else(|error| panic!("schema preparation failed: {error}"));
    assert_eq!(prepared.structural_identity(), primitive("tests.Node"));
}

#[test]
fn definitions_reject_duplicate_and_dangling_names() {
    let identity = primitive("tests.Node");
    let duplicate = require_schema_error(DefinitionsSchema::new(
        node_reference(identity),
        vec![node_definition(identity), node_definition(identity)],
    ));
    assert_eq!(duplicate.details()[0].code, "schema_invalid");

    let dangling = require_schema_error(DefinitionsSchema::new(
        Schema::model_reference("tests.Missing", identity, "tests.Node"),
        vec![node_definition(identity)],
    ));
    assert_eq!(dangling.details()[0].code, "schema_invalid");
}

#[test]
fn definitions_can_reuse_non_model_targets() {
    let integer = Schema::exact_integer();
    let reference = Schema::definition_reference("shared.integer", &integer)
        .unwrap_or_else(|error| panic!("reference schema failed: {error}"));
    let schema = Schema::Definitions(
        DefinitionsSchema::new(
            Schema::Tuple(vec![reference.clone(), reference]),
            vec![DefinitionSchema {
                name: "shared.integer",
                schema: integer,
            }],
        )
        .unwrap_or_else(|error| panic!("definition schema failed: {error}")),
    );
    let arena = json(&schema, b"[1,2]")
        .unwrap_or_else(|error| panic!("shared scalar validation failed: {error}"));
    assert!(matches!(
        arena.get(arena.root()),
        Some(ValidatedValue::Tuple(values)) if values.len() == 2
    ));
}

#[test]
fn recursive_string_field_errors_keep_the_complete_path() {
    let error = require_validation_error(json(
        &recursive_node_schema(),
        br#"{"value":1,"next":{"value":"bad","next":null}}"#,
    ));
    assert_eq!(error.details()[0].code, "int_parsing");
    assert_eq!(error.details()[0].location.len(), 2);
}

#[test]
fn recursive_definition_can_mix_concrete_fields() {
    let identity = primitive("tests.NamedNode");
    let schema = Schema::Definitions(
        DefinitionsSchema::new(
            model(
                "tests.NamedNode",
                identity,
                vec![
                    required("name", Schema::String(StringConstraints::default())),
                    required(
                        "next",
                        Schema::Nullable(Box::new(Schema::model_reference(
                            "tests.NamedNode",
                            identity,
                            "tests.NamedNode",
                        ))),
                    ),
                ],
            ),
            vec![DefinitionSchema {
                name: "tests.NamedNode",
                schema: model(
                    "tests.NamedNode",
                    identity,
                    vec![
                        required("name", Schema::String(StringConstraints::default())),
                        required(
                            "next",
                            Schema::Nullable(Box::new(Schema::model_reference(
                                "tests.NamedNode",
                                identity,
                                "tests.NamedNode",
                            ))),
                        ),
                    ],
                ),
            }],
        )
        .unwrap_or_else(|error| panic!("definition schema failed: {error}")),
    );
    let arena = json(&schema, br#"{"name":"root","next":null}"#)
        .unwrap_or_else(|error| panic!("mixed recursive validation failed: {error}"));
    assert!(matches!(
        arena.get(arena.root()),
        Some(ValidatedValue::Model(_))
    ));
}
