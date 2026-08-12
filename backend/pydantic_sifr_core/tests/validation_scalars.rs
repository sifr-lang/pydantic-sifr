use bigdecimal::BigDecimal;
use num_bigint::BigInt;
use num_complex::Complex64;
use num_rational::BigRational;
use pydantic_sifr_core::{
    ComplexConstraints, DecimalConstraints, InputProfile, IntegerConstraints, IntegerTarget,
    JsonLimits, NativeValue, Schema, StringConstraints, StringPattern, ValidatedArena,
    ValidatedValue, ValidationError, ValidationOptions, build_native_input, parse_json, validate,
};

fn require_validation_error(result: Result<ValidatedArena, ValidationError>) -> ValidationError {
    match result {
        Ok(_) => panic!("expected validation error"),
        Err(error) => error,
    }
}

fn validate_native(
    schema: &Schema,
    value: NativeValue,
    strict: bool,
) -> Result<ValidatedArena, ValidationError> {
    let input = build_native_input(&value, JsonLimits::default())
        .unwrap_or_else(|error| panic!("native input failed: {error}"));
    validate(
        schema,
        &input,
        ValidationOptions {
            strict,
            ..ValidationOptions::default()
        },
    )
}

fn root(arena: &ValidatedArena) -> &ValidatedValue {
    arena
        .get(arena.root())
        .unwrap_or_else(|| panic!("validated root must exist"))
}

fn first_code(error: &ValidationError) -> &'static str {
    error
        .details()
        .first()
        .map(|detail| detail.code)
        .unwrap_or("missing_error")
}

#[test]
fn exact_integer_preserves_arbitrary_precision_and_fixed_targets_reject_overflow() {
    let exact = validate_native(
        &Schema::exact_integer(),
        NativeValue::Integer("123456789012345678901234567890".to_owned()),
        true,
    )
    .unwrap_or_else(|error| panic!("exact integer failed: {error}"));
    assert_eq!(
        root(&exact),
        &ValidatedValue::ExactInt(
            "123456789012345678901234567890"
                .parse::<BigInt>()
                .unwrap_or_else(|error| panic!("test integer failed: {error}"))
        )
    );

    let schema = Schema::Integer {
        target: IntegerTarget::I8,
        constraints: IntegerConstraints::default(),
    };
    let error = require_validation_error(validate_native(
        &schema,
        NativeValue::Integer("128".to_owned()),
        true,
    ));
    assert_eq!(first_code(&error), "integer_overflow");
}

#[test]
fn integer_strict_lax_and_strings_profiles_are_distinct() {
    let strict_error = require_validation_error(validate_native(
        &Schema::exact_integer(),
        NativeValue::String("42".to_owned()),
        true,
    ));
    assert_eq!(first_code(&strict_error), "int_type");

    let lax = validate_native(
        &Schema::exact_integer(),
        NativeValue::String("42".to_owned()),
        false,
    )
    .unwrap_or_else(|error| panic!("lax integer failed: {error}"));
    assert_eq!(root(&lax), &ValidatedValue::ExactInt(BigInt::from(42)));

    let input = build_native_input(&NativeValue::String("42".to_owned()), JsonLimits::default())
        .unwrap_or_else(|error| panic!("native input failed: {error}"));
    let strings = validate(
        &Schema::exact_integer(),
        &input,
        ValidationOptions {
            strict: true,
            profile: InputProfile::Strings,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("strings integer failed: {error}"));
    assert_eq!(root(&strings), &ValidatedValue::ExactInt(BigInt::from(42)));
}

#[test]
fn decimal_fraction_and_complex_values_keep_exact_or_structural_meaning() {
    let decimal = validate_native(
        &Schema::Decimal(DecimalConstraints {
            max_digits: Some(6),
            decimal_places: Some(3),
            ..DecimalConstraints::default()
        }),
        NativeValue::Decimal("12.340".to_owned()),
        true,
    )
    .unwrap_or_else(|error| panic!("decimal failed: {error}"));
    assert_eq!(
        root(&decimal),
        &ValidatedValue::Decimal(
            "12.340"
                .parse::<BigDecimal>()
                .unwrap_or_else(|error| panic!("test decimal failed: {error}"))
        )
    );

    let fraction = validate_native(
        &Schema::Fraction(IntegerConstraints::default()),
        NativeValue::Fraction {
            numerator: "6".to_owned(),
            denominator: "-8".to_owned(),
        },
        true,
    )
    .unwrap_or_else(|error| panic!("fraction failed: {error}"));
    assert_eq!(
        root(&fraction),
        &ValidatedValue::Fraction(BigRational::new(BigInt::from(-3), BigInt::from(4)))
    );

    let complex = validate_native(
        &Schema::Complex(ComplexConstraints::default()),
        NativeValue::Complex {
            real: 3.0,
            imaginary: 4.0,
        },
        true,
    )
    .unwrap_or_else(|error| panic!("complex failed: {error}"));
    assert_eq!(
        root(&complex),
        &ValidatedValue::Complex(Complex64::new(3.0, 4.0))
    );
}

#[test]
fn string_pipeline_strips_then_checks_length_and_pattern_then_changes_case() {
    let pattern = StringPattern::compile("^[a-z]{3}$")
        .unwrap_or_else(|error| panic!("test pattern failed: {error}"));
    let schema = Schema::String(StringConstraints {
        strip_whitespace: true,
        ascii_only: true,
        min_length: Some(3),
        max_length: Some(3),
        pattern: Some(pattern),
        to_upper: true,
        ..StringConstraints::default()
    });
    let value = validate_native(&schema, NativeValue::String("  abc  ".to_owned()), true)
        .unwrap_or_else(|error| panic!("string failed: {error}"));
    assert_eq!(root(&value), &ValidatedValue::String("ABC".to_owned()));
}

#[test]
fn native_and_json_integer_limits_fail_before_large_numeric_allocation() {
    let limits = JsonLimits {
        max_integer_digits: 4,
        ..JsonLimits::default()
    };
    let native = match build_native_input(&NativeValue::Integer("12345".to_owned()), limits) {
        Ok(_) => panic!("expected native integer limit error"),
        Err(error) => error,
    };
    assert!(native.to_string().contains("maximum integer digits"));

    let json = match parse_json(b"12345", limits) {
        Ok(_) => panic!("expected JSON integer limit error"),
        Err(error) => error,
    };
    assert_eq!(json.code, "json_integer_limit");
}
