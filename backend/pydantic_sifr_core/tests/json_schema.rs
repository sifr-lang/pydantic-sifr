use pydantic_sifr_core::{
    BytesConstraints, CollectionConstraints, EnumSchema, EnumVariant, IntegerConstraints,
    IntegerTarget, JsonIntegerProfile, JsonSchemaErrorKind, JsonSchemaMode, LiteralSchema,
    LiteralValue, Schema, StringConstraints, TypeAdapter, generate_json_schema,
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
            "items": {
                "type": "integer",
                "minimum": -32768,
                "maximum": 32767,
                "x-sifr-integer-profile": "exact",
                "x-sifr-generated-client-warning":
                    "client must use an exact integer JSON parser for this field"
            },
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

#[test]
fn integer_profiles_have_distinct_static_schema_representations() {
    let schema = Schema::Integer {
        target: IntegerTarget::I32,
        constraints: IntegerConstraints::default(),
    };
    let web = generate_json_schema(
        &schema,
        JsonSchemaMode::Serialization,
        JsonIntegerProfile::Web,
    )
    .unwrap_or_else(|error| panic!("web schema failed: {error}"));
    assert_eq!(web["type"], "integer");
    assert_eq!(web["minimum"], i32::MIN);
    assert_eq!(web["maximum"], i32::MAX);
    assert_eq!(web["x-sifr-integer-profile"], "web");

    let strings = generate_json_schema(
        &schema,
        JsonSchemaMode::Serialization,
        JsonIntegerProfile::StringInts,
    )
    .unwrap_or_else(|error| panic!("string integer schema failed: {error}"));
    assert_eq!(strings["type"], "string");
    assert_eq!(strings["pattern"], "^-?[0-9]+$");
    assert_eq!(strings["x-sifr-format"], "integer-decimal-string");
    assert_eq!(strings["x-sifr-minimum"], i32::MIN);
    assert_eq!(strings["x-sifr-maximum"], i32::MAX);
}

#[test]
fn validation_mode_keeps_numeric_input_for_string_integer_output() {
    let generated = generate_json_schema(
        &Schema::Integer {
            target: IntegerTarget::I64,
            constraints: IntegerConstraints::default(),
        },
        JsonSchemaMode::Validation,
        JsonIntegerProfile::StringInts,
    )
    .unwrap_or_else(|error| panic!("validation schema failed: {error}"));

    assert_eq!(generated["type"], "integer");
    assert_eq!(generated["x-sifr-integer-profile"], "string_ints");
}

#[test]
fn unsafe_web_range_fails_with_the_compiler_owned_diagnostic_code() {
    let Err(error) = generate_json_schema(
        &Schema::Integer {
            target: IntegerTarget::I64,
            constraints: IntegerConstraints::default(),
        },
        JsonSchemaMode::Serialization,
        JsonIntegerProfile::Web,
    ) else {
        panic!("wide json.web integer must fail closed");
    };

    assert_eq!(error.kind(), JsonSchemaErrorKind::IntegerPolicy);
    assert_eq!(error.diagnostic_code(), Some("SIFR-INT-0009"));
}

#[test]
fn safe_constraints_authorize_exact_integer_under_web_profile() {
    let safe = 9_007_199_254_740_991_i64;
    let generated = generate_json_schema(
        &Schema::Integer {
            target: IntegerTarget::Exact,
            constraints: IntegerConstraints {
                greater_or_equal: Some((-safe).into()),
                less_or_equal: Some(safe.into()),
                ..IntegerConstraints::default()
            },
        },
        JsonSchemaMode::Serialization,
        JsonIntegerProfile::Web,
    )
    .unwrap_or_else(|error| panic!("bounded web schema failed: {error}"));

    assert_eq!(generated["minimum"], -safe);
    assert_eq!(generated["maximum"], safe);
    assert_eq!(generated["x-sifr-integer-profile"], "web");
}

#[test]
fn integer_literals_follow_the_selected_serialization_profile() {
    let schema = Schema::Literal(
        LiteralSchema::new(vec![LiteralValue::Integer(42.into())])
            .unwrap_or_else(|error| panic!("literal schema setup failed: {error}")),
    );
    let generated = generate_json_schema(
        &schema,
        JsonSchemaMode::Serialization,
        JsonIntegerProfile::StringInts,
    )
    .unwrap_or_else(|error| panic!("literal schema failed: {error}"));

    assert_eq!(generated["const"], "42");
    assert_eq!(generated["x-sifr-integer-profile"], "string_ints");
    assert_eq!(generated["x-sifr-minimum"], 42);
    assert_eq!(generated["x-sifr-maximum"], 42);
    assert_eq!(generated["x-sifr-format"], "integer-decimal-string");
}

#[test]
fn unsafe_web_integer_literals_fail_closed() {
    let unsafe_value = num_bigint::BigInt::from(9_007_199_254_740_992_i64);
    let schema = Schema::Literal(
        LiteralSchema::new(vec![LiteralValue::Integer(unsafe_value)])
            .unwrap_or_else(|error| panic!("literal schema setup failed: {error}")),
    );
    let Err(error) = generate_json_schema(
        &schema,
        JsonSchemaMode::Serialization,
        JsonIntegerProfile::Web,
    ) else {
        panic!("unsafe web literal must fail closed");
    };

    assert_eq!(error.kind(), JsonSchemaErrorKind::IntegerPolicy);
    assert_eq!(error.diagnostic_code(), Some("SIFR-INT-0009"));
}

#[test]
fn integer_enum_variants_share_literal_profile_handling() {
    let schema = Schema::Enum(
        EnumSchema::new(
            "Status",
            vec![
                EnumVariant {
                    name: "ready",
                    input: LiteralValue::Integer(1.into()),
                    discriminant: 0,
                },
                EnumVariant {
                    name: "done",
                    input: LiteralValue::Integer(2.into()),
                    discriminant: 1,
                },
            ],
        )
        .unwrap_or_else(|error| panic!("enum schema setup failed: {error}")),
    );
    let generated = generate_json_schema(
        &schema,
        JsonSchemaMode::Serialization,
        JsonIntegerProfile::StringInts,
    )
    .unwrap_or_else(|error| panic!("enum schema failed: {error}"));

    assert_eq!(generated["enum"], json!(["1", "2"]));
    assert_eq!(generated["x-sifr-integer-profile"], "string_ints");
}
