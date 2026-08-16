use std::collections::{BTreeMap, HashMap, HashSet};

use pydantic_sifr_core::{
    CollectionConstraints, IntegerConstraints, IntegerTarget, JsonIntegerProfile, NativeValue,
    PreparedSchema, Schema, SerializationOptions, SerializationPlan, serialize_json,
    serialize_structural,
};

#[test]
fn scalar_and_nullable_fixtures_use_declared_json_representations() {
    assert_json(&Schema::Bool, &true, b"true");
    assert_json(
        &Schema::String(Default::default()),
        &"typed".to_owned(),
        br#""typed""#,
    );
    assert_json(
        &Schema::Nullable(Box::new(integer_schema())),
        &Some(-42_i64),
        b"-42",
    );
    assert_json(
        &Schema::Nullable(Box::new(integer_schema())),
        &None::<i64>,
        b"null",
    );
}

#[test]
fn sequence_tuple_set_and_mapping_fixtures_stream_without_a_json_tree() {
    let list_schema = Schema::List {
        item: Box::new(integer_schema()),
        constraints: CollectionConstraints::default(),
    };
    assert_json(&list_schema, &vec![1_i64, 2_i64, 3_i64], b"[1,2,3]");

    let tuple_schema = Schema::Tuple(vec![Schema::String(Default::default()), integer_schema()]);
    assert_json(
        &tuple_schema,
        &("answer".to_owned(), 42_i64),
        br#"["answer",42]"#,
    );

    let set_schema = Schema::Set {
        item: Box::new(integer_schema()),
        constraints: CollectionConstraints::default(),
    };
    let set = HashSet::from([3_i64, 1_i64, 2_i64]);
    let mut set_output: Vec<i64> = serde_json::from_slice(&json(&set_schema, &set))
        .unwrap_or_else(|error| panic!("set JSON decode failed: {error}"));
    set_output.sort_unstable();
    assert_eq!(set_output, vec![1_i64, 2_i64, 3_i64]);

    let mapping_schema = Schema::Mapping {
        key: Box::new(Schema::String(Default::default())),
        value: Box::new(integer_schema()),
        constraints: CollectionConstraints::default(),
    };
    let mapping = HashMap::from([("left".to_owned(), 1_i64), ("right".to_owned(), 2_i64)]);
    let mapping_output: BTreeMap<String, i64> =
        serde_json::from_slice(&json(&mapping_schema, &mapping))
            .unwrap_or_else(|error| panic!("mapping JSON decode failed: {error}"));
    assert_eq!(
        mapping_output,
        BTreeMap::from([("left".to_owned(), 1_i64), ("right".to_owned(), 2_i64)])
    );
}

#[test]
fn structural_and_json_outputs_share_the_same_container_projection() {
    let schema = Schema::List {
        item: Box::new(integer_schema()),
        constraints: CollectionConstraints::default(),
    };
    let plan = plan(&schema);
    let value = vec![4_i64, 5_i64];

    let structural = serialize_structural(&plan, &value, &SerializationOptions::default())
        .unwrap_or_else(|error| panic!("structural serialization failed: {error}"));
    let json = serialize_json(&plan, &value, &SerializationOptions::default())
        .unwrap_or_else(|error| panic!("JSON serialization failed: {error}"));

    assert_eq!(
        structural,
        NativeValue::List(vec![
            NativeValue::Integer("4".to_owned()),
            NativeValue::Integer("5".to_owned()),
        ])
    );
    assert_eq!(json, b"[4,5]");
}

fn assert_json<T: sifr_runtime::interop::structural::StructuralProject>(
    schema: &Schema,
    value: &T,
    expected: &[u8],
) {
    let actual = json(schema, value);
    assert_eq!(actual, expected);
}

fn json<T: sifr_runtime::interop::structural::StructuralProject>(
    schema: &Schema,
    value: &T,
) -> Vec<u8> {
    let plan = plan(schema);
    serialize_json(&plan, value, &SerializationOptions::default())
        .unwrap_or_else(|error| panic!("JSON serialization failed: {error}"))
}

fn plan(schema: &Schema) -> SerializationPlan {
    let prepared = PreparedSchema::new(schema)
        .unwrap_or_else(|error| panic!("schema preparation failed: {error}"));
    SerializationPlan::from_prepared(prepared, JsonIntegerProfile::Exact)
        .unwrap_or_else(|error| panic!("serializer plan failed: {error}"))
}

fn integer_schema() -> Schema {
    Schema::Integer {
        target: IntegerTarget::I64,
        constraints: IntegerConstraints::default(),
    }
}
