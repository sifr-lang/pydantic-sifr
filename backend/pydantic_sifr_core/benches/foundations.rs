use std::collections::HashMap;
use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use pydantic_sifr_core::{
    CollectionConstraints, IntegerConstraints, IntegerTarget, JsonIntegerProfile, JsonLimits,
    PreparedSchema, Schema, SerializationOptions, SerializationPlan, parse_json, serialize_json,
    serialize_structural,
};

fn parse_representative_json(criterion: &mut Criterion) {
    let input = br#"{"id":123456789012345678901234567890,"name":"Ada","active":true,"tags":["core","native"]}"#;
    criterion.bench_function("json_foundation/representative", |bencher| {
        bencher.iter(|| parse_json(input, JsonLimits::default()))
    });
}

fn serialize_representative_model(criterion: &mut Criterion) {
    let schema = Schema::Mapping {
        key: Box::new(Schema::String(Default::default())),
        value: Box::new(Schema::List {
            item: Box::new(Schema::Integer {
                target: IntegerTarget::I64,
                constraints: IntegerConstraints::default(),
            }),
            constraints: CollectionConstraints::default(),
        }),
        constraints: CollectionConstraints::default(),
    };
    let prepared = PreparedSchema::new(&schema)
        .unwrap_or_else(|error| panic!("schema preparation failed: {error}"));
    let plan = SerializationPlan::from_prepared(prepared, JsonIntegerProfile::Exact)
        .unwrap_or_else(|error| panic!("serializer plan failed: {error}"));
    let value = HashMap::from([
        ("alpha".to_owned(), (0_i64..32_i64).collect::<Vec<_>>()),
        ("beta".to_owned(), (32_i64..64_i64).collect::<Vec<_>>()),
    ]);
    let options = SerializationOptions::default();

    criterion.bench_function("serialization/streaming_json", |bencher| {
        bencher.iter(|| black_box(serialize_json(&plan, &value, &options)))
    });
    criterion.bench_function("serialization/structural_output", |bencher| {
        bencher.iter(|| black_box(serialize_structural(&plan, &value, &options)))
    });
}

criterion_group!(
    foundations,
    parse_representative_json,
    serialize_representative_model
);
criterion_main!(foundations);
