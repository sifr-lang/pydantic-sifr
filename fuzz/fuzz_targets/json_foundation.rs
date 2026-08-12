#![no_main]

use libfuzzer_sys::fuzz_target;
use pydantic_sifr_core::{JsonLimits, parse_json};

fuzz_target!(|data: &[u8]| {
    let _ = parse_json(
        data,
        JsonLimits {
            max_input_bytes: 1_048_576,
            max_depth: 64,
            max_nodes: 16_384,
            max_string_bytes: 1_048_576,
        },
    );
});
