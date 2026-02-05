use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use serde_json::json;

mod common;
use common::{create_sample_records, setup_in_memory_index, setup_redb_index};

/// Benchmark single record insertion (put)
fn bench_backend_put_single(c: &mut Criterion) {
    let mut group = c.benchmark_group("backend_put_single");

    // InMemory backend
    group.bench_function("in_memory", |b| {
        let index = setup_in_memory_index();
        let _record = create_sample_records(1).into_iter().next().unwrap();

        b.iter(|| {
            let rec = create_sample_records(1).into_iter().next().unwrap();
            index
                .upsert(black_box(&rec))
                .expect("upsert should succeed");
        });
    });

    // Redb backend
    group.bench_function("redb", |b| {
        let (index, _temp_dir) = setup_redb_index();
        b.iter(|| {
            let rec = create_sample_records(1).into_iter().next().unwrap();
            index
                .upsert(black_box(&rec))
                .expect("upsert should succeed");
        });
    });

    group.finish();
}

/// Benchmark batch insertion
fn bench_backend_batch_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("backend_batch_insert");

    for size in [10, 100, 1000].iter() {
        let records = create_sample_records(*size);

        // InMemory backend
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_function(format!("in_memory_{}", size), |b| {
            b.iter(|| {
                let index = setup_in_memory_index();
                for record in &records {
                    index
                        .upsert(black_box(record))
                        .expect("upsert should succeed");
                }
            });
        });

        // Redb backend
        group.bench_function(format!("redb_{}", size), |b| {
            b.iter(|| {
                let (index, _temp_dir) = setup_redb_index();
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

/// Benchmark random reads (get)
fn bench_backend_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("backend_get");

    // Setup indices with 1000 records
    let inmem_index = setup_in_memory_index();
    let (redb_index, _temp_dir) = setup_redb_index();
    let records = create_sample_records(1000);

    for record in &records {
        inmem_index.upsert(record).expect("upsert should succeed");
        redb_index.upsert(record).expect("upsert should succeed");
    }

    let hash_keys: Vec<String> = records.iter().map(|r| r.canonical_hash.clone()).collect();

    // InMemory backend
    group.bench_function("in_memory", |b| {
        let mut i = 0;
        b.iter(|| {
            let key = &hash_keys[i % hash_keys.len()];
            let _ = inmem_index.get(black_box(key)).expect("get should succeed");
            i += 1;
        });
    });

    // Redb backend
    group.bench_function("redb", |b| {
        let mut i = 0;
        b.iter(|| {
            let key = &hash_keys[i % hash_keys.len()];
            let _ = redb_index.get(black_box(key)).expect("get should succeed");
            i += 1;
        });
    });

    group.finish();
}

/// Benchmark full table scan
fn bench_backend_scan(c: &mut Criterion) {
    let mut group = c.benchmark_group("backend_scan");

    for size in [100, 1000, 10000].iter() {
        let records = create_sample_records(*size);

        // InMemory backend
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_function(format!("in_memory_{}", size), |b| {
            let index = setup_in_memory_index();
            for record in &records {
                index.upsert(record).expect("upsert should succeed");
            }

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

        // Redb backend
        group.bench_function(format!("redb_{}", size), |b| {
            let (index, _temp_dir) = setup_redb_index();
            for record in &records {
                index.upsert(record).expect("upsert should succeed");
            }

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

/// Benchmark update operations
fn bench_backend_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("backend_update");

    // Setup indices with 1000 records
    let inmem_index = setup_in_memory_index();
    let (redb_index, _temp_dir) = setup_redb_index();
    let records = create_sample_records(1000);

    for record in &records {
        inmem_index.upsert(record).expect("upsert should succeed");
        redb_index.upsert(record).expect("upsert should succeed");
    }

    // InMemory backend - update existing records
    group.bench_function("in_memory", |b| {
        let mut i = 0;
        b.iter(|| {
            let mut rec = records[i % records.len()].clone();
            rec.metadata = json!({"updated": i});
            inmem_index
                .upsert(black_box(&rec))
                .expect("upsert should succeed");
            i += 1;
        });
    });

    // Redb backend - update existing records
    group.bench_function("redb", |b| {
        let mut i = 0;
        b.iter(|| {
            let mut rec = records[i % records.len()].clone();
            rec.metadata = json!({"updated": i});
            redb_index
                .upsert(black_box(&rec))
                .expect("upsert should succeed");
            i += 1;
        });
    });

    group.finish();
}

/// Benchmark mixed workload (read/write ratio)
fn bench_backend_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("backend_mixed_workload");

    // Setup indices with 1000 records
    let inmem_index = setup_in_memory_index();
    let (redb_index, _temp_dir) = setup_redb_index();
    let records = create_sample_records(1000);

    for record in &records {
        inmem_index.upsert(record).expect("upsert should succeed");
        redb_index.upsert(record).expect("upsert should succeed");
    }

    let hash_keys: Vec<String> = records.iter().map(|r| r.canonical_hash.clone()).collect();

    // 80% reads, 20% writes
    group.bench_function("in_memory_80r20w", |b| {
        let mut i = 0;
        b.iter(|| {
            if i % 5 == 0 {
                // Write
                let rec = create_sample_records(1).into_iter().next().unwrap();
                inmem_index
                    .upsert(black_box(&rec))
                    .expect("upsert should succeed");
            } else {
                // Read
                let key = &hash_keys[i % hash_keys.len()];
                let _ = inmem_index.get(black_box(key)).expect("get should succeed");
            }
            i += 1;
        });
    });

    group.bench_function("redb_80r20w", |b| {
        let mut i = 0;
        b.iter(|| {
            if i % 5 == 0 {
                // Write
                let rec = create_sample_records(1).into_iter().next().unwrap();
                redb_index
                    .upsert(black_box(&rec))
                    .expect("upsert should succeed");
            } else {
                // Read
                let key = &hash_keys[i % hash_keys.len()];
                let _ = redb_index.get(black_box(key)).expect("get should succeed");
            }
            i += 1;
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_backend_put_single,
    bench_backend_batch_insert,
    bench_backend_get,
    bench_backend_scan,
    bench_backend_update,
    bench_backend_mixed_workload
);
criterion_main!(benches);
