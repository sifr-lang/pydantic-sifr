use num_bigint::BigInt;
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
        &NativeValue::Sequence(vec![
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
    let ValidatedValue::Sequence(items) = root(&output) else {
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
