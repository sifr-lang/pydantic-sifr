#![no_main]

use libfuzzer_sys::fuzz_target;
use pydantic_sifr_core::{
    ClockSnapshot, InputProfile, JsonLimits, PatternSchema, RelativeTimeConstraint, Schema,
    TemporalKind, TemporalSchema, ValidationLimits, ValidationOptions, parse_json, validate,
};

fuzz_target!(|data: &[u8]| {
    let Ok(input) = parse_json(
        data,
        JsonLimits {
            max_input_bytes: 1_048_576,
            max_depth: 32,
            max_nodes: 16_384,
            max_string_bytes: 1_048_576,
            max_integer_digits: 4_300,
            max_collection_items: 16_384,
        },
    ) else {
        return;
    };
    let selector = data.first().copied().unwrap_or_default();
    let schema = match selector % 7 {
        0 => Schema::Temporal(TemporalSchema {
            kind: TemporalKind::Date,
            relative: Some(RelativeTimeConstraint::Past),
        }),
        1 => Schema::Temporal(TemporalSchema {
            kind: TemporalKind::Time,
            relative: None,
        }),
        2 => Schema::Temporal(TemporalSchema {
            kind: TemporalKind::DateTime,
            relative: Some(RelativeTimeConstraint::Future),
        }),
        3 => Schema::Temporal(TemporalSchema {
            kind: TemporalKind::Duration,
            relative: None,
        }),
        4 => Schema::Uuid { version: Some(4) },
        5 => Schema::Url,
        _ => Schema::Pattern(PatternSchema {
            case_insensitive: selector & 8 != 0,
            multi_line: selector & 16 != 0,
            dot_matches_new_line: selector & 32 != 0,
        }),
    };
    let _ = validate(
        &schema,
        &input,
        ValidationOptions {
            strict: selector & 64 != 0,
            profile: InputProfile::Json,
            limits: ValidationLimits {
                max_depth: 32,
                max_collection_items: 16_384,
                max_string_bytes: 1_048_576,
                ..ValidationLimits::default()
            },
            clock: ClockSnapshot {
                unix_seconds: 1_704_067_200,
                microsecond: 0,
            },
        },
    );
});
