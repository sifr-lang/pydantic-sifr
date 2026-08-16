use pydantic_sifr_core::{
    BytesConstraints, CollectionConstraints, IntegerConstraints, IntegerTarget, JsonIntegerProfile,
    JsonSchemaErrorKind, JsonSchemaMode, Schema, StringConstraints, TypeAdapter,
    generate_json_schema,
};
use serde_json::json;

#[test]
fn adapter_generates_schema_from_its_prepared_core_schema() {
    let schema = Schema::List {
        item: Box::new(Schema::Integer {
            target: IntegerTarget::I16,
            constraints: IntegerConstraints::default(),
        }),
        constraints: CollectionConstraints {
            min_length: Some(1),
            max_length: Some(3),
        },
    };
    let adapter = TypeAdapter::<Vec<i16>>::new(&schema, JsonIntegerProfile::Exact)
        .unwrap_or_else(|error| panic!("adapter setup failed: {error}"));

    let generated = adapter
        .json_schema(JsonSchemaMode::Validation)
        .unwrap_or_else(|error| panic!("JSON Schema generation failed: {error}"));

    assert_eq!(
        generated,
        json!({
            "type": "array",
            "items": {"type": "integer", "minimum": -32768, "maximum": 32767},
            "minItems": 1,
            "maxItems": 3
        })
    );
}

#[test]
fn mode_specific_controls_select_the_declared_core_schema_branch() {
    let schema = Schema::json_or_structural(
        Schema::String(StringConstraints {
            min_length: Some(2),
            ..StringConstraints::default()
        }),
        Schema::String(StringConstraints {
            max_length: Some(4),
            ..StringConstraints::default()
        }),
    )
    .unwrap_or_else(|error| panic!("control schema setup failed: {error}"));

    let validation = generate_json_schema(
        &schema,
        JsonSchemaMode::Validation,
        JsonIntegerProfile::Exact,
    )
    .unwrap_or_else(|error| panic!("validation schema failed: {error}"));
    let serialization = generate_json_schema(
        &schema,
        JsonSchemaMode::Serialization,
        JsonIntegerProfile::Exact,
    )
    .unwrap_or_else(|error| panic!("serialization schema failed: {error}"));

    assert_eq!(validation, json!({"type": "string", "minLength": 2}));
    assert_eq!(serialization, json!({"type": "string", "maxLength": 4}));
}

#[test]
fn unsupported_schema_fails_instead_of_emitting_a_permissive_fallback() {
    let Err(error) = generate_json_schema(
        &Schema::Fraction(Default::default()),
        JsonSchemaMode::Validation,
        JsonIntegerProfile::Exact,
    ) else {
        panic!("specialized numeric schema must fail closed until implemented");
    };

    assert_eq!(
        error.kind(),
        pydantic_sifr_core::JsonSchemaErrorKind::UnsupportedSchema
    );
}

#[test]
fn fixed_integer_bounds_are_intersected_with_declared_constraints() {
    let generated = generate_json_schema(
        &Schema::Integer {
            target: IntegerTarget::I32,
            constraints: IntegerConstraints {
                greater_or_equal: Some((-10_000_000_000_i64).into()),
                less_or_equal: Some(10_000_000_000_i64.into()),
                ..IntegerConstraints::default()
            },
        },
        JsonSchemaMode::Validation,
        JsonIntegerProfile::Exact,
    )
    .unwrap_or_else(|error| panic!("integer schema failed: {error}"));

    assert_eq!(generated["minimum"], json!(i32::MIN));
    assert_eq!(generated["maximum"], json!(i32::MAX));
}

#[test]
fn bytes_and_decimal_fail_closed_until_their_representations_are_exact() {
    for schema in [
        Schema::Bytes(BytesConstraints::default()),
        Schema::Decimal(Default::default()),
    ] {
        let Err(error) = generate_json_schema(
            &schema,
            JsonSchemaMode::Validation,
            JsonIntegerProfile::Exact,
        ) else {
            panic!("schema without an exact representation must fail closed");
        };
        assert_eq!(error.kind(), JsonSchemaErrorKind::UnsupportedSchema);
    }
}

#[test]
fn non_positive_multiple_of_is_rejected_as_invalid_json_schema() {
    let Err(error) = generate_json_schema(
        &Schema::Integer {
            target: IntegerTarget::Exact,
            constraints: IntegerConstraints {
                multiple_of: Some(0.into()),
                ..IntegerConstraints::default()
            },
        },
        JsonSchemaMode::Validation,
        JsonIntegerProfile::Exact,
    ) else {
        panic!("zero multipleOf must fail JSON Schema generation");
    };
    assert_eq!(error.kind(), JsonSchemaErrorKind::InvalidNumber);
}
