use pydantic_sifr_core::{
    InputValue, JsonLimits, NativeInputError, SequenceKind, project_structural_input,
};

#[test]
fn structural_projection_builds_nested_input_without_an_intermediate_model() {
    let source = vec![(7_i64, Some("Ada".to_owned())), (8_i64, None::<String>)];
    let Ok(input) = project_structural_input(&source, JsonLimits::default()) else {
        panic!("the bounded structural input must project");
    };

    let Some(root) = input.get(input.root()) else {
        panic!("the projected root must exist");
    };
    let InputValue::Sequence {
        kind: SequenceKind::List,
        items,
    } = root
    else {
        panic!("the projected root must be a list");
    };
    assert_eq!(items.len(), 2);

    let Some(first_value) = input.get(items[0]) else {
        panic!("the first tuple must exist");
    };
    let InputValue::Sequence {
        kind: SequenceKind::Tuple,
        items: first,
    } = first_value
    else {
        panic!("the first list item must be a tuple");
    };
    assert!(matches!(input.get(first[0]), Some(InputValue::Integer(value)) if value == "7"));
    assert!(matches!(input.get(first[1]), Some(InputValue::String(value)) if value == "Ada"));

    let Some(second_value) = input.get(items[1]) else {
        panic!("the second tuple must exist");
    };
    let InputValue::Sequence {
        kind: SequenceKind::Tuple,
        items: second,
    } = second_value
    else {
        panic!("the second list item must be a tuple");
    };
    assert!(matches!(input.get(second[0]), Some(InputValue::Integer(value)) if value == "8"));
    assert!(matches!(input.get(second[1]), Some(InputValue::Null)));
}

#[test]
fn structural_projection_enforces_limits_before_aggregate_allocation() {
    let mut limits = JsonLimits {
        max_collection_items: 1,
        ..JsonLimits::default()
    };
    let collection = project_structural_input(&vec![1_i64, 2_i64], limits);
    assert_eq!(
        collection,
        Err(NativeInputError::Limit("maximum collection items"))
    );

    limits.max_collection_items = JsonLimits::default().max_collection_items;
    limits.max_nodes = 1;
    let nodes = project_structural_input(&vec![1_i64], limits);
    assert_eq!(nodes, Err(NativeInputError::Limit("maximum node count")));

    limits.max_nodes = JsonLimits::default().max_nodes;
    limits.max_integer_digits = 1;
    let integer = project_structural_input(&12_i64, limits);
    assert_eq!(
        integer,
        Err(NativeInputError::Limit("maximum integer digits"))
    );
}

#[test]
fn structural_projection_uses_the_same_root_depth_contract_as_json() {
    let limits = JsonLimits {
        max_depth: 1,
        ..JsonLimits::default()
    };
    assert!(project_structural_input(&vec![vec![1_i64]], limits).is_ok());
    assert_eq!(
        project_structural_input(&vec![vec![vec![1_i64]]], limits),
        Err(NativeInputError::Limit("maximum depth"))
    );
}

#[test]
fn unordered_structural_collections_have_canonical_projected_order() {
    let first_set = HashSet::from(["gamma".to_owned(), "alpha".to_owned(), "beta".to_owned()]);
    let second_set = HashSet::from(["beta".to_owned(), "gamma".to_owned(), "alpha".to_owned()]);
    let Ok(first_set_input) = project_structural_input(&first_set, JsonLimits::default()) else {
        panic!("the first set must project");
    };
    let Ok(second_set_input) = project_structural_input(&second_set, JsonLimits::default()) else {
        panic!("the second set must project");
    };
    assert_eq!(
        root_sequence_strings(&first_set_input),
        root_sequence_strings(&second_set_input)
    );

    let first_map = HashMap::from([
        ("gamma".to_owned(), 3_i64),
        ("alpha".to_owned(), 1_i64),
        ("beta".to_owned(), 2_i64),
    ]);
    let second_map = HashMap::from([
        ("beta".to_owned(), 2_i64),
        ("gamma".to_owned(), 3_i64),
        ("alpha".to_owned(), 1_i64),
    ]);
    let Ok(first_map_input) = project_structural_input(&first_map, JsonLimits::default()) else {
        panic!("the first mapping must project");
    };
    let Ok(second_map_input) = project_structural_input(&second_map, JsonLimits::default()) else {
        panic!("the second mapping must project");
    };
    assert_eq!(
        root_mapping_keys(&first_map_input),
        root_mapping_keys(&second_map_input)
    );
}

fn root_sequence_strings(input: &pydantic_sifr_core::InputArena) -> Vec<String> {
    let Some(InputValue::Sequence { items, .. }) = input.get(input.root()) else {
        panic!("the root must be a sequence");
    };
    items
        .iter()
        .map(|id| match input.get(*id) {
            Some(InputValue::String(value)) => value.clone(),
            _ => panic!("the sequence child must be a string"),
        })
        .collect()
}

fn root_mapping_keys(input: &pydantic_sifr_core::InputArena) -> Vec<String> {
    let Some(InputValue::Mapping(entries)) = input.get(input.root()) else {
        panic!("the root must be a mapping");
    };
    entries
        .iter()
        .map(|(id, _)| match input.get(*id) {
            Some(InputValue::String(value)) => value.clone(),
            _ => panic!("the mapping key must be a string"),
        })
        .collect()
}
use std::collections::{HashMap, HashSet};
