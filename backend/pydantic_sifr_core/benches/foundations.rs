use criterion::{Criterion, criterion_group, criterion_main};
use pydantic_sifr_core::{JsonLimits, parse_json};

fn parse_representative_json(criterion: &mut Criterion) {
    let input = br#"{"id":123456789012345678901234567890,"name":"Ada","active":true,"tags":["core","native"]}"#;
    criterion.bench_function("json_foundation/representative", |bencher| {
        bencher.iter(|| parse_json(input, JsonLimits::default()))
    });
}

criterion_group!(foundations, parse_representative_json);
criterion_main!(foundations);
