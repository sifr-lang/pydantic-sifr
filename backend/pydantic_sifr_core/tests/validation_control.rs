use num_bigint::BigInt;
use pydantic_sifr_core::{
    CollectionConstraints, ComplexConstraints, DecimalConstraints, DurationValue,
    FractionConstraints, InputProfile, IntegerConstraints, IntegerTarget, JsonLimits, NativeValue,
    PatternSchema, Schema, StringConstraints, TemporalKind, TemporalSchema, ValidatedArena,
    ValidatedValue, ValidationError, ValidationOptions, build_native_input, parse_json, validate,
};

fn integer(constraints: IntegerConstraints) -> Schema {
    Schema::Integer {
        target: IntegerTarget::Exact,
        constraints,
    }
}

fn string(constraints: StringConstraints) -> Schema {
    Schema::String(constraints)
}

fn root(arena: &ValidatedArena) -> &ValidatedValue {
    arena
        .get(arena.root())
        .unwrap_or_else(|| panic!("validated root must exist"))
}

fn require_error<T>(result: Result<T, ValidationError>) -> ValidationError {
    match result {
        Ok(_) => panic!("expected validation error"),
        Err(error) => error,
    }
}

fn native(
    schema: &Schema,
    input: &NativeValue,
    options: ValidationOptions,
) -> Result<ValidatedArena, ValidationError> {
    let input = build_native_input(input, JsonLimits::default())
        .unwrap_or_else(|error| panic!("native input failed: {error}"));
    validate(schema, &input, options)
}

#[test]
fn lax_or_strict_obeys_default_and_explicit_override() {
    let lax = integer(IntegerConstraints::default());
    let strict = integer(IntegerConstraints {
        greater_than: Some(BigInt::from(10)),
        ..IntegerConstraints::default()
    });
    let schema = Schema::lax_or_strict(lax, strict, true)
        .unwrap_or_else(|error| panic!("control schema failed: {error}"));

    let default_output = native(
        &schema,
        &NativeValue::String("12".to_owned()),
        ValidationOptions::default(),
    )
    .unwrap_or_else(|error| panic!("default strict branch failed: {error}"));
    assert!(
        matches!(root(&default_output), ValidatedValue::ExactInt(value) if value == &BigInt::from(12))
    );

    let lax_output = native(
        &schema,
        &NativeValue::String("5".to_owned()),
        ValidationOptions {
            strict_override: Some(false),
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("explicit lax branch failed: {error}"));
    assert!(
        matches!(root(&lax_output), ValidatedValue::ExactInt(value) if value == &BigInt::from(5))
    );

    let strict_error = require_error(native(
        &schema,
        &NativeValue::String("12".to_owned()),
        ValidationOptions {
            strict_override: Some(true),
            ..ValidationOptions::default()
        },
    ));
    assert_eq!(strict_error.details()[0].code, "int_type");
}

#[test]
fn control_branches_must_have_one_structural_output_type() {
    let error = require_error(Schema::lax_or_strict(
        Schema::exact_integer(),
        string(StringConstraints::default()),
        false,
    ));
    assert_eq!(error.details()[0].code, "schema_invalid");

    let error = require_error(Schema::json_or_structural(
        Schema::exact_integer(),
        string(StringConstraints::default()),
    ));
    assert_eq!(error.details()[0].code, "schema_invalid");
}

#[test]
fn json_or_structural_selects_from_the_original_input_profile() {
    let schema = Schema::json_or_structural(
        string(StringConstraints {
            to_upper: true,
            ..StringConstraints::default()
        }),
        string(StringConstraints {
            to_lower: true,
            ..StringConstraints::default()
        }),
    )
    .unwrap_or_else(|error| panic!("profile control failed: {error}"));

    let json_input = parse_json(br#""MiXeD""#, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON input failed: {error}"));
    let json_output = validate(
        &schema,
        &json_input,
        ValidationOptions {
            profile: InputProfile::Json,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("JSON branch failed: {error}"));
    assert_eq!(
        root(&json_output),
        &ValidatedValue::String("MIXED".to_owned())
    );

    let native_output = native(
        &schema,
        &NativeValue::String("MiXeD".to_owned()),
        ValidationOptions::default(),
    )
    .unwrap_or_else(|error| panic!("structural branch failed: {error}"));
    assert_eq!(
        root(&native_output),
        &ValidatedValue::String("mixed".to_owned())
    );
}

#[test]
fn chain_flattens_and_hands_each_typed_output_to_the_next_step() {
    let inner = Schema::chain(vec![
        integer(IntegerConstraints::default()),
        Schema::Float(Default::default()),
    ])
    .unwrap_or_else(|error| panic!("inner chain failed: {error}"));
    let schema = Schema::chain(vec![
        string(StringConstraints {
            strip_whitespace: true,
            ..StringConstraints::default()
        }),
        inner,
        Schema::Decimal(DecimalConstraints::default()),
    ])
    .unwrap_or_else(|error| panic!("outer chain failed: {error}"));

    let Schema::Chain(chain) = &schema else {
        panic!("multi-step chain must retain its control node");
    };
    assert_eq!(chain.steps().len(), 4);

    let output = native(
        &schema,
        &NativeValue::Bytes(b" 123 ".to_vec()),
        ValidationOptions::default(),
    )
    .unwrap_or_else(|error| panic!("typed chain failed: {error}"));
    assert!(matches!(root(&output), ValidatedValue::Decimal(value) if value.to_string() == "123"));
}

#[test]
fn chain_preserves_profile_for_nested_controls_and_errors_at_the_failing_step() {
    let routed = Schema::json_or_structural(
        string(StringConstraints {
            to_upper: true,
            ..StringConstraints::default()
        }),
        string(StringConstraints {
            to_lower: true,
            ..StringConstraints::default()
        }),
    )
    .unwrap_or_else(|error| panic!("profile control failed: {error}"));
    let schema = Schema::chain(vec![
        string(StringConstraints::default()),
        routed,
        integer(IntegerConstraints::default()),
    ])
    .unwrap_or_else(|error| panic!("chain failed: {error}"));
    let input = parse_json(br#""abc""#, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON input failed: {error}"));
    let error = require_error(validate(
        &schema,
        &input,
        ValidationOptions {
            profile: InputProfile::Json,
            ..ValidationOptions::default()
        },
    ));
    assert_eq!(error.details()[0].code, "int_parsing");
}

#[test]
fn chain_rejects_empty_and_erases_one_step() {
    let error = require_error(Schema::chain(Vec::new()));
    assert_eq!(error.details()[0].code, "schema_invalid");

    let schema = Schema::chain(vec![Schema::Bool])
        .unwrap_or_else(|error| panic!("one-step chain failed: {error}"));
    assert_eq!(schema, Schema::Bool);
}

#[test]
fn chain_handoff_preserves_specialized_and_aggregate_values() {
    let duration = Schema::Temporal(TemporalSchema {
        kind: TemporalKind::Duration,
        relative: None,
    });
    let duration_chain = Schema::chain(vec![duration.clone(), duration])
        .unwrap_or_else(|error| panic!("duration chain failed: {error}"));
    let duration_output = native(
        &duration_chain,
        &NativeValue::Duration("-P2DT3H4M5.6S".to_owned()),
        ValidationOptions::default(),
    )
    .unwrap_or_else(|error| panic!("duration handoff failed: {error}"));
    assert!(matches!(
        root(&duration_output),
        ValidatedValue::Duration(DurationValue {
            positive: false,
            days: 2,
            ..
        })
    ));

    let fraction = Schema::Fraction(FractionConstraints::default());
    let fraction_chain = Schema::chain(vec![fraction.clone(), fraction])
        .unwrap_or_else(|error| panic!("fraction chain failed: {error}"));
    let fraction_output = native(
        &fraction_chain,
        &NativeValue::Fraction {
            numerator: "6".to_owned(),
            denominator: "8".to_owned(),
        },
        ValidationOptions::default(),
    )
    .unwrap_or_else(|error| panic!("fraction handoff failed: {error}"));
    assert!(
        matches!(root(&fraction_output), ValidatedValue::Fraction(value) if value.numer() == &BigInt::from(3) && value.denom() == &BigInt::from(4))
    );

    let complex = Schema::Complex(ComplexConstraints::default());
    let complex_chain = Schema::chain(vec![complex.clone(), complex])
        .unwrap_or_else(|error| panic!("complex chain failed: {error}"));
    let complex_output = native(
        &complex_chain,
        &NativeValue::Complex {
            real: 1.25,
            imaginary: -2.5,
        },
        ValidationOptions::default(),
    )
    .unwrap_or_else(|error| panic!("complex handoff failed: {error}"));
    assert!(
        matches!(root(&complex_output), ValidatedValue::Complex(value) if value.re == 1.25 && value.im == -2.5)
    );

    let pattern = Schema::Pattern(PatternSchema {
        case_insensitive: true,
        multi_line: false,
        dot_matches_new_line: false,
    });
    let pattern_chain = Schema::chain(vec![pattern.clone(), pattern])
        .unwrap_or_else(|error| panic!("pattern chain failed: {error}"));
    let pattern_output = native(
        &pattern_chain,
        &NativeValue::Pattern {
            source: "abc".to_owned(),
            flags: 1,
        },
        ValidationOptions::default(),
    )
    .unwrap_or_else(|error| panic!("pattern handoff failed: {error}"));
    assert!(
        matches!(root(&pattern_output), ValidatedValue::Pattern(value) if value.source() == "abc" && value.flags() == 1)
    );

    let list = Schema::List {
        item: Box::new(string(StringConstraints::default())),
        constraints: CollectionConstraints::default(),
    };
    let list_chain = Schema::chain(vec![list.clone(), list])
        .unwrap_or_else(|error| panic!("list chain failed: {error}"));
    let list_output = native(
        &list_chain,
        &NativeValue::List(vec![NativeValue::String("x".to_owned())]),
        ValidationOptions::default(),
    )
    .unwrap_or_else(|error| panic!("list handoff failed: {error}"));
    assert!(matches!(root(&list_output), ValidatedValue::Sequence(items) if items.len() == 1));
}
