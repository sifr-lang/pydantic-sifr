#![no_main]

use libfuzzer_sys::fuzz_target;
use pydantic_sifr_core::{CollectionConstraints, PreparedSchema, Schema};

fuzz_target!(|data: &[u8]| {
    let mut schema = Schema::Bool;
    for byte in data.iter().take(300) {
        schema = if byte & 1 == 0 {
            Schema::Nullable(Box::new(schema))
        } else {
            Schema::List {
                item: Box::new(schema),
                constraints: CollectionConstraints::default(),
            }
        };
    }
    let _ = PreparedSchema::new(&schema);
});
