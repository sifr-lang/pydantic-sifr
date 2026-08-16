use std::collections::HashMap;
use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use pydantic_sifr_core::{
    CollectionConstraints, IntegerConstraints, IntegerTarget, JsonIntegerProfile, JsonLimits,
    PreparedSchema, Schema, SerializationOptions, SerializationPlan, ValidationOptions, parse_json,
    serialize_json, validate, validate_and_construct,
};

type RepresentativeModel = HashMap<String, Vec<i64>>;

const REPRESENTATIVE_JSON: &[u8] = br#"{"alpha":[0,1,2,3,4,5,6,7],"beta":[8,9,10,11,12,13,14,15]}"#;

fn representative_schema() -> Schema {
    Schema::Mapping {
        key: Box::new(Schema::String(Default::default())),
        value: Box::new(Schema::List {
            item: Box::new(Schema::Integer {
                target: IntegerTarget::I64,
                constraints: IntegerConstraints::default(),
            }),
            constraints: CollectionConstraints::default(),
        }),
        constraints: CollectionConstraints::default(),
    }
}

fn representative_model() -> RepresentativeModel {
    HashMap::from([
        ("alpha".to_owned(), (0_i64..8_i64).collect()),
        ("beta".to_owned(), (8_i64..16_i64).collect()),
    ])
}

fn benchmark_foundations(criterion: &mut Criterion) {
    let schema = representative_schema();
    let prepared = PreparedSchema::new(&schema)
        .unwrap_or_else(|error| panic!("schema preparation failed: {error}"));
    let input = parse_json(REPRESENTATIVE_JSON, JsonLimits::default())
        .unwrap_or_else(|error| panic!("representative JSON failed to parse: {error}"));
    let plan = SerializationPlan::from_prepared(prepared, JsonIntegerProfile::Exact)
        .unwrap_or_else(|error| panic!("serializer plan failed: {error}"));
    let model = representative_model();

    criterion.bench_function("parse/representative_json", |bencher| {
        bencher.iter(|| parse_json(black_box(REPRESENTATIVE_JSON), JsonLimits::default()))
    });
    criterion.bench_function("validate/representative_model", |bencher| {
        bencher.iter(|| validate(&schema, black_box(&input), ValidationOptions::default()))
    });
    criterion.bench_function("construct/representative_model", |bencher| {
        bencher.iter(|| {
            validate_and_construct::<RepresentativeModel>(
                &prepared,
                black_box(&input),
                ValidationOptions::default(),
            )
        })
    });
    criterion.bench_function("serialize/representative_model", |bencher| {
        bencher.iter(|| serialize_json(&plan, black_box(&model), &SerializationOptions::default()))
    });
}

criterion_group!(foundations, benchmark_foundations);
criterion_main!(foundations);
