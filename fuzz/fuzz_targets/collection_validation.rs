#![no_main]

use libfuzzer_sys::fuzz_target;
use pydantic_sifr_core::{
    CollectionConstraints, InputProfile, JsonLimits, Schema, StringConstraints,
    ValidationLimits, ValidationOptions, parse_json, validate, validated_iterator,
};

fuzz_target!(|data: &[u8]| {
    let input_limits = JsonLimits {
        max_input_bytes: 1_048_576,
        max_depth: 64,
        max_nodes: 16_384,
        max_string_bytes: 1_048_576,
        max_integer_digits: 4_300,
        max_collection_items: 16_384,
    };
    let Ok(input) = parse_json(data, input_limits) else {
        return;
    };
    let selector = data.first().copied().unwrap_or_default();
    let item = Box::new(Schema::String(StringConstraints::default()));
    let constraints = CollectionConstraints {
        min_length: Some(0),
        max_length: Some(16_384),
    };
    let schema = match selector % 6 {
        0 => Schema::List { item, constraints },
        1 => Schema::Tuple(vec![
            Schema::exact_integer(),
            Schema::String(StringConstraints::default()),
        ]),
        2 => Schema::Mapping {
            key: item.clone(),
            value: Box::new(Schema::exact_integer()),
            constraints,
        },
        3 => Schema::Set { item, constraints },
        4 => Schema::FrozenSet { item, constraints },
        _ => Schema::Generator { item, constraints },
    };
    let options = ValidationOptions {
        strict: selector & 8 != 0,
        profile: InputProfile::Json,
        limits: ValidationLimits {
            max_depth: 64,
            max_collection_items: 16_384,
            max_string_bytes: 1_048_576,
            max_errors: 32,
            ..ValidationLimits::default()
        },
        ..ValidationOptions::default()
    };
    if matches!(schema, Schema::Generator { .. }) {
        if let Ok(mut iterator) = validated_iterator(&schema, &input, options) {
            for _ in iterator.by_ref().take(64) {}
        }
    } else {
        let _ = validate(&schema, &input, options);
    }
});
