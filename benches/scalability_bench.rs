use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use index::{IndexRecord, INDEX_SCHEMA_VERSION};
use serde_json::json;
use std::sync::Arc;
use std::thread;

mod common;
use common::setup_in_memory_index;

/// Create a sample record with variable size
fn create_record(id: usize, text_size: usize) -> IndexRecord {
    let text = "x".repeat(text_size);
    IndexRecord {
        schema_version: INDEX_SCHEMA_VERSION,
        canonical_hash: format!("hash-{}", id),
        perceptual: Some(vec![id as u64, (id + 1) as u64, (id + 2) as u64]),
        embedding: Some(vec![(id % 128) as i8; 384]), // 384-dim embedding
        metadata: json!({
            "id": id,
            "tenant": "bench",
            "content": text
        }),
    }
}

/// Benchmark batch insertion at scale (1K to 100K records)
fn bench_batch_insert_scale(c: &mut Criterion) {
    let mut group = c.benchmark_group("scalability_batch_insert");

    for size in [1000, 10000, 50000, 100000].iter() {
        let records: Vec<IndexRecord> = (0..*size).map(|i| create_record(i, 100)).collect();

        group.throughput(Throughput::Elements(*size as u64));
        group.sample_size(10); // Reduce sample size for large benchmarks
        group.bench_function(format!("records_{}", size), |b| {
            b.iter_with_setup(setup_in_memory_index, |index| {
                for record in &records {
                    index
                        .upsert(black_box(record))
                        .expect("upsert should succeed");
                }
            });
        });
    }

    group.finish();
}

/// Benchmark query performance as index grows
fn bench_query_at_scale(c: &mut Criterion) {
    let mut group = c.benchmark_group("scalability_query");

    for size in [1000, 10000, 50000, 100000].iter() {
        // Pre-populate index
        let index = setup_in_memory_index();
        for i in 0..*size {
            let record = create_record(i, 100);
            index.upsert(&record).expect("upsert should succeed");
        }

        let query_hash = "hash-500"; // Query for a specific record

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_function(format!("records_{}", size), |b| {
            b.iter(|| {
                let _ = index.get(black_box(query_hash));
            });
        });
    }

    group.finish();
}

/// Benchmark memory usage estimation at different scales
fn bench_memory_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("scalability_memory");

    for size in [1000, 10000, 50000].iter() {
        let records: Vec<IndexRecord> = (0..*size).map(|i| create_record(i, 100)).collect();

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_function(format!("records_{}", size), |b| {
            b.iter_with_setup(setup_in_memory_index, |index| {
                for record in &records {
                    index.upsert(record).expect("upsert should succeed");
                }
                // Index is dropped here, measuring insert overhead
            });
        });
    }

    group.finish();
}

/// Benchmark concurrent insert operations at scale
fn bench_concurrent_insert_scale(c: &mut Criterion) {
    let mut group = c.benchmark_group("scalability_concurrent_insert");

    for size in [1000, 10000, 50000].iter() {
        let records: Vec<IndexRecord> = (0..*size).map(|i| create_record(i, 100)).collect();

        group.throughput(Throughput::Elements(*size as u64));
        group.sample_size(10);
        group.bench_function(format!("records_{}", size), |b| {
            b.iter_with_setup(
                || {
                    let index = Arc::new(setup_in_memory_index());
                    (index, records.clone())
                },
                |(index, recs)| {
                    let chunks: Vec<Vec<IndexRecord>> =
                        recs.chunks(recs.len() / 4).map(|c| c.to_vec()).collect();

                    let handles: Vec<_> = chunks
                        .into_iter()
                        .map(|chunk| {
                            let idx = Arc::clone(&index);
                            thread::spawn(move || {
                                for record in chunk {
                                    idx.upsert(&record).expect("upsert should succeed");
                                }
                            })
                        })
                        .collect();

                    for handle in handles {
                        handle.join().expect("thread should complete");
                    }
                },
            );
        });
    }

    group.finish();
}

/// Benchmark perceptual search at scale
fn bench_perceptual_search_scale(c: &mut Criterion) {
    let mut group = c.benchmark_group("scalability_perceptual_search");

    for size in [1000, 10000, 50000].iter() {
        // Pre-populate with perceptual fingerprints
        let index = setup_in_memory_index();
        for i in 0..*size {
            let record = IndexRecord {
                schema_version: INDEX_SCHEMA_VERSION,
                canonical_hash: format!("hash-{}", i),
                perceptual: Some(vec![
                    i as u64 % 1000,
                    (i + 1) as u64 % 1000,
                    (i + 2) as u64 % 1000,
                ]),
                embedding: None,
                metadata: json!({"id": i}),
            };
            index.upsert(&record).expect("upsert should succeed");
        }

        let query_fp = vec![500u64, 501, 502];

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_function(format!("records_{}", size), |b| {
            b.iter(|| {
                let query_record = IndexRecord {
                    schema_version: INDEX_SCHEMA_VERSION,
                    canonical_hash: "query".into(),
                    perceptual: Some(query_fp.clone()),
                    embedding: None,
                    metadata: json!({}),
                };
                let _ = index.search(
                    black_box(&query_record),
                    black_box(index::QueryMode::Perceptual),
                    black_box(10),
                );
            });
        });
    }

    group.finish();
}

/// Benchmark semantic search at scale
fn bench_semantic_search_scale(c: &mut Criterion) {
    let mut group = c.benchmark_group("scalability_semantic_search");

    for size in [1000, 10000, 50000].iter() {
        // Pre-populate with embeddings
        let index = setup_in_memory_index();
        for i in 0..*size {
            let record = IndexRecord {
                schema_version: INDEX_SCHEMA_VERSION,
                canonical_hash: format!("hash-{}", i),
                perceptual: None,
                embedding: Some(vec![(i % 128) as i8; 384]),
                metadata: json!({"id": i}),
            };
            index.upsert(&record).expect("upsert should succeed");
        }

        let query_embedding = vec![64i8; 384];

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_function(format!("records_{}", size), |b| {
            b.iter(|| {
                let query_record = IndexRecord {
                    schema_version: INDEX_SCHEMA_VERSION,
                    canonical_hash: "query".into(),
                    perceptual: None,
                    embedding: Some(query_embedding.clone()),
                    metadata: json!({}),
                };
                let _ = index.search(
                    black_box(&query_record),
                    black_box(index::QueryMode::Semantic),
                    black_box(10),
                );
            });
        });
    }

    group.finish();
}

/// Benchmark full scan performance at scale
fn bench_full_scan_scale(c: &mut Criterion) {
    let mut group = c.benchmark_group("scalability_full_scan");

    for size in [1000, 10000, 50000, 100000].iter() {
        // Pre-populate index
        let index = setup_in_memory_index();
        for i in 0..*size {
            let record = create_record(i, 100);
            index.upsert(&record).expect("upsert should succeed");
        }

        group.throughput(Throughput::Elements(*size as u64));
        group.sample_size(10);
        group.bench_function(format!("records_{}", size), |b| {
            b.iter(|| {
                let mut count = 0;
                index
                    .scan(&mut |_rec| {
                        count += 1;
                        Ok(())
                    })
                    .expect("scan should succeed");
                black_box(count);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_batch_insert_scale,
    bench_query_at_scale,
    bench_memory_overhead,
    bench_concurrent_insert_scale,
    bench_perceptual_search_scale,
    bench_semantic_search_scale,
    bench_full_scan_scale
);
criterion_main!(benches);
