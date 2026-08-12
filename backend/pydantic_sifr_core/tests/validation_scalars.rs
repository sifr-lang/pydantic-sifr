use bigdecimal::BigDecimal;
use num_bigint::BigInt;
use num_complex::Complex64;
use num_rational::BigRational;
use pydantic_sifr_core::{
    BytesConstraints, BytesJsonMode, ComplexConstraints, DecimalConstraints, FloatConstraints,
    FractionConstraints, InputProfile, IntegerConstraints, IntegerTarget, JsonLimits, NativeValue,
    Schema, StringConstraints, StringPattern, ValidatedArena, ValidatedValue, ValidationError,
    ValidationOptions, build_native_input, parse_json, validate,
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
        &Schema::Fraction(FractionConstraints::default()),
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

#[test]
fn fraction_constraints_compare_the_exact_rational_value() {
    let one = BigRational::from_integer(BigInt::from(1));
    let schema = Schema::Fraction(FractionConstraints {
        greater_or_equal: Some(one),
        multiple_of: Some(BigRational::new(BigInt::from(1), BigInt::from(4))),
        ..FractionConstraints::default()
    });
    let error = require_validation_error(validate_native(
        &schema,
        NativeValue::Fraction {
            numerator: "1".to_owned(),
            denominator: "2".to_owned(),
        },
        true,
    ));
    assert_eq!(first_code(&error), "greater_than_equal");
}

#[test]
fn decimal_and_fraction_reject_extreme_exponents_and_string_digit_bypass() {
    let decimal_error = require_validation_error(validate_native(
        &Schema::Decimal(DecimalConstraints {
            max_digits: Some(10),
            ..DecimalConstraints::default()
        }),
        NativeValue::String("1e-2000000000".to_owned()),
        false,
    ));
    assert_eq!(first_code(&decimal_error), "resource_limit");

    let fraction_error = require_validation_error(validate_native(
        &Schema::Fraction(FractionConstraints::default()),
        NativeValue::String("1e-2000000000".to_owned()),
        false,
    ));
    assert_eq!(first_code(&fraction_error), "resource_limit");

    let digits = "1".repeat(4_301);
    let integer_error = require_validation_error(validate_native(
        &Schema::exact_integer(),
        NativeValue::String(digits),
        false,
    ));
    assert_eq!(first_code(&integer_error), "resource_limit");
}

#[test]
fn decimal_digit_count_includes_normalized_whole_trailing_zeros() {
    let error = require_validation_error(validate_native(
        &Schema::Decimal(DecimalConstraints {
            max_digits: Some(2),
            ..DecimalConstraints::default()
        }),
        NativeValue::Decimal("100".to_owned()),
        true,
    ));
    assert_eq!(first_code(&error), "decimal_max_digits");

    let fractional_error = require_validation_error(validate_native(
        &Schema::Decimal(DecimalConstraints {
            max_digits: Some(3),
            ..DecimalConstraints::default()
        }),
        NativeValue::Decimal("0.00001".to_owned()),
        true,
    ));
    assert_eq!(first_code(&fractional_error), "decimal_max_digits");
}

#[test]
fn float_multiple_overflow_and_strict_strings_bytes_return_stable_results() {
    let float_error = require_validation_error(validate_native(
        &Schema::Float(FloatConstraints {
            multiple_of: Some(1e-308),
            ..FloatConstraints::default()
        }),
        NativeValue::Float(1e308),
        true,
    ));
    assert_eq!(first_code(&float_error), "multiple_of");

    for (value, multiple) in [(1e9, 3.0), (1e12, 3.0), (1_000_000_000_000.5, 1.0)] {
        let error = require_validation_error(validate_native(
            &Schema::Float(FloatConstraints {
                multiple_of: Some(multiple),
                ..FloatConstraints::default()
            }),
            NativeValue::Float(value),
            true,
        ));
        assert_eq!(first_code(&error), "multiple_of");
    }

    let input = build_native_input(
        &NativeValue::String("abc".to_owned()),
        JsonLimits::default(),
    )
    .unwrap_or_else(|error| panic!("native input failed: {error}"));
    let bytes = validate(
        &Schema::Bytes(BytesConstraints::default()),
        &input,
        ValidationOptions {
            strict: true,
            profile: InputProfile::Strings,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("strict strings bytes failed: {error}"));
    assert_eq!(root(&bytes), &ValidatedValue::Bytes(b"abc".to_vec()));
}

#[test]
fn json_profile_runs_the_same_engine_and_validation_limits_are_independent() {
    let input = parse_json(br#""42""#, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON input failed: {error}"));
    let value = validate(
        &Schema::exact_integer(),
        &input,
        ValidationOptions {
            profile: InputProfile::Json,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("JSON-profile integer failed: {error}"));
    assert_eq!(root(&value), &ValidatedValue::ExactInt(BigInt::from(42)));

    let strict_strings_error = require_validation_error(validate(
        &Schema::exact_integer(),
        &parse_json(b"42", JsonLimits::default())
            .unwrap_or_else(|error| panic!("JSON input failed: {error}")),
        ValidationOptions {
            profile: InputProfile::Strings,
            ..ValidationOptions::default()
        },
    ));
    assert_eq!(first_code(&strict_strings_error), "strings_type");

    let long = build_native_input(
        &NativeValue::String("abcdef".to_owned()),
        JsonLimits::default(),
    )
    .unwrap_or_else(|error| panic!("native input failed: {error}"));
    let limited_error = require_validation_error(validate(
        &Schema::String(StringConstraints::default()),
        &long,
        ValidationOptions {
            limits: pydantic_sifr_core::ValidationLimits {
                max_string_bytes: 5,
                ..pydantic_sifr_core::ValidationLimits::default()
            },
            ..ValidationOptions::default()
        },
    ));
    assert_eq!(first_code(&limited_error), "resource_limit");
}

#[test]
fn strict_json_accepts_json_native_representations_for_extended_scalars() {
    let options = ValidationOptions {
        strict: true,
        profile: InputProfile::Json,
        ..ValidationOptions::default()
    };
    let decimal_input = parse_json(b"1.5", JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON decimal input failed: {error}"));
    let decimal = validate(
        &Schema::Decimal(DecimalConstraints::default()),
        &decimal_input,
        options,
    )
    .unwrap_or_else(|error| panic!("strict JSON decimal failed: {error}"));
    assert_eq!(
        root(&decimal),
        &ValidatedValue::Decimal(
            "1.5"
                .parse::<BigDecimal>()
                .unwrap_or_else(|error| panic!("test decimal failed: {error}"))
        )
    );

    let fraction = validate(
        &Schema::Fraction(FractionConstraints::default()),
        &decimal_input,
        options,
    )
    .unwrap_or_else(|error| panic!("strict JSON fraction failed: {error}"));
    assert_eq!(
        root(&fraction),
        &ValidatedValue::Fraction(BigRational::new(BigInt::from(3), BigInt::from(2)))
    );

    let complex_input = parse_json(br#""3+4j""#, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON complex input failed: {error}"));
    let complex = validate(
        &Schema::Complex(ComplexConstraints::default()),
        &complex_input,
        options,
    )
    .unwrap_or_else(|error| panic!("strict JSON complex failed: {error}"));
    assert_eq!(
        root(&complex),
        &ValidatedValue::Complex(Complex64::new(3.0, 4.0))
    );

    let bytes_input = parse_json(br#""abc""#, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON bytes input failed: {error}"));
    let bytes = validate(
        &Schema::Bytes(BytesConstraints::default()),
        &bytes_input,
        options,
    )
    .unwrap_or_else(|error| panic!("strict JSON bytes failed: {error}"));
    assert_eq!(root(&bytes), &ValidatedValue::Bytes(b"abc".to_vec()));
}

#[test]
fn json_bytes_base64_policy_decodes_and_rejects_invalid_input() {
    let schema = Schema::Bytes(BytesConstraints {
        json_mode: BytesJsonMode::Base64,
        ..BytesConstraints::default()
    });
    let encoded = parse_json(br#""AAEC_f7_""#, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON base64 input failed: {error}"));
    let decoded = validate(
        &schema,
        &encoded,
        ValidationOptions {
            strict: true,
            profile: InputProfile::Json,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("base64 bytes validation failed: {error}"));
    assert_eq!(
        root(&decoded),
        &ValidatedValue::Bytes(vec![0, 1, 2, 253, 254, 255])
    );

    let invalid = parse_json(br#""not base64""#, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON base64 input failed: {error}"));
    let error = require_validation_error(validate(
        &schema,
        &invalid,
        ValidationOptions {
            profile: InputProfile::Json,
            ..ValidationOptions::default()
        },
    ));
    assert_eq!(first_code(&error), "bytes_base64");
}
