use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use index::{IndexRecord, INDEX_SCHEMA_VERSION};
use serde_json::json;
use std::sync::Arc;
use std::thread;

mod common;
use common::{create_sample_records, setup_in_memory_index};

/// Benchmark concurrent upserts with different thread counts
fn bench_concurrent_upserts(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_upserts");
    let total_records = 10000;

    for num_threads in [1, 2, 4, 8].iter() {
        group.throughput(Throughput::Elements(total_records as u64));
        group.bench_function(format!("{}_threads", num_threads), |b| {
            b.iter_with_setup(
                || {
                    let index = Arc::new(setup_in_memory_index());
                    let records = create_sample_records(total_records);
                    let chunks: Vec<Vec<IndexRecord>> = records
                        .chunks(total_records / num_threads)
                        .map(|c| c.to_vec())
                        .collect();
                    (index, chunks)
                },
                |(index, chunks)| {
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

/// Benchmark concurrent queries under load
fn bench_concurrent_queries(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_queries");

    // Pre-populate index
    let index = Arc::new(setup_in_memory_index());
    let records = create_sample_records(10000);
    for record in &records {
        index.upsert(record).expect("upsert should succeed");
    }

    let query_keys: Vec<String> = records.iter().map(|r| r.canonical_hash.clone()).collect();

    for num_threads in [1, 2, 4, 8].iter() {
        let queries_per_thread = 1000;
        group.throughput(Throughput::Elements(
            (num_threads * queries_per_thread) as u64,
        ));
        group.bench_function(format!("{}_threads", num_threads), |b| {
            b.iter(|| {
                let handles: Vec<_> = (0..*num_threads)
                    .map(|thread_id| {
                        let idx = Arc::clone(&index);
                        let keys = query_keys.clone();
                        thread::spawn(move || {
                            for i in 0..queries_per_thread {
                                let key = &keys[(thread_id * queries_per_thread + i) % keys.len()];
                                let _ = idx.get(key);
                            }
                        })
                    })
                    .collect();

                for handle in handles {
                    handle.join().expect("thread should complete");
                }
            });
        });
    }

    group.finish();
}

/// Benchmark read/write contention
fn bench_read_write_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("read_write_contention");
    let index = Arc::new(setup_in_memory_index());
    let records = create_sample_records(1000);

    // Pre-populate
    for record in &records {
        index.upsert(record).expect("upsert should succeed");
    }

    let query_keys: Vec<String> = records.iter().map(|r| r.canonical_hash.clone()).collect();

    for write_ratio in [10, 25, 50].iter() {
        group.bench_function(format!("{}_percent_writes", write_ratio), |b| {
            b.iter(|| {
                let handles: Vec<_> = (0..4)
                    .map(|thread_id| {
                        let idx = Arc::clone(&index);
                        let keys = query_keys.clone();
                        thread::spawn(move || {
                            for i in 0..100 {
                                if i % (100 / write_ratio) == 0 {
                                    // Write operation
                                    let new_record = IndexRecord {
                                        schema_version: INDEX_SCHEMA_VERSION,
                                        canonical_hash: format!("new-{}-{}", thread_id, i),
                                        perceptual: Some(vec![i as u64]),
                                        embedding: Some(vec![i as i8]),
                                        metadata: json!({"thread": thread_id, "op": i}),
                                    };
                                    let _ = idx.upsert(&new_record);
                                } else {
                                    // Read operation
                                    let key = &keys[i % keys.len()];
                                    let _ = idx.get(key);
                                }
                            }
                        })
                    })
                    .collect();

                for handle in handles {
                    handle.join().expect("thread should complete");
                }
            });
        });
    }

    group.finish();
}

/// Benchmark DashMap operations (lock-free hash map)
fn bench_dashmap_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("dashmap_operations");
    let index = Arc::new(setup_in_memory_index());

    // Populate initial data
    let initial_records = create_sample_records(1000);
    for record in &initial_records {
        index.upsert(record).expect("upsert should succeed");
    }

    // Concurrent insert benchmark
    group.bench_function("concurrent_insert", |b| {
        b.iter_with_setup(
            || create_sample_records(100),
            |records| {
                let handles: Vec<_> = records
                    .into_iter()
                    .map(|record| {
                        let idx = Arc::clone(&index);
                        thread::spawn(move || {
                            idx.upsert(&record).expect("upsert should succeed");
                        })
                    })
                    .collect();

                for handle in handles {
                    handle.join().expect("thread should complete");
                }
            },
        );
    });

    // Concurrent read benchmark
    let read_keys: Vec<String> = initial_records
        .iter()
        .map(|r| r.canonical_hash.clone())
        .collect();
    group.bench_function("concurrent_read", |b| {
        b.iter(|| {
            let keys = read_keys.clone();
            let handles: Vec<_> = keys
                .into_iter()
                .map(|key| {
                    let idx = Arc::clone(&index);
                    thread::spawn(move || {
                        let _ = idx.get(&key);
                    })
                })
                .collect();

            for handle in handles {
                handle.join().expect("thread should complete");
            }
        });
    });

    group.finish();
}

/// Benchmark throughput (operations per second)
fn bench_throughput_rps(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput_rps");
    let index = Arc::new(setup_in_memory_index());

    // Pre-populate
    let records = create_sample_records(1000);
    for record in &records {
        index.upsert(record).expect("upsert should succeed");
    }

    let keys: Vec<String> = records.iter().map(|r| r.canonical_hash.clone()).collect();

    // Measure sustained throughput with mixed workload
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(5));
    group.bench_function("mixed_workload", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..4)
                .map(|thread_id| {
                    let idx = Arc::clone(&index);
                    let thread_keys = keys.clone();
                    thread::spawn(move || {
                        for i in 0..1000 {
                            if i % 10 == 0 {
                                // 10% writes
                                let record = IndexRecord {
                                    schema_version: INDEX_SCHEMA_VERSION,
                                    canonical_hash: format!("throughput-{}-{}", thread_id, i),
                                    perceptual: Some(vec![i as u64]),
                                    embedding: Some(vec![i as i8]),
                                    metadata: json!({"i": i}),
                                };
                                let _ = idx.upsert(&record);
                            } else {
                                // 90% reads
                                let key = &thread_keys[i % thread_keys.len()];
                                let _ = idx.get(key);
                            }
                        }
                    })
                })
                .collect();

            for handle in handles {
                handle.join().expect("thread should complete");
            }
        });
    });

    group.finish();
}

/// Benchmark batch operations with different thread counts
fn bench_concurrent_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_batch");
    let batch_sizes = vec![100, 1000, 5000];

    for batch_size in &batch_sizes {
        let records = create_sample_records(*batch_size);

        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_function(format!("batch_{}", batch_size), |b| {
            b.iter_with_setup(
                || {
                    let index = Arc::new(setup_in_memory_index());
                    (index, records.clone())
                },
                |(index, recs)| {
                    use rayon::prelude::*;

                    recs.par_chunks(100).for_each(|chunk| {
                        for record in chunk {
                            let _ = index.upsert(record);
                        }
                    });
                },
            );
        });
    }

    group.finish();
}

/// Benchmark latency under concurrent load
fn bench_latency_under_load(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency_under_load");

    // Setup index with background load
    let index = Arc::new(setup_in_memory_index());
    let records = create_sample_records(1000);
    for record in &records {
        index.upsert(record).expect("upsert should succeed");
    }

    let keys: Vec<String> = records.iter().map(|r| r.canonical_hash.clone()).collect();

    // Start background writer thread
    let bg_index = Arc::clone(&index);
    let bg_handle = thread::spawn(move || {
        for i in 0..10000 {
            let record = IndexRecord {
                schema_version: INDEX_SCHEMA_VERSION,
                canonical_hash: format!("bg-{}", i),
                perceptual: Some(vec![i as u64]),
                embedding: Some(vec![i as i8]),
                metadata: json!({"bg": true}),
            };
            let _ = bg_index.upsert(&record);
            thread::sleep(std::time::Duration::from_micros(100));
        }
    });

    // Measure read latency while background load is running
    group.bench_function("read_with_bg_write", |b| {
        b.iter(|| {
            for key in &keys[..10] {
                let _ = index.get(black_box(key));
            }
        });
    });

    // Cleanup
    drop(bg_handle);

    group.finish();
}

criterion_group!(
    benches,
    bench_concurrent_upserts,
    bench_concurrent_queries,
    bench_read_write_contention,
    bench_dashmap_operations,
    bench_throughput_rps,
    bench_concurrent_batch,
    bench_latency_under_load
);
criterion_main!(benches);
