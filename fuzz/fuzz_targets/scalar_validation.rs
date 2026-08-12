#![no_main]

use libfuzzer_sys::fuzz_target;
use pydantic_sifr_core::{
    BytesConstraints, DecimalConstraints, FloatConstraints, JsonLimits, Schema,
    StringConstraints, ValidationOptions, parse_json, validate,
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
    let schema = match data.first().map(|byte| byte % 5) {
        Some(0) => Schema::exact_integer(),
        Some(1) => Schema::Float(FloatConstraints::default()),
        Some(2) => Schema::Decimal(DecimalConstraints {
            max_digits: Some(4_300),
            decimal_places: Some(4_300),
            ..DecimalConstraints::default()
        }),
        Some(3) => Schema::String(StringConstraints::default()),
        _ => Schema::Bytes(BytesConstraints::default()),
    };
    let _ = validate(&schema, &input, ValidationOptions::default());
});
