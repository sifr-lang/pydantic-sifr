#![no_main]

use libfuzzer_sys::fuzz_target;
use pydantic_sifr_core::{
    BytesConstraints, DecimalConstraints, FloatConstraints, FractionConstraints, InputProfile,
    JsonLimits, Schema, StringConstraints, ValidationOptions, parse_json, validate,
};

fuzz_target!(|data: &[u8]| {
    let limits = JsonLimits {
        max_input_bytes: 1_048_576,
        max_depth: 64,
        max_nodes: 16_384,
        max_string_bytes: 1_048_576,
        max_integer_digits: 4_300,
        max_collection_items: 16_384,
    };
    let Ok(input) = parse_json(data, limits) else {
        return;
    };
    let schema = match data.first().map(|byte| byte % 6) {
        Some(0) => Schema::exact_integer(),
        Some(1) => Schema::Float(FloatConstraints::default()),
        Some(2) => Schema::Decimal(DecimalConstraints {
            max_digits: Some(4_300),
            decimal_places: Some(4_300),
            ..DecimalConstraints::default()
        }),
        Some(3) => Schema::Fraction(FractionConstraints::default()),
        Some(4) => Schema::String(StringConstraints::default()),
        _ => Schema::Bytes(BytesConstraints::default()),
    };
    let selector = data.get(1).copied().unwrap_or_default();
    let profile = match selector % 3 {
        0 => InputProfile::Native,
        1 => InputProfile::Json,
        _ => InputProfile::Strings,
    };
    let _ = validate(
        &schema,
        &input,
        ValidationOptions {
            strict: selector & 4 != 0,
            profile,
            ..ValidationOptions::default()
        },
    );
});
