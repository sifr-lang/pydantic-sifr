use std::collections::BTreeMap;

use pydantic_sifr_core::validation::SchemaRef;
use pydantic_sifr_core::{
    CollectionConstraints, DefinitionSchema, DefinitionsSchema, ExtraPolicy, FieldDefault,
    JsonLimits, ModelField, ModelSchema, NativeValue, PreparedSchema, Schema, StringConstraints,
    UnionChoice, UnionMode, UnionSchema, ValidatedArena, ValidatedValue, ValidationLimits,
    ValidationOptions, parse_json, validate, validated_iterator,
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

#[test]
fn embedded_json_reference_keeps_its_definition_scope() {
    let integer = Schema::exact_integer();
    let reference = Schema::definition_reference("shared.integer", &integer)
        .unwrap_or_else(|error| panic!("reference schema failed: {error}"));
    let schema = Schema::Definitions(
        DefinitionsSchema::new(
            Schema::EmbeddedJson(Box::new(reference)),
            vec![DefinitionSchema {
                name: "shared.integer",
                schema: integer,
            }],
        )
        .unwrap_or_else(|error| panic!("definition schema failed: {error}")),
    );
    let arena = json(&schema, br#""42""#)
        .unwrap_or_else(|error| panic!("embedded reference validation failed: {error}"));
    assert!(matches!(
        arena.get(arena.root()),
        Some(ValidatedValue::ExactInt(value)) if value.to_string() == "42"
    ));
}

#[test]
fn default_reference_keeps_its_definition_scope() {
    let text = Schema::String(StringConstraints::default());
    let reference = Schema::definition_reference("shared.text", &text)
        .unwrap_or_else(|error| panic!("reference schema failed: {error}"));
    let field = ModelField {
        name: "label",
        schema: reference,
        input: true,
        default: Some(FieldDefault::Static(NativeValue::String(
            "ready".to_owned(),
        ))),
        validation_aliases: Vec::new(),
        metadata: BTreeMap::new(),
    };
    let root = model("tests.Defaulted", primitive("tests.Defaulted"), vec![field]);
    let schema = Schema::Definitions(
        DefinitionsSchema::new(
            root,
            vec![DefinitionSchema {
                name: "shared.text",
                schema: text,
            }],
        )
        .unwrap_or_else(|error| panic!("definition schema failed: {error}")),
    );
    let arena = json(&schema, b"{}")
        .unwrap_or_else(|error| panic!("default reference validation failed: {error}"));
    let Some(ValidatedValue::Model(value)) = arena.get(arena.root()) else {
        panic!("expected model output");
    };
    assert!(matches!(
        arena.get(value.fields()[0].1),
        Some(ValidatedValue::String(value)) if value == "ready"
    ));
}

#[test]
fn json_mapping_key_reference_keeps_its_definition_scope() {
    let integer = Schema::exact_integer();
    let reference = Schema::definition_reference("shared.integer", &integer)
        .unwrap_or_else(|error| panic!("reference schema failed: {error}"));
    let schema = Schema::Definitions(
        DefinitionsSchema::new(
            Schema::Mapping {
                key: Box::new(reference),
                value: Box::new(Schema::Bool),
                constraints: CollectionConstraints::default(),
            },
            vec![DefinitionSchema {
                name: "shared.integer",
                schema: integer,
            }],
        )
        .unwrap_or_else(|error| panic!("definition schema failed: {error}")),
    );
    let arena = json(&schema, br#"{"7":true}"#)
        .unwrap_or_else(|error| panic!("mapping reference validation failed: {error}"));
    assert!(matches!(
        arena.get(arena.root()),
        Some(ValidatedValue::Mapping(entries)) if entries.len() == 1
    ));
}

#[test]
fn lazy_generator_reference_keeps_its_definition_scope() {
    let integer = Schema::exact_integer();
    let reference = Schema::definition_reference("shared.integer", &integer)
        .unwrap_or_else(|error| panic!("reference schema failed: {error}"));
    let schema = Schema::Definitions(
        DefinitionsSchema::new(
            Schema::Generator {
                item: Box::new(reference),
                constraints: CollectionConstraints::default(),
            },
            vec![DefinitionSchema {
                name: "shared.integer",
                schema: integer,
            }],
        )
        .unwrap_or_else(|error| panic!("definition schema failed: {error}")),
    );
    let input = parse_json(b"[1,2]", JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON input failed: {error}"));
    let mut iterator = validated_iterator(
        &schema,
        &input,
        ValidationOptions {
            profile: pydantic_sifr_core::InputProfile::Json,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("generator setup failed: {error}"));
    for expected in ["1", "2"] {
        let arena = match iterator.next() {
            Some(Ok(arena)) => arena,
            Some(Err(error)) => panic!("generator item failed: {error}"),
            None => panic!("generator ended early"),
        };
        assert!(matches!(
            arena.get(arena.root()),
            Some(ValidatedValue::ExactInt(value)) if value.to_string() == expected
        ));
    }
    assert!(iterator.next().is_none());
}

#[test]
fn unreachable_definition_references_are_checked() {
    let error = require_schema_error(DefinitionsSchema::new(
        Schema::Bool,
        vec![DefinitionSchema {
            name: "tests.Dead",
            schema: Schema::model_reference(
                "tests.Missing",
                primitive("tests.Missing"),
                "tests.Missing",
            ),
        }],
    ));
    assert_eq!(error.details()[0].code, "schema_invalid");
}

#[test]
fn definition_references_reject_sum_targets() {
    let sum = Schema::Union(
        UnionSchema::new(
            vec![
                UnionChoice {
                    label: "integer",
                    schema: Schema::exact_integer(),
                },
                UnionChoice {
                    label: "text",
                    schema: Schema::String(StringConstraints::default()),
                },
            ],
            UnionMode::Smart,
            false,
            None,
        )
        .unwrap_or_else(|error| panic!("union schema failed: {error}")),
    );
    let error = match Schema::definition_reference("shared.sum", &sum) {
        Ok(_) => panic!("expected sum reference rejection"),
        Err(error) => error,
    };
    assert_eq!(error.details()[0].code, "schema_invalid");

    let embedded = Schema::EmbeddedJson(Box::new(Schema::exact_integer()));
    let error = match Schema::definition_reference("shared.embedded", &embedded) {
        Ok(_) => panic!("expected embedded JSON reference rejection"),
        Err(error) => error,
    };
    assert_eq!(error.details()[0].code, "schema_invalid");
}

#[test]
fn definitions_wrapper_flattens_the_root_union_layout() {
    let inner = Schema::Union(
        UnionSchema::new(
            vec![
                UnionChoice {
                    label: "text",
                    schema: Schema::String(StringConstraints::default()),
                },
                UnionChoice {
                    label: "integer",
                    schema: Schema::exact_integer(),
                },
            ],
            UnionMode::Smart,
            false,
            None,
        )
        .unwrap_or_else(|error| panic!("inner union schema failed: {error}")),
    );
    let definitions = Schema::Definitions(
        DefinitionsSchema::new(inner, Vec::new())
            .unwrap_or_else(|error| panic!("definition schema failed: {error}")),
    );
    let schema = Schema::Union(
        UnionSchema::new(
            vec![
                UnionChoice {
                    label: "defined",
                    schema: definitions,
                },
                UnionChoice {
                    label: "boolean",
                    schema: Schema::Bool,
                },
            ],
            UnionMode::Smart,
            false,
            None,
        )
        .unwrap_or_else(|error| panic!("outer union schema failed: {error}")),
    );
    let arena = json(&schema, br#""ready""#)
        .unwrap_or_else(|error| panic!("defined union validation failed: {error}"));
    let Some(ValidatedValue::Union(value)) = arena.get(arena.root()) else {
        panic!("expected flattened union output");
    };
    assert_eq!(value.index(), 2);
}

#[test]
fn smart_union_ranks_a_reference_by_its_target_exactness() {
    let text = Schema::String(StringConstraints::default());
    let reference = Schema::definition_reference("shared.text", &text)
        .unwrap_or_else(|error| panic!("reference schema failed: {error}"));
    let union = Schema::Union(
        UnionSchema::new(
            vec![
                UnionChoice {
                    label: "integer",
                    schema: Schema::exact_integer(),
                },
                UnionChoice {
                    label: "text",
                    schema: reference,
                },
            ],
            UnionMode::Smart,
            false,
            None,
        )
        .unwrap_or_else(|error| panic!("union schema failed: {error}")),
    );
    let schema = Schema::Definitions(
        DefinitionsSchema::new(
            union,
            vec![DefinitionSchema {
                name: "shared.text",
                schema: text,
            }],
        )
        .unwrap_or_else(|error| panic!("definition schema failed: {error}")),
    );
    let arena = json(&schema, br#""1""#)
        .unwrap_or_else(|error| panic!("reference union validation failed: {error}"));
    let Some(ValidatedValue::Union(value)) = arena.get(arena.root()) else {
        panic!("expected union output");
    };
    assert_eq!(value.index(), 1);
    assert!(matches!(
        arena.get(value.value()),
        Some(ValidatedValue::String(value)) if value == "1"
    ));
}

#[test]
fn smart_union_ranks_a_mapping_with_a_referenced_string_key() {
    let text = Schema::String(StringConstraints::default());
    let reference = Schema::definition_reference("shared.text", &text)
        .unwrap_or_else(|error| panic!("reference schema failed: {error}"));
    let mapping = |key| Schema::Mapping {
        key: Box::new(key),
        value: Box::new(Schema::exact_integer()),
        constraints: CollectionConstraints::default(),
    };
    let union = Schema::Union(
        UnionSchema::new(
            vec![
                UnionChoice {
                    label: "integer key",
                    schema: mapping(Schema::exact_integer()),
                },
                UnionChoice {
                    label: "text key",
                    schema: mapping(reference),
                },
            ],
            UnionMode::Smart,
            false,
            None,
        )
        .unwrap_or_else(|error| panic!("union schema failed: {error}")),
    );
    let schema = Schema::Definitions(
        DefinitionsSchema::new(
            union,
            vec![DefinitionSchema {
                name: "shared.text",
                schema: text,
            }],
        )
        .unwrap_or_else(|error| panic!("definition schema failed: {error}")),
    );
    let arena = json(&schema, br#"{"1":2}"#)
        .unwrap_or_else(|error| panic!("mapping union validation failed: {error}"));
    let Some(ValidatedValue::Union(value)) = arena.get(arena.root()) else {
        panic!("expected union output");
    };
    let Some(ValidatedValue::Mapping(entries)) = arena.get(value.value()) else {
        panic!("expected mapping member");
    };
    assert!(matches!(
        arena.get(entries[0].0),
        Some(ValidatedValue::String(value)) if value == "1"
    ));
}

#[test]
fn fresh_input_resets_the_arena_local_recursion_trace() {
    let identity = primitive("tests.FreshNode");
    let reference = Schema::model_reference("tests.FreshNode", identity, "tests.FreshNode");
    let target = model(
        "tests.FreshNode",
        identity,
        vec![required(
            "raw",
            Schema::EmbeddedJson(Box::new(reference.clone())),
        )],
    );
    let schema = Schema::Definitions(
        DefinitionsSchema::new(
            reference,
            vec![DefinitionSchema {
                name: "tests.FreshNode",
                schema: target,
            }],
        )
        .unwrap_or_else(|error| panic!("definition schema failed: {error}")),
    );
    let error = require_validation_error(json(&schema, br#"{"raw":"{\"raw\":null}"}"#));
    assert_eq!(error.details()[0].code, "json_type");
    assert_ne!(error.details()[0].code, "recursion_loop");
}

#[test]
fn definitions_schema_view_reports_its_root_child() {
    let schema = Schema::Definitions(
        DefinitionsSchema::new(Schema::Bool, Vec::new())
            .unwrap_or_else(|error| panic!("definition schema failed: {error}")),
    );
    let view = SchemaRef::owned(&schema);
    assert_eq!(
        view.child_count()
            .unwrap_or_else(|error| panic!("child count failed: {error}")),
        1
    );
    assert!(view.child(0).is_ok());
}
