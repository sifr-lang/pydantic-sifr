use proptest::prelude::*;
use pydantic_sifr_core::{
    ClockSnapshot, CollectionConstraints, InputProfile, JsonLimits, NativeValue, PatternSchema,
    RelativeTimeConstraint, Schema, StringConstraints, TemporalKind, TemporalSchema,
    UrlConstraints, ValidatedArena, ValidatedValue, ValidationError, ValidationOptions,
    build_native_input, parse_json, validate,
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

    let date = parse_json(br#""2024-01-01""#, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON date failed: {error}"));
    let date_error = require_error(validate(
        &temporal(TemporalKind::Date, Some(RelativeTimeConstraint::Past)),
        &date,
        ValidationOptions {
            profile: InputProfile::Json,
            clock,
            ..ValidationOptions::default()
        },
    ));
    assert_eq!(date_error.details()[0].code, "date_past");
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
        &Schema::Url(UrlConstraints::default()),
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
fn url_pattern_and_relative_schema_rejections_are_typed() {
    let opaque = parse_json(br#""data:text/plain,hello""#, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON opaque URL failed: {error}"));
    let url_error = require_error(validate(
        &Schema::Url(UrlConstraints::default()),
        &opaque,
        ValidationOptions {
            profile: InputProfile::Json,
            ..ValidationOptions::default()
        },
    ));
    assert_eq!(url_error.details()[0].code, "url_scheme");

    let invalid_pattern = parse_json(br#""(""#, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON pattern failed: {error}"));
    let pattern_error = require_error(validate(
        &Schema::Pattern(PatternSchema {
            case_insensitive: false,
            multi_line: false,
            dot_matches_new_line: false,
        }),
        &invalid_pattern,
        ValidationOptions {
            profile: InputProfile::Json,
            ..ValidationOptions::default()
        },
    ));
    assert_eq!(pattern_error.details()[0].code, "pattern_parsing");

    for kind in [TemporalKind::Time, TemporalKind::Duration] {
        let source = if kind == TemporalKind::Time {
            br#""12:00:00""#.as_slice()
        } else {
            br#""P1D""#.as_slice()
        };
        let input = parse_json(source, JsonLimits::default())
            .unwrap_or_else(|error| panic!("JSON temporal failed: {error}"));
        let error = require_error(validate(
            &temporal(kind, Some(RelativeTimeConstraint::Past)),
            &input,
            ValidationOptions {
                profile: InputProfile::Json,
                ..ValidationOptions::default()
            },
        ));
        assert_eq!(error.details()[0].code, "schema_invalid");
    }
}

#[test]
fn clock_snapshot_seconds_never_switch_to_millisecond_inference() {
    let input = parse_json(br#""2604-01-01""#, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON date failed: {error}"));
    let error = require_error(validate(
        &temporal(TemporalKind::Date, Some(RelativeTimeConstraint::Future)),
        &input,
        ValidationOptions {
            profile: InputProfile::Json,
            clock: ClockSnapshot {
                unix_seconds: 20_100_000_000,
                microsecond: 0,
            },
            ..ValidationOptions::default()
        },
    ));
    assert_eq!(error.details()[0].code, "date_future");
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

#[test]
fn collection_identity_distinguishes_naive_and_explicit_utc_temporals() {
    for (schema, values) in [
        (
            temporal(TemporalKind::Time, None),
            vec![
                NativeValue::Time("12:00:00".to_owned()),
                NativeValue::Time("12:00:00Z".to_owned()),
            ],
        ),
        (
            temporal(TemporalKind::DateTime, None),
            vec![
                NativeValue::DateTime("2024-01-01T00:00:00".to_owned()),
                NativeValue::DateTime("2024-01-01T00:00:00Z".to_owned()),
            ],
        ),
    ] {
        let input = build_native_input(&NativeValue::Set(values), JsonLimits::default())
            .unwrap_or_else(|error| panic!("native temporal set failed: {error}"));
        let output = validate(
            &Schema::Set {
                item: Box::new(schema),
                constraints: CollectionConstraints {
                    min_length: Some(2),
                    max_length: Some(2),
                },
            },
            &input,
            ValidationOptions {
                strict: true,
                ..ValidationOptions::default()
            },
        )
        .unwrap_or_else(|error| panic!("temporal set validation failed: {error}"));
        let ValidatedValue::Set(items) = root(&output) else {
            panic!("expected temporal set");
        };
        assert_eq!(items.len(), 2);
    }

    let mapping = build_native_input(
        &NativeValue::Mapping(vec![
            (
                NativeValue::DateTime("2024-01-01T00:00:00".to_owned()),
                NativeValue::String("naive".to_owned()),
            ),
            (
                NativeValue::DateTime("2024-01-01T00:00:00Z".to_owned()),
                NativeValue::String("utc".to_owned()),
            ),
        ]),
        JsonLimits::default(),
    )
    .unwrap_or_else(|error| panic!("native temporal mapping failed: {error}"));
    let output = validate(
        &Schema::Mapping {
            key: Box::new(temporal(TemporalKind::DateTime, None)),
            value: Box::new(Schema::String(StringConstraints::default())),
            constraints: CollectionConstraints::default(),
        },
        &mapping,
        ValidationOptions {
            strict: true,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("temporal mapping validation failed: {error}"));
    let ValidatedValue::Mapping(entries) = root(&output) else {
        panic!("expected temporal mapping");
    };
    assert_eq!(entries.len(), 2);
}

#[test]
fn url_validation_enforces_source_length_and_allowed_schemes() {
    let https = parse_json(br#""https://example.com/path""#, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON URL failed: {error}"));
    let constraints = UrlConstraints {
        max_length: Some(24),
        allowed_schemes: vec!["https".to_owned()],
    };
    validate(
        &Schema::Url(constraints.clone()),
        &https,
        ValidationOptions {
            profile: InputProfile::Json,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("constrained URL failed: {error}"));

    let too_long = require_error(validate(
        &Schema::Url(UrlConstraints {
            max_length: Some(23),
            ..constraints.clone()
        }),
        &https,
        ValidationOptions {
            profile: InputProfile::Json,
            ..ValidationOptions::default()
        },
    ));
    assert_eq!(too_long.details()[0].code, "url_too_long");

    let http = parse_json(br#""http://example.com/path""#, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON URL failed: {error}"));
    let wrong_scheme = require_error(validate(
        &Schema::Url(constraints),
        &http,
        ValidationOptions {
            profile: InputProfile::Json,
            ..ValidationOptions::default()
        },
    ));
    assert_eq!(wrong_scheme.details()[0].code, "url_scheme");
    assert_eq!(
        wrong_scheme.details()[0]
            .context
            .get("scheme")
            .map(String::as_str),
        Some("http")
    );
}

proptest! {
    #[test]
    fn arbitrary_special_json_never_panics(
        bytes in proptest::collection::vec(any::<u8>(), 0..8192),
        selector in any::<u8>(),
    ) {
        let Ok(input) = parse_json(&bytes, JsonLimits {
            max_input_bytes: 8192,
            max_depth: 16,
            max_nodes: 1024,
            max_string_bytes: 8192,
            max_integer_digits: 1024,
            max_collection_items: 1024,
        }) else {
            return Ok(());
        };
        let schema = match selector % 5 {
            0 => Schema::Url(UrlConstraints::default()),
            1 => Schema::Uuid { version: None },
            2 => temporal(TemporalKind::Date, None),
            3 => temporal(TemporalKind::DateTime, None),
            _ => Schema::Pattern(PatternSchema {
                case_insensitive: false,
                multi_line: false,
                dot_matches_new_line: false,
            }),
        };
        let result = std::panic::catch_unwind(|| {
            validate(
                &schema,
                &input,
                ValidationOptions {
                    profile: InputProfile::Json,
                    ..ValidationOptions::default()
                },
            )
        });
        prop_assert!(result.is_ok());
    }
}
