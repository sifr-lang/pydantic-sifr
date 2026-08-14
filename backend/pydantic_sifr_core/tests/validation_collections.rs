use num_bigint::BigInt;
use proptest::prelude::*;
use pydantic_sifr_core::{
    CollectionConstraints, InputProfile, JsonLimits, LocationItem, NativeValue, Schema,
    StringConstraints, ValidatedArena, ValidatedValue, ValidationError, ValidationLimits,
    ValidationOptions, build_native_input, parse_json, validate, validated_iterator,
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

#[test]
fn list_aggregates_stable_index_errors_and_honors_the_error_cap() {
    let input = build_native_input(
        &NativeValue::List(vec![
            NativeValue::String("bad".to_owned()),
            NativeValue::String("also-bad".to_owned()),
            NativeValue::String("still-bad".to_owned()),
        ]),
        JsonLimits::default(),
    )
    .unwrap_or_else(|error| panic!("native input failed: {error}"));
    let schema = Schema::List {
        item: Box::new(Schema::exact_integer()),
        constraints: CollectionConstraints::default(),
    };
    let error = require_error(validate(
        &schema,
        &input,
        ValidationOptions {
            strict: true,
            limits: ValidationLimits {
                max_errors: 2,
                ..ValidationLimits::default()
            },
            ..ValidationOptions::default()
        },
    ));
    assert_eq!(error.details().len(), 2);
    assert!(error.is_truncated());
    assert_eq!(error.details()[0].location, vec![LocationItem::Index(0)]);
    assert_eq!(error.details()[1].location, vec![LocationItem::Index(1)]);
}

#[test]
fn aggregation_with_default_limit_and_nested_multi_errors_never_panics() {
    let input = parse_json(b"[[1,2],[3,4]]", JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON input failed: {error}"));
    let schema = Schema::List {
        item: Box::new(Schema::List {
            item: Box::new(Schema::String(StringConstraints::default())),
            constraints: CollectionConstraints::default(),
        }),
        constraints: CollectionConstraints::default(),
    };
    let error = require_error(validate(&schema, &input, ValidationOptions::default()));
    assert_eq!(error.details().len(), 4);
    assert!(!error.is_truncated());
    assert_eq!(
        error.details()[3].location,
        vec![LocationItem::Index(1), LocationItem::Index(1)]
    );
}

#[test]
fn strict_native_collection_kinds_do_not_coerce() {
    let input = build_native_input(
        &NativeValue::List(vec![NativeValue::Integer("1".to_owned())]),
        JsonLimits::default(),
    )
    .unwrap_or_else(|error| panic!("native input failed: {error}"));
    let tuple = Schema::Tuple(vec![Schema::exact_integer()]);
    let error = require_error(validate(
        &tuple,
        &input,
        ValidationOptions {
            strict: true,
            ..ValidationOptions::default()
        },
    ));
    assert_eq!(error.details()[0].code, "collection_type");

    let lax = validate(&tuple, &input, ValidationOptions::default())
        .unwrap_or_else(|error| panic!("lax tuple conversion failed: {error}"));
    assert!(matches!(root(&lax), ValidatedValue::Tuple(_)));
}

#[test]
fn strict_mapping_accepts_native_mapping_and_json_object_only_in_their_profiles() {
    let schema = Schema::Mapping {
        key: Box::new(Schema::String(StringConstraints::default())),
        value: Box::new(Schema::exact_integer()),
        constraints: CollectionConstraints::default(),
    };
    let native = build_native_input(
        &NativeValue::Mapping(vec![(
            NativeValue::String("one".to_owned()),
            NativeValue::Integer("1".to_owned()),
        )]),
        JsonLimits::default(),
    )
    .unwrap_or_else(|error| panic!("native mapping failed: {error}"));
    let native_output = validate(
        &schema,
        &native,
        ValidationOptions {
            strict: true,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("strict native mapping failed: {error}"));
    assert!(matches!(root(&native_output), ValidatedValue::Mapping(_)));

    let json = parse_json(br#"{"one":1}"#, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON object failed: {error}"));
    let mislabeled = require_error(validate(
        &schema,
        &json,
        ValidationOptions {
            strict: true,
            profile: InputProfile::Native,
            ..ValidationOptions::default()
        },
    ));
    assert_eq!(mislabeled.details()[0].code, "mapping_type");

    let json_output = validate(
        &schema,
        &json,
        ValidationOptions {
            strict: true,
            profile: InputProfile::Json,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("strict JSON mapping failed: {error}"));
    assert!(matches!(root(&json_output), ValidatedValue::Mapping(_)));

    let object = build_native_input(
        &NativeValue::Object(vec![(
            "one".to_owned(),
            NativeValue::Integer("1".to_owned()),
        )]),
        JsonLimits::default(),
    )
    .unwrap_or_else(|error| panic!("native object failed: {error}"));
    let object_error = require_error(validate(
        &schema,
        &object,
        ValidationOptions {
            strict: true,
            ..ValidationOptions::default()
        },
    ));
    assert_eq!(object_error.details()[0].code, "mapping_type");

    let lax_object = validate(&schema, &object, ValidationOptions::default())
        .unwrap_or_else(|error| panic!("lax structural object conversion failed: {error}"));
    assert!(matches!(root(&lax_object), ValidatedValue::Mapping(_)));
}

#[test]
fn strict_json_mapping_converts_typed_object_keys() {
    let input = parse_json(br#"{"1":2}"#, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON object failed: {error}"));
    let output = validate(
        &Schema::Mapping {
            key: Box::new(Schema::exact_integer()),
            value: Box::new(Schema::exact_integer()),
            constraints: CollectionConstraints::default(),
        },
        &input,
        ValidationOptions {
            strict: true,
            profile: InputProfile::Json,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("strict JSON typed keys failed: {error}"));
    let ValidatedValue::Mapping(entries) = root(&output) else {
        panic!("expected typed mapping");
    };
    let key = entries
        .first()
        .map(|entry| entry.0)
        .unwrap_or_else(|| panic!("typed mapping key must exist"));
    assert_eq!(
        output.get(key),
        Some(&ValidatedValue::ExactInt(BigInt::from(1)))
    );
}

#[test]
fn tuple_validates_each_declared_position() {
    let input = parse_json(br#"[1,"two"]"#, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON input failed: {error}"));
    let schema = Schema::Tuple(vec![
        Schema::exact_integer(),
        Schema::String(StringConstraints::default()),
    ]);
    let output = validate(
        &schema,
        &input,
        ValidationOptions {
            strict: true,
            profile: InputProfile::Json,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("tuple failed: {error}"));
    let ValidatedValue::Tuple(items) = root(&output) else {
        panic!("expected validated tuple sequence");
    };
    assert_eq!(items.len(), 2);
    assert_eq!(
        output.get(items[0]),
        Some(&ValidatedValue::ExactInt(BigInt::from(1)))
    );
    assert_eq!(
        output.get(items[1]),
        Some(&ValidatedValue::String("two".to_owned()))
    );
}

#[test]
fn mapping_validates_object_keys_and_values_into_one_arena() {
    let input = parse_json(br#"{"one":1,"two":2}"#, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON input failed: {error}"));
    let schema = Schema::Mapping {
        key: Box::new(Schema::String(StringConstraints::default())),
        value: Box::new(Schema::exact_integer()),
        constraints: CollectionConstraints::default(),
    };
    let output = validate(
        &schema,
        &input,
        ValidationOptions {
            strict: true,
            profile: InputProfile::Json,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("mapping failed: {error}"));
    let ValidatedValue::Mapping(entries) = root(&output) else {
        panic!("expected validated mapping");
    };
    assert_eq!(entries.len(), 2);
    assert_eq!(
        output.get(entries[0].0),
        Some(&ValidatedValue::String("one".to_owned()))
    );
    assert_eq!(
        output.get(entries[1].1),
        Some(&ValidatedValue::ExactInt(BigInt::from(2)))
    );
}

#[test]
fn duplicate_mapping_keys_and_unhashable_set_items_have_stable_locations() {
    let mapping = build_native_input(
        &NativeValue::Mapping(vec![
            (
                NativeValue::Integer("1".to_owned()),
                NativeValue::String("first".to_owned()),
            ),
            (
                NativeValue::String("1".to_owned()),
                NativeValue::String("second".to_owned()),
            ),
        ]),
        JsonLimits::default(),
    )
    .unwrap_or_else(|error| panic!("native mapping failed: {error}"));
    let mapping_error = require_error(validate(
        &Schema::Mapping {
            key: Box::new(Schema::exact_integer()),
            value: Box::new(Schema::String(StringConstraints::default())),
            constraints: CollectionConstraints::default(),
        },
        &mapping,
        ValidationOptions::default(),
    ));
    assert_eq!(mapping_error.details()[0].code, "mapping_key_duplicate");
    assert_eq!(
        mapping_error.details()[0].location,
        vec![LocationItem::MappingKey(1)]
    );

    let nested = parse_json(b"[[1],[2]]", JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON set input failed: {error}"));
    let set_error = require_error(validate(
        &Schema::Set {
            item: Box::new(Schema::List {
                item: Box::new(Schema::exact_integer()),
                constraints: CollectionConstraints::default(),
            }),
            constraints: CollectionConstraints::default(),
        },
        &nested,
        ValidationOptions::default(),
    ));
    assert_eq!(set_error.details().len(), 2);
    assert_eq!(set_error.details()[0].code, "set_item_unhashable");
    assert_eq!(
        set_error.details()[0].location,
        vec![LocationItem::Index(0)]
    );
    assert_eq!(
        set_error.details()[1].location,
        vec![LocationItem::Index(1)]
    );
}

#[test]
fn set_and_frozenset_deduplicate_after_item_validation() {
    let input = parse_json(b"[1,1,2]", JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON input failed: {error}"));
    for frozen in [false, true] {
        let schema = if frozen {
            Schema::FrozenSet {
                item: Box::new(Schema::exact_integer()),
                constraints: CollectionConstraints::default(),
            }
        } else {
            Schema::Set {
                item: Box::new(Schema::exact_integer()),
                constraints: CollectionConstraints::default(),
            }
        };
        let output = validate(&schema, &input, ValidationOptions::default())
            .unwrap_or_else(|error| panic!("set failed: {error}"));
        let length = match root(&output) {
            ValidatedValue::Set(items) | ValidatedValue::FrozenSet(items) => items.len(),
            _ => panic!("expected validated set"),
        };
        assert_eq!(length, 2);
    }
}

#[test]
fn embedded_json_reuses_the_json_profile_and_imports_the_normalized_tree() {
    let input = build_native_input(
        &NativeValue::String("[1,2]".to_owned()),
        JsonLimits::default(),
    )
    .unwrap_or_else(|error| panic!("native input failed: {error}"));
    let schema = Schema::EmbeddedJson(Box::new(Schema::List {
        item: Box::new(Schema::exact_integer()),
        constraints: CollectionConstraints::default(),
    }));
    let output = validate(&schema, &input, ValidationOptions::default())
        .unwrap_or_else(|error| panic!("embedded JSON failed: {error}"));
    let ValidatedValue::Sequence(items) = root(&output) else {
        panic!("expected imported JSON sequence");
    };
    assert_eq!(items.len(), 2);
    assert_eq!(
        output.get(items[1]),
        Some(&ValidatedValue::ExactInt(BigInt::from(2)))
    );
}

#[test]
fn embedded_composite_values_remap_ids_after_existing_mapping_keys() {
    let input = build_native_input(
        &NativeValue::Mapping(vec![(
            NativeValue::String("items".to_owned()),
            NativeValue::String("[[1,2]]".to_owned()),
        )]),
        JsonLimits::default(),
    )
    .unwrap_or_else(|error| panic!("native input failed: {error}"));
    let schema = Schema::Mapping {
        key: Box::new(Schema::String(StringConstraints::default())),
        value: Box::new(Schema::EmbeddedJson(Box::new(Schema::List {
            item: Box::new(Schema::List {
                item: Box::new(Schema::exact_integer()),
                constraints: CollectionConstraints::default(),
            }),
            constraints: CollectionConstraints::default(),
        }))),
        constraints: CollectionConstraints::default(),
    };
    let output = validate(&schema, &input, ValidationOptions::default())
        .unwrap_or_else(|error| panic!("mapping with embedded JSON failed: {error}"));
    let ValidatedValue::Mapping(entries) = root(&output) else {
        panic!("expected mapping root");
    };
    let value_id = entries
        .first()
        .map(|entry| entry.1)
        .unwrap_or_else(|| panic!("mapping entry must exist"));
    let ValidatedValue::Sequence(outer) = output
        .get(value_id)
        .unwrap_or_else(|| panic!("embedded outer list must exist"))
    else {
        panic!("expected embedded outer list");
    };
    let inner_id = outer
        .first()
        .copied()
        .unwrap_or_else(|| panic!("embedded inner list must exist"));
    let ValidatedValue::Sequence(inner) = output
        .get(inner_id)
        .unwrap_or_else(|| panic!("embedded inner value must resolve"))
    else {
        panic!("expected embedded inner list");
    };
    assert_eq!(inner.len(), 2);
    assert_eq!(
        output.get(inner[1]),
        Some(&ValidatedValue::ExactInt(BigInt::from(2)))
    );
}

#[test]
fn nested_embedded_json_shares_one_depth_budget() {
    let mut schema = Schema::exact_integer();
    let mut source = "1".to_owned();
    for _ in 0..5 {
        schema = Schema::EmbeddedJson(Box::new(schema));
        source = serde_json::to_string(&source)
            .unwrap_or_else(|error| panic!("test JSON encoding failed: {error}"));
    }
    let input = build_native_input(&NativeValue::String(source), JsonLimits::default())
        .unwrap_or_else(|error| panic!("native input failed: {error}"));
    let error = require_error(validate(
        &schema,
        &input,
        ValidationOptions {
            limits: ValidationLimits {
                max_depth: 3,
                ..ValidationLimits::default()
            },
            ..ValidationOptions::default()
        },
    ));
    assert_eq!(error.details()[0].code, "recursion_limit");
}

#[test]
fn validated_iterator_defers_item_validation_until_consumption() {
    let input = parse_json(br#"[1,"bad",3]"#, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON input failed: {error}"));
    let schema = Schema::Generator {
        item: Box::new(Schema::exact_integer()),
        constraints: CollectionConstraints::default(),
    };
    let mut iterator = validated_iterator(
        &schema,
        &input,
        ValidationOptions {
            strict: true,
            profile: InputProfile::Json,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("iterator creation failed: {error}"));

    let first = iterator
        .next()
        .unwrap_or_else(|| panic!("first item must exist"))
        .unwrap_or_else(|error| panic!("first item failed: {error}"));
    assert_eq!(root(&first), &ValidatedValue::ExactInt(BigInt::from(1)));

    let second = match iterator.next() {
        Some(Err(error)) => error,
        Some(Ok(_)) => panic!("second item must fail"),
        None => panic!("second item must exist"),
    };
    assert_eq!(second.details()[0].location, vec![LocationItem::Index(1)]);

    let third = iterator
        .next()
        .unwrap_or_else(|| panic!("third item must exist"))
        .unwrap_or_else(|error| panic!("third item failed: {error}"));
    assert_eq!(root(&third), &ValidatedValue::ExactInt(BigInt::from(3)));
    assert!(iterator.next().is_none());
}

#[test]
fn validated_iterator_relaxes_only_the_generator_container_kind() {
    let input = build_native_input(
        &NativeValue::Tuple(vec![NativeValue::Integer("1".to_owned())]),
        JsonLimits::default(),
    )
    .unwrap_or_else(|error| panic!("native input failed: {error}"));
    let schema = Schema::Generator {
        item: Box::new(Schema::exact_integer()),
        constraints: CollectionConstraints::default(),
    };
    let mut iterator = validated_iterator(
        &schema,
        &input,
        ValidationOptions {
            strict: false,
            strict_override: Some(true),
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("iterator creation failed: {error}"));

    let item = iterator
        .next()
        .unwrap_or_else(|| panic!("item must exist"))
        .unwrap_or_else(|error| panic!("item validation failed: {error}"));
    assert_eq!(root(&item), &ValidatedValue::ExactInt(BigInt::from(1)));
    assert!(iterator.next().is_none());
}

proptest! {
    #[test]
    fn arbitrary_collection_json_never_panics(
        bytes in proptest::collection::vec(any::<u8>(), 0..16_384),
        selector in any::<u8>(),
    ) {
        let Ok(input) = parse_json(&bytes, JsonLimits {
            max_input_bytes: 16_384,
            max_depth: 32,
            max_nodes: 2_048,
            max_string_bytes: 16_384,
            max_integer_digits: 1_024,
            max_collection_items: 2_048,
        }) else {
            return Ok(());
        };
        let constraints = CollectionConstraints {
            min_length: Some(0),
            max_length: Some(2_048),
        };
        let schema = match selector % 5 {
            0 => Schema::List {
                item: Box::new(Schema::exact_integer()),
                constraints,
            },
            1 => Schema::Tuple(vec![Schema::exact_integer(), Schema::Bool]),
            2 => Schema::Mapping {
                key: Box::new(Schema::String(StringConstraints::default())),
                value: Box::new(Schema::exact_integer()),
                constraints,
            },
            3 => Schema::Set {
                item: Box::new(Schema::exact_integer()),
                constraints,
            },
            _ => Schema::FrozenSet {
                item: Box::new(Schema::String(StringConstraints::default())),
                constraints,
            },
        };
        let result = std::panic::catch_unwind(|| {
            validate(
                &schema,
                &input,
                ValidationOptions {
                    profile: InputProfile::Json,
                    limits: ValidationLimits {
                        max_depth: 32,
                        max_collection_items: 2_048,
                        max_string_bytes: 16_384,
                        max_numeric_digits: 1_024,
                        max_decimal_exponent: 1_024,
                        max_errors: 32,
                    },
                    ..ValidationOptions::default()
                },
            )
        });
        prop_assert!(result.is_ok());
    }
}
