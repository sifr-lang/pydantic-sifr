use num_rational::BigRational;
use pydantic_sifr_core::{
    ComplexConstraints, DefinitionSchema, DefinitionsSchema, ExtraPolicy, FractionConstraints,
    IntegerConstraints, IntegerTarget, JSON_SCHEMA_DIALECT, JsonIntegerProfile, JsonSchemaMode,
    JsonSchemaOptions, ModelField, ModelSchema, Schema, StringConstraints, generate_json_schema,
};
use serde_json::{Value, json};
use sifr_runtime::interop::structural::primitive;

#[test]
fn representative_documents_have_deterministic_snapshots() {
    insta::assert_json_snapshot!(
        "recursive_validation",
        document(&recursive_schema(), JsonSchemaMode::Validation, false)
    );
    insta::assert_json_snapshot!(
        "aliased_serialization",
        document(&aliased_schema(), JsonSchemaMode::Serialization, true)
    );
    insta::assert_json_snapshot!(
        "specialized_validation",
        document(&specialized_schema(), JsonSchemaMode::Validation, false)
    );
}

#[test]
fn repeated_generation_is_byte_deterministic() {
    let schema = recursive_schema();
    let expected = serde_json::to_vec(&document(&schema, JsonSchemaMode::Validation, false))
        .unwrap_or_else(|error| panic!("schema serialization failed: {error}"));
    for _ in 0..32 {
        let actual = serde_json::to_vec(&document(&schema, JsonSchemaMode::Validation, false))
            .unwrap_or_else(|error| panic!("schema serialization failed: {error}"));
        assert_eq!(actual, expected);
    }
}

#[test]
fn representative_documents_conform_to_draft_2020_12() {
    let recursive = document(&recursive_schema(), JsonSchemaMode::Validation, false);
    let aliased = document(&aliased_schema(), JsonSchemaMode::Serialization, true);
    let specialized = document(&specialized_schema(), JsonSchemaMode::Validation, false);

    for schema in [&recursive, &aliased, &specialized] {
        assert_eq!(schema["$schema"], JSON_SCHEMA_DIALECT);
        jsonschema::draft202012::meta::validate(schema)
            .unwrap_or_else(|error| panic!("Draft 2020-12 meta-schema rejected output: {error}"));
        jsonschema::draft202012::new(schema)
            .unwrap_or_else(|error| panic!("Draft 2020-12 compiler rejected output: {error}"));
    }

    let recursive_validator = jsonschema::draft202012::new(&recursive)
        .unwrap_or_else(|error| panic!("recursive schema failed to compile: {error}"));
    assert!(recursive_validator.is_valid(&json!({"next": null})));
    assert!(recursive_validator.is_valid(&json!({"next": {"next": null}})));
    assert!(!recursive_validator.is_valid(&json!({"next": null, "extra": true})));
    assert!(!recursive_validator.is_valid(&json!({})));

    let aliased_validator = jsonschema::draft202012::new(&aliased)
        .unwrap_or_else(|error| panic!("aliased schema failed to compile: {error}"));
    assert!(aliased_validator.is_valid(&json!({"public_id": 7})));
    assert!(!aliased_validator.is_valid(&json!({"identifier": 7})));
    assert!(!aliased_validator.is_valid(&json!({"public_id": "7"})));
}

fn document(schema: &Schema, mode: JsonSchemaMode, by_alias: bool) -> Value {
    generate_json_schema(
        schema,
        JsonSchemaOptions::new(mode, by_alias),
        JsonIntegerProfile::Exact,
    )
    .unwrap_or_else(|error| panic!("JSON Schema generation failed: {error}"))
}

fn recursive_schema() -> Schema {
    let identity = primitive("tests.SnapshotNode");
    let node = Schema::Model(
        ModelSchema::new(
            "tests.SnapshotNode",
            identity,
            vec![ModelField::required(
                "next",
                Schema::Nullable(Box::new(Schema::model_reference(
                    "SnapshotNode",
                    identity,
                    "tests.SnapshotNode",
                ))),
            )],
            ExtraPolicy::Forbid,
            false,
            true,
        )
        .unwrap_or_else(|error| panic!("recursive model setup failed: {error}")),
    );
    Schema::Definitions(
        DefinitionsSchema::new(
            Schema::model_reference("SnapshotNode", identity, "tests.SnapshotNode"),
            vec![DefinitionSchema {
                name: "SnapshotNode",
                schema: node,
            }],
        )
        .unwrap_or_else(|error| panic!("recursive definitions setup failed: {error}")),
    )
}

fn aliased_schema() -> Schema {
    let mut identifier = ModelField::required(
        "identifier",
        Schema::Integer {
            target: IntegerTarget::I32,
            constraints: IntegerConstraints::default(),
        },
    );
    identifier.metadata.insert(
        "pydantic.serialization_alias".to_owned(),
        "public_id".to_owned(),
    );
    Schema::Model(
        ModelSchema::new(
            "tests.SnapshotAlias",
            primitive("tests.SnapshotAlias"),
            vec![identifier],
            ExtraPolicy::Forbid,
            false,
            true,
        )
        .unwrap_or_else(|error| panic!("aliased model setup failed: {error}")),
    )
}

fn specialized_schema() -> Schema {
    Schema::Model(
        ModelSchema::new(
            "tests.SnapshotSpecialized",
            primitive("tests.SnapshotSpecialized"),
            vec![
                ModelField::required(
                    "fraction",
                    Schema::Fraction(FractionConstraints {
                        greater_or_equal: Some(BigRational::new(1.into(), 3.into())),
                        ..FractionConstraints::default()
                    }),
                ),
                ModelField::required("complex", Schema::Complex(ComplexConstraints::default())),
                ModelField::required(
                    "label",
                    Schema::String(StringConstraints {
                        min_length: Some(1),
                        max_length: Some(16),
                        ..StringConstraints::default()
                    }),
                ),
            ],
            ExtraPolicy::Forbid,
            false,
            true,
        )
        .unwrap_or_else(|error| panic!("specialized model setup failed: {error}")),
    )
}
