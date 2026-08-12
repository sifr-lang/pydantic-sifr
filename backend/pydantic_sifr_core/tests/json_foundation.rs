use proptest::prelude::*;
use pydantic_sifr_core::{InputArena, InputValue, JsonInputError, JsonLimits, parse_json};

fn require_error(result: Result<InputArena, JsonInputError>) -> JsonInputError {
    match result {
        Ok(_) => panic!("expected JSON input error"),
        Err(error) => error,
    }
}

#[test]
fn parses_exact_integers_and_nested_input_into_checked_arena() {
    let input = br#"{"small":1,"exact":123456789012345678901234567890,"items":[true,null]}"#;
    let arena = parse_json(input, JsonLimits::default()).unwrap_or_else(|error| panic!("{error}"));
    let root = arena
        .get(arena.root())
        .unwrap_or_else(|| panic!("root must exist"));
    let InputValue::Object(fields) = root else {
        panic!("expected object root");
    };
    let exact_id = fields
        .iter()
        .find(|(key, _)| key == "exact")
        .map(|(_, id)| *id)
        .unwrap_or_else(|| panic!("exact field must exist"));
    assert_eq!(
        arena.get(exact_id),
        Some(&InputValue::Integer(
            "123456789012345678901234567890".to_owned()
        ))
    );
}

#[test]
fn malformed_json_has_stable_typed_location() {
    let error = require_error(parse_json(b"{\n  \"value\": ]", JsonLimits::default()));
    assert_eq!(error.code, "json_invalid");
    assert_eq!(error.line, 2);
    assert!(error.column > 1);
    assert!(error.message.contains("value"));
}

#[test]
fn rejects_duplicate_keys_and_resource_limit_exhaustion() {
    let duplicate = require_error(parse_json(br#"{"a": 1, "a": 2}"#, JsonLimits::default()));
    assert_eq!(duplicate.code, "json_invalid");

    let limits = JsonLimits {
        max_input_bytes: 64,
        max_depth: 2,
        max_nodes: 8,
        max_string_bytes: 4,
        max_integer_digits: 32,
        max_collection_items: 8,
    };
    let limited = require_error(parse_json(br#"{"long": "value"}"#, limits));
    assert_eq!(limited.code, "input_limit_exceeded");
}

proptest! {
    #[test]
    fn arbitrary_json_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..8192)) {
        let result = std::panic::catch_unwind(|| parse_json(&bytes, JsonLimits {
            max_input_bytes: 8192,
            max_depth: 32,
            max_nodes: 4096,
            max_string_bytes: 65_536,
            max_integer_digits: 4_300,
            max_collection_items: 4_096,
        }));
        prop_assert!(result.is_ok());
    }
}
