use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use index::{BackendConfig, IndexConfig, IndexRecord, UfpIndex, INDEX_SCHEMA_VERSION};
use ndarray::Array1;
use serde_json::json;

fn sample_record(id: usize) -> IndexRecord {
    IndexRecord {
        schema_version: INDEX_SCHEMA_VERSION,
        canonical_hash: format!("hash-{id}"),
        perceptual: Some(vec![id as u64, (id + 1) as u64, (id + 2) as u64]),
        embedding: Some(vec![1, 2, 3, 4]),
        metadata: json!({"id": id}),
    }
}

fn bench_index(c: &mut Criterion) {
    let config = IndexConfig::new().with_backend(BackendConfig::in_memory());
    let index = UfpIndex::new(config).expect("index");

    let mut group = c.benchmark_group("index");

    group.bench_function("upsert_single", |b| {
        let rec = sample_record(1);
        b.iter(|| index.upsert(black_box(&rec)).expect("upsert"))
    });

    for size in [10, 100, 1000].iter() {
        let records: Vec<IndexRecord> = (0..*size).map(sample_record).collect();
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_function(format!("batch_{size}"), |b| {
            b.iter(|| index.batch_insert(black_box(&records)).expect("batch"))
        });
    }

    let vec = Array1::from_vec(vec![0.5; 384]);
    group.bench_function("quantize", |b| {
        b.iter(|| UfpIndex::quantize(black_box(&vec), black_box(127.0)))
    });

    group.finish();
}

criterion_group!(benches, bench_index);
criterion_main!(benches);
