use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use index::{BackendConfig, IndexConfig, IndexRecord, QueryMode, UfpIndex, INDEX_SCHEMA_VERSION};
use serde_json::json;

fn bench_index_upserts(c: &mut Criterion) {
    let records = build_semantic_records(1_000);

    c.bench_function("index_upsert_1000_records", |b| {
        b.iter_batched(
            || records.clone(),
            |records| {
                let cfg = IndexConfig::new().with_backend(BackendConfig::in_memory());
                let index = UfpIndex::new(cfg).expect("in-memory index");
                for record in records {
                    index.upsert(&record).expect("upsert succeeds");
                }
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_semantic_search(c: &mut Criterion) {
    let (index, query) = build_semantic_index(2_000);

    c.bench_function("index_semantic_search_top10", |b| {
        b.iter(|| {
            let hits = index
                .search(&query, QueryMode::Semantic, 10)
                .expect("semantic search");
            black_box(hits);
        });
    });
}

fn bench_perceptual_search(c: &mut Criterion) {
    let (index, query) = build_perceptual_index(2_000);

    c.bench_function("index_perceptual_search_top10", |b| {
        b.iter(|| {
            let hits = index
                .search(&query, QueryMode::Perceptual, 10)
                .expect("perceptual search");
            black_box(hits);
        });
    });
}

fn build_semantic_index(count: usize) -> (UfpIndex, IndexRecord) {
    let records = build_semantic_records(count);
    let query = semantic_query();
    (populate_index(records), query)
}

fn build_perceptual_index(count: usize) -> (UfpIndex, IndexRecord) {
    let records = build_perceptual_records(count);
    let query = perceptual_query();
    (populate_index(records), query)
}

fn populate_index(records: Vec<IndexRecord>) -> UfpIndex {
    let cfg = IndexConfig::new().with_backend(BackendConfig::in_memory());
    let index = UfpIndex::new(cfg).expect("index init");
    for record in &records {
        index.upsert(record).expect("seed record");
    }
    index
}

fn semantic_query() -> IndexRecord {
    IndexRecord {
        schema_version: INDEX_SCHEMA_VERSION,
        canonical_hash: "query-semantic".into(),
        perceptual: None,
        embedding: Some((0..64).map(|i| (i as i8 % 40) - 20).collect()),
        metadata: json!({}),
    }
}

fn perceptual_query() -> IndexRecord {
    IndexRecord {
        schema_version: INDEX_SCHEMA_VERSION,
        canonical_hash: "query-perceptual".into(),
        perceptual: Some((0..128).map(|i| i as u64 + 10).collect()),
        embedding: None,
        metadata: json!({}),
    }
}

fn build_semantic_records(count: usize) -> Vec<IndexRecord> {
    (0..count)
        .map(|i| IndexRecord {
            schema_version: INDEX_SCHEMA_VERSION,
            canonical_hash: format!("semantic-doc-{i:05}"),
            perceptual: None,
            embedding: Some(
                (0..64)
                    .map(|offset| {
                        let seed = ((i + offset) % 127) as i32 - 63;
                        seed as i8
                    })
                    .collect(),
            ),
            metadata: json!({ "tenant": i % 5 }),
        })
        .collect()
}

fn build_perceptual_records(count: usize) -> Vec<IndexRecord> {
    (0..count)
        .map(|i| IndexRecord {
            schema_version: INDEX_SCHEMA_VERSION,
            canonical_hash: format!("perceptual-doc-{i:05}"),
            perceptual: Some(
                (0..128)
                    .map(|offset| i as u64 * 3 + offset as u64)
                    .collect(),
            ),
            embedding: None,
            metadata: json!({ "tenant": (i + 1) % 7 }),
        })
        .collect()
}

criterion_group!(
    index_benches,
    bench_index_upserts,
    bench_semantic_search,
    bench_perceptual_search
);
criterion_main!(index_benches);
