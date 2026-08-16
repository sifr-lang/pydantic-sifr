use pydantic_sifr_core::{
    CollectionConstraints, Complex, ComplexConstraints, Fraction, FractionConstraints,
    IntegerConstraints, IntegerTarget, JsonIntegerProfile, JsonLimits, NativeValue, Schema,
    SerializationOptions, TypeAdapter, TypeAdapterBuildErrorKind, ValidationOptions,
};

#[test]
fn reusable_adapter_validates_every_input_profile_and_serializes() {
    let schema = integer_list_schema();
    let adapter = TypeAdapter::<Vec<i64>>::new(&schema, JsonIntegerProfile::Exact)
        .unwrap_or_else(|error| panic!("adapter setup failed: {error}"));

    let json = adapter
        .validate_json(
            b"[1,2]",
            JsonLimits::default(),
            ValidationOptions::default(),
        )
        .unwrap_or_else(|error| panic!("JSON validation failed: {error}"));
    assert_eq!(json, vec![1_i64, 2_i64]);

    let native_input = NativeValue::List(vec![
        NativeValue::Integer("3".to_owned()),
        NativeValue::Integer("4".to_owned()),
    ]);
    let native = adapter
        .validate_native(
            &native_input,
            JsonLimits::default(),
            ValidationOptions::default(),
        )
        .unwrap_or_else(|error| panic!("native validation failed: {error}"));
    assert_eq!(native, vec![3_i64, 4_i64]);

    let strings_input = NativeValue::List(vec![
        NativeValue::String("5".to_owned()),
        NativeValue::String("6".to_owned()),
    ]);
    let strings = adapter
        .validate_strings(
            &strings_input,
            JsonLimits::default(),
            ValidationOptions::default(),
        )
        .unwrap_or_else(|error| panic!("strings validation failed: {error}"));
    assert_eq!(strings, vec![5_i64, 6_i64]);

    let structural = adapter
        .validate_structural(
            &vec![7_i64, 8_i64],
            JsonLimits::default(),
            ValidationOptions::default(),
        )
        .unwrap_or_else(|error| panic!("structural validation failed: {error}"));
    assert_eq!(structural, vec![7_i64, 8_i64]);

    let output = adapter
        .dump_json(&structural, &SerializationOptions::default())
        .unwrap_or_else(|error| panic!("JSON serialization failed: {error}"));
    assert_eq!(output, b"[7,8]");
    let output = adapter
        .dump_structural(&structural, &SerializationOptions::default())
        .unwrap_or_else(|error| panic!("structural serialization failed: {error}"));
    assert_eq!(
        output,
        NativeValue::List(vec![
            NativeValue::Integer("7".to_owned()),
            NativeValue::Integer("8".to_owned()),
        ])
    );
}

#[test]
fn specialized_numeric_adapters_construct_and_serialize_public_values() {
    let fraction_schema = Schema::Fraction(FractionConstraints::default());
    let fraction_adapter =
        TypeAdapter::<Fraction>::new(&fraction_schema, JsonIntegerProfile::Exact)
            .unwrap_or_else(|error| panic!("fraction adapter setup failed: {error}"));
    let fraction = fraction_adapter
        .validate_json(
            br#""6/-8""#,
            JsonLimits::default(),
            ValidationOptions::default(),
        )
        .unwrap_or_else(|error| panic!("fraction validation failed: {error}"));
    assert_eq!(fraction.to_string(), "-3/4");
    assert_eq!(
        fraction_adapter
            .dump_json(&fraction, &SerializationOptions::default())
            .unwrap_or_else(|error| panic!("fraction JSON output failed: {error}")),
        br#""-3/4""#
    );
    assert_eq!(
        fraction_adapter
            .dump_structural(&fraction, &SerializationOptions::default())
            .unwrap_or_else(|error| panic!("fraction structural output failed: {error}")),
        NativeValue::String("-3/4".to_owned())
    );

    let complex_schema = Schema::Complex(ComplexConstraints::default());
    let complex_adapter = TypeAdapter::<Complex>::new(&complex_schema, JsonIntegerProfile::Exact)
        .unwrap_or_else(|error| panic!("complex adapter setup failed: {error}"));
    let complex = complex_adapter
        .validate_json(
            br#""3+4j""#,
            JsonLimits::default(),
            ValidationOptions::default(),
        )
        .unwrap_or_else(|error| panic!("complex validation failed: {error}"));
    assert_eq!(complex.real(), 3.0);
    assert_eq!(complex.imaginary(), 4.0);
    assert_eq!(
        complex_adapter
            .dump_json(&complex, &SerializationOptions::default())
            .unwrap_or_else(|error| panic!("complex JSON output failed: {error}")),
        br#""3+4j""#
    );
}

#[test]
fn adapter_reuses_its_selected_integer_profile() {
    let schema = integer_list_schema();
    let adapter = TypeAdapter::<Vec<i64>>::new(&schema, JsonIntegerProfile::StringInts)
        .unwrap_or_else(|error| panic!("adapter setup failed: {error}"));

    let output = adapter
        .dump_json(&vec![9_i64, 10_i64], &SerializationOptions::default())
        .unwrap_or_else(|error| panic!("JSON serialization failed: {error}"));
    assert_eq!(output, br#"["9","10"]"#);
}

#[test]
fn adapter_rejects_a_target_shape_mismatch_at_construction() {
    let schema = integer_list_schema();
    let Err(error) = TypeAdapter::<String>::new(&schema, JsonIntegerProfile::Exact) else {
        panic!("adapter construction must reject a mismatched target");
    };
    assert_eq!(error.kind(), TypeAdapterBuildErrorKind::ShapeMismatch);
}

fn integer_list_schema() -> Schema {
    Schema::List {
        item: Box::new(Schema::Integer {
            target: IntegerTarget::I64,
            constraints: IntegerConstraints::default(),
        }),
        constraints: CollectionConstraints::default(),
    }
}
