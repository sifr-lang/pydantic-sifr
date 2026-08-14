use sifr_runtime::interop::structural::StaticProgramValue;

use super::*;
use crate::{InputProfile, JsonLimits, ValidationOptions, parse_json};

fn record(fields: Vec<(&'static str, StaticProgramValue)>) -> StaticProgramValue {
    StaticProgramValue::Record(Box::leak(fields.into_boxed_slice()))
}

fn list(values: Vec<StaticProgramValue>) -> StaticProgramValue {
    StaticProgramValue::List(Box::leak(values.into_boxed_slice()))
}

fn union_node(children: &[&'static str]) -> StaticProgramValue {
    union_node_with_metadata(children, Vec::new())
}

fn union_node_with_metadata(
    children: &[&'static str],
    metadata: Vec<StaticProgramValue>,
) -> StaticProgramValue {
    record(vec![
        ("kind", StaticProgramValue::String("union")),
        (
            "children",
            list(
                children
                    .iter()
                    .map(|index| StaticProgramValue::Integer(index))
                    .collect(),
            ),
        ),
        ("metadata", list(metadata)),
        ("error_code", StaticProgramValue::None),
        ("error_message", StaticProgramValue::None),
    ])
}

fn defined_union_node(name: &'static str, children: &[&'static str]) -> StaticProgramValue {
    record(vec![
        ("kind", StaticProgramValue::String("union")),
        (
            "children",
            list(
                children
                    .iter()
                    .map(|index| StaticProgramValue::Integer(index))
                    .collect(),
            ),
        ),
        ("metadata", list(Vec::new())),
        ("error_code", StaticProgramValue::None),
        ("error_message", StaticProgramValue::None),
        ("definition", StaticProgramValue::String(name)),
    ])
}

fn metadata(key: &'static str, value: &'static str) -> StaticProgramValue {
    record(vec![
        ("key", StaticProgramValue::String(key)),
        ("value", StaticProgramValue::String(value)),
    ])
}

fn mapping_node(key: &'static str, value: &'static str) -> StaticProgramValue {
    record(vec![
        ("kind", StaticProgramValue::String("dict")),
        (
            "children",
            list(vec![
                StaticProgramValue::Integer(key),
                StaticProgramValue::Integer(value),
            ]),
        ),
    ])
}

fn integer_node() -> StaticProgramValue {
    record(vec![
        ("kind", StaticProgramValue::String("int")),
        ("children", list(Vec::new())),
        ("integer_target", StaticProgramValue::String("int64")),
        ("integer_constraints", StaticProgramValue::None),
    ])
}

fn string_node() -> StaticProgramValue {
    record(vec![
        ("kind", StaticProgramValue::String("str")),
        ("children", list(Vec::new())),
        ("string_constraints", StaticProgramValue::None),
    ])
}

fn definition_reference_node(name: &'static str) -> StaticProgramValue {
    record(vec![
        ("kind", StaticProgramValue::String("definition-ref")),
        ("children", list(Vec::new())),
        ("reference", StaticProgramValue::String(name)),
    ])
}

fn defined_string_node(name: &'static str) -> StaticProgramValue {
    record(vec![
        ("kind", StaticProgramValue::String("str")),
        ("children", list(Vec::new())),
        ("string_constraints", StaticProgramValue::None),
        ("definition", StaticProgramValue::String(name)),
    ])
}

fn model_node(name: &'static str, field_node: &'static str) -> StaticProgramValue {
    let field = record(vec![
        ("name", StaticProgramValue::String("x")),
        ("node", StaticProgramValue::Integer(field_node)),
        ("validation_alias", list(Vec::new())),
        ("default", StaticProgramValue::None),
    ]);
    let model = record(vec![
        ("name", StaticProgramValue::String(name)),
        ("fields", list(vec![field])),
        ("extra", StaticProgramValue::String("ignore")),
        ("populate_by_name", StaticProgramValue::Bool(false)),
        ("location_by_alias", StaticProgramValue::Bool(true)),
    ]);
    record(vec![
        ("kind", StaticProgramValue::String("model")),
        ("children", list(Vec::new())),
        ("model", model),
    ])
}

fn schema_program(nodes: Vec<StaticProgramValue>) -> &'static StaticProgramValue {
    Box::leak(Box::new(record(vec![
        ("nodes", list(nodes)),
        ("root", StaticProgramValue::Integer("0")),
    ])))
}

fn selected_union_index(program: &'static StaticProgramValue, payload: &[u8]) -> usize {
    let schema = SchemaRef::from_static_program(program)
        .unwrap_or_else(|error| panic!("static schema failed: {error}"));
    let input = parse_json(payload, JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON input failed: {error}"));
    let output = super::super::validate_ref(
        schema,
        &input,
        ValidationOptions {
            profile: InputProfile::Json,
            ..ValidationOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("static validation failed: {error}"));
    let Some(ValidatedValue::Union(value)) = output.get(output.root()) else {
        panic!("expected a union output");
    };
    value.index()
}

#[test]
fn static_mapping_union_ranks_recursive_value_exactness() {
    let program = schema_program(vec![
        union_node(&["1", "4"]),
        mapping_node("2", "3"),
        string_node(),
        integer_node(),
        mapping_node("5", "6"),
        string_node(),
        string_node(),
    ]);

    assert_eq!(selected_union_index(program, br#"{"a":"1"}"#), 1);
}

#[test]
fn static_model_union_ranks_recursive_field_exactness() {
    let program = schema_program(vec![
        union_node(&["1", "3"]),
        model_node("tests.A", "2"),
        integer_node(),
        model_node("tests.B", "4"),
        string_node(),
    ]);

    assert_eq!(selected_union_index(program, br#"{"x":"1"}"#), 1);
}

#[test]
fn static_left_to_right_union_keeps_the_first_successful_branch() {
    let program = schema_program(vec![
        union_node_with_metadata(
            &["1", "4"],
            vec![metadata("pydantic.union.mode", "left_to_right")],
        ),
        mapping_node("2", "3"),
        string_node(),
        integer_node(),
        mapping_node("5", "6"),
        string_node(),
        string_node(),
    ]);

    assert_eq!(selected_union_index(program, br#"{"a":"1"}"#), 0);
}

#[test]
fn static_smart_union_ranks_a_reference_by_its_target_exactness() {
    let program = schema_program(vec![
        union_node(&["1", "2"]),
        integer_node(),
        definition_reference_node("shared.text"),
        defined_string_node("shared.text"),
    ]);

    assert_eq!(selected_union_index(program, br#""1""#), 1);
}

#[test]
fn static_smart_union_ranks_a_mapping_with_a_referenced_string_key() {
    let program = schema_program(vec![
        union_node(&["1", "4"]),
        mapping_node("2", "3"),
        integer_node(),
        integer_node(),
        mapping_node("5", "6"),
        definition_reference_node("shared.text"),
        integer_node(),
        defined_string_node("shared.text"),
    ]);

    assert_eq!(selected_union_index(program, br#"{"1":2}"#), 1);
}

#[test]
fn static_definition_reference_rejects_a_union_target() {
    let program = schema_program(vec![
        definition_reference_node("shared.sum"),
        defined_union_node("shared.sum", &["2", "3"]),
        integer_node(),
        string_node(),
    ]);
    let schema = SchemaRef::from_static_program(program)
        .unwrap_or_else(|error| panic!("static schema failed: {error}"));
    let input = parse_json(b"1", JsonLimits::default())
        .unwrap_or_else(|error| panic!("JSON input failed: {error}"));
    let error = match super::super::validate_ref(
        schema,
        &input,
        ValidationOptions {
            profile: InputProfile::Json,
            ..ValidationOptions::default()
        },
    ) {
        Ok(_) => panic!("expected static reference rejection"),
        Err(error) => error,
    };
    assert_eq!(error.details()[0].code, "schema_invalid");
}
