#![no_main]

use libfuzzer_sys::fuzz_target;
use pydantic_sifr_core::{
    CollectionConstraints, JsonLimits, Schema, StringConstraints, ValidationOptions,
    validate_json_and_construct,
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
    let options = ValidationOptions::default();
    if data.first().is_some_and(|selector| selector & 1 == 0) {
        let _ = validate_json_and_construct::<String>(
            &Schema::String(StringConstraints::default()),
            data,
            limits,
            options,
        );
    } else {
        let _ = validate_json_and_construct::<Vec<String>>(
            &Schema::List {
                item: Box::new(Schema::String(StringConstraints::default())),
                constraints: CollectionConstraints::default(),
            },
            data,
            limits,
            options,
        );
    }
});
