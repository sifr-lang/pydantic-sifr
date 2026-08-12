use pydantic_sifr_core::{
    ClockSnapshot, InputProfile, JsonLimits, NativeValue, PatternSchema, RelativeTimeConstraint,
    Schema, TemporalKind, TemporalSchema, ValidatedArena, ValidatedValue, ValidationError,
    ValidationOptions, build_native_input, parse_json, validate,
};

fn require_error(result: Result<ValidatedArena, ValidationError>) -> ValidationError {
    match result {
        Ok(_) => panic!("expected validation error"),
        Err(error) => error,
    }
}

fn root(arena: &ValidatedArena) -> &ValidatedValue {
    arena
        .get(arena.root())
        .unwrap_or_else(|| panic!("validated root must exist"))
}

fn temporal(kind: TemporalKind, relative: Option<RelativeTimeConstraint>) -> Schema {
    Schema::Temporal(TemporalSchema { kind, relative })
}

#[test]
fn native_temporal_values_validate_strictly_and_preserve_components() {
    let cases = [
        (
            temporal(TemporalKind::Date, None),
            NativeValue::Date("2024-02-29".to_owned()),
        ),
        (
            temporal(TemporalKind::Time, None),
            NativeValue::Time("12:34:56.123456+02:00".to_owned()),
        ),
        (
            temporal(TemporalKind::DateTime, None),
            NativeValue::DateTime("2024-02-29T12:34:56Z".to_owned()),
        ),
        (
            temporal(TemporalKind::Duration, None),
            NativeValue::Duration("P2DT3H4M5.6S".to_owned()),
        ),
    ];
    for (schema, input) in cases {
        let input = build_native_input(&input, JsonLimits::default())
            .unwrap_or_else(|error| panic!("native temporal input failed: {error}"));
        let output = validate(
            &schema,
            &input,
            ValidationOptions {
                strict: true,
                ..ValidationOptions::default()
            },
        )
        .unwrap_or_else(|error| panic!("strict temporal validation failed: {error}"));
        assert!(matches!(
            root(&output),
            ValidatedValue::Date(_)
                | ValidatedValue::Time(_)
                | ValidatedValue::DateTime(_)
                | ValidatedValue::Duration(_)
        ));
    }
}

#[test]
fn relative_temporal_constraints_use_the_injected_clock_snapshot() {
    let clock = ClockSnapshot {
        unix_seconds: 1_704_067_200,
        microsecond: 0,
    };
    let past = parse_json(br#""2023-12-31T23:59:59Z""#, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON datetime failed: {error}"));
    validate(
        &temporal(TemporalKind::DateTime, Some(RelativeTimeConstraint::Past)),
        &past,
        ValidationOptions {
            strict: true,
            profile: InputProfile::Json,
            clock,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("past datetime failed: {error}"));

    let error = require_error(validate(
        &temporal(TemporalKind::DateTime, Some(RelativeTimeConstraint::Future)),
        &past,
        ValidationOptions {
            profile: InputProfile::Json,
            clock,
            ..ValidationOptions::default()
        },
    ));
    assert_eq!(error.details()[0].code, "datetime_future");
}

#[test]
fn uuid_validation_checks_version_and_json_profile() {
    let input = parse_json(
        br#""550e8400-e29b-41d4-a716-446655440000""#,
        JsonLimits::default(),
    )
    .unwrap_or_else(|error| panic!("JSON UUID failed: {error}"));
    let output = validate(
        &Schema::Uuid { version: Some(4) },
        &input,
        ValidationOptions {
            strict: true,
            profile: InputProfile::Json,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("UUID validation failed: {error}"));
    assert!(matches!(root(&output), ValidatedValue::Uuid(_)));

    let error = require_error(validate(
        &Schema::Uuid { version: Some(1) },
        &input,
        ValidationOptions {
            profile: InputProfile::Json,
            ..ValidationOptions::default()
        },
    ));
    assert_eq!(error.details()[0].code, "uuid_version");
}

#[test]
fn url_validation_returns_a_canonical_absolute_url() {
    let input = parse_json(
        br#""https://EXAMPLE.com/a/../b?q=1""#,
        JsonLimits::default(),
    )
    .unwrap_or_else(|error| panic!("JSON URL failed: {error}"));
    let output = validate(
        &Schema::Url,
        &input,
        ValidationOptions {
            strict: true,
            profile: InputProfile::Json,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("URL validation failed: {error}"));
    assert_eq!(
        root(&output),
        &ValidatedValue::Url("https://example.com/b?q=1".to_owned())
    );
}

#[test]
fn compiled_patterns_preserve_source_and_flags_and_enforce_bounds() {
    let input = build_native_input(
        &NativeValue::Pattern {
            source: "^abc$".to_owned(),
            flags: 1,
        },
        JsonLimits::default(),
    )
    .unwrap_or_else(|error| panic!("native pattern failed: {error}"));
    let output = validate(
        &Schema::Pattern(PatternSchema {
            case_insensitive: false,
            multi_line: true,
            dot_matches_new_line: false,
        }),
        &input,
        ValidationOptions {
            strict: true,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("pattern validation failed: {error}"));
    let ValidatedValue::Pattern(pattern) = root(&output) else {
        panic!("expected compiled pattern");
    };
    assert_eq!(pattern.source(), "^abc$");
    assert_eq!(pattern.flags(), 3);
    assert!(pattern.is_match("ABC"));

    let unsupported = build_native_input(
        &NativeValue::Pattern {
            source: "abc".to_owned(),
            flags: 128,
        },
        JsonLimits::default(),
    )
    .unwrap_or_else(|error| panic!("native pattern failed: {error}"));
    let error = require_error(validate(
        &Schema::Pattern(PatternSchema {
            case_insensitive: false,
            multi_line: false,
            dot_matches_new_line: false,
        }),
        &unsupported,
        ValidationOptions::default(),
    ));
    assert_eq!(error.details()[0].code, "pattern_flags");
}

#[test]
fn strict_native_special_schemas_reject_plain_strings() {
    let input = build_native_input(
        &NativeValue::String("2024-01-01".to_owned()),
        JsonLimits::default(),
    )
    .unwrap_or_else(|error| panic!("native string failed: {error}"));
    let error = require_error(validate(
        &temporal(TemporalKind::Date, None),
        &input,
        ValidationOptions {
            strict: true,
            ..ValidationOptions::default()
        },
    ));
    assert_eq!(error.details()[0].code, "temporal_type");
}
