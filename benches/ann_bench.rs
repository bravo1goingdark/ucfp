use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use index::ann::{AnnConfig, AnnIndex};
use ndarray::Array1;
use rand::distr::StandardUniform;
use rand::RngExt;
use std::hint::black_box;

/// Generate random vector of given dimension
fn random_vector(dim: usize) -> Vec<f32> {
    let rng = rand::rng();
    // sample_iter is optimized for generating sequences
    rng.sample_iter(StandardUniform).take(dim).collect()
}

/// Benchmark ANN index insertion at different scales
fn bench_ann_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("ann_insert");
    let dim = 384; // BGE-small dimension

    for size in [100, 500, 1000, 5000].iter() {
        let vectors: Vec<(String, Vec<f32>)> = (0..*size)
            .map(|i| (format!("vec-{}", i), random_vector(dim)))
            .collect();

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_function(format!("insert_{}", size), |b| {
            b.iter(|| {
                let config = AnnConfig::default();
                let mut ann = AnnIndex::new(dim, config);
                for (id, vec) in &vectors {
                    let _ = ann.insert(black_box(id.clone()), black_box(vec.clone()));
                }
            });
        });
    }

    group.finish();
}

/// Benchmark ANN index build time at different scales
fn bench_ann_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("ann_build");
    let dim = 384;

    for size in [100, 500, 1000, 5000].iter() {
        let vectors: Vec<(String, Vec<f32>)> = (0..*size)
            .map(|i| (format!("vec-{}", i), random_vector(dim)))
            .collect();

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_function(format!("build_{}", size), |b| {
            b.iter_with_setup(
                || {
                    let config = AnnConfig::default();
                    let mut ann = AnnIndex::new(dim, config);
                    for (id, vec) in &vectors {
                        let _ = ann.insert(id.clone(), vec.clone());
                    }
                    ann
                },
                |mut ann| {
                    ann.build();
                },
            );
        });
    }

    group.finish();
}

/// Benchmark ANN search vs linear search
fn bench_ann_vs_linear(c: &mut Criterion) {
    let mut group = c.benchmark_group("ann_vs_linear");
    let dim = 384;

    for size in [100, 1000, 5000, 10000].iter() {
        let vectors: Vec<(String, Vec<f32>)> = (0..*size)
            .map(|i| (format!("vec-{}", i), random_vector(dim)))
            .collect();

        let query = random_vector(dim);

        // Linear search (no HNSW build)
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_function(format!("linear_{}", size), |b| {
            let config = AnnConfig::default();
            let mut ann = AnnIndex::new(dim, config);
            for (id, vec) in &vectors {
                let _ = ann.insert(id.clone(), vec.clone());
            }
            // Don't build - forces linear search

            b.iter(|| {
                let _ = ann.search(black_box(&query), black_box(10));
            });
        });

        // ANN search (with HNSW built)
        group.bench_function(format!("hnsw_{}", size), |b| {
            let config = AnnConfig {
                enabled: true,
                min_vectors_for_ann: 100,
                ..Default::default()
            };
            let mut ann = AnnIndex::new(dim, config);
            for (id, vec) in &vectors {
                let _ = ann.insert(id.clone(), vec.clone());
            }
            ann.build();

            b.iter(|| {
                let _ = ann.search(black_box(&query), black_box(10));
            });
        });
    }

    group.finish();
}

/// Benchmark ANN search with different top_k values
fn bench_ann_topk(c: &mut Criterion) {
    let mut group = c.benchmark_group("ann_topk");
    let dim = 384;
    let num_vectors = 5000;

    let vectors: Vec<(String, Vec<f32>)> = (0..num_vectors)
        .map(|i| (format!("vec-{}", i), random_vector(dim)))
        .collect();

    let config = AnnConfig {
        enabled: true,
        min_vectors_for_ann: 100,
        ..Default::default()
    };
    let mut ann = AnnIndex::new(dim, config);
    for (id, vec) in &vectors {
        let _ = ann.insert(id.clone(), vec.clone());
    }
    ann.build();

    let query = random_vector(dim);

    for k in [1, 5, 10, 50, 100].iter() {
        group.bench_function(format!("top_k_{}", k), |b| {
            b.iter(|| {
                let _ = ann.search(black_box(&query), black_box(*k));
            });
        });
    }

    group.finish();
}

/// Benchmark ANN with different HNSW parameters
fn bench_ann_parameters(c: &mut Criterion) {
    let mut group = c.benchmark_group("ann_parameters");
    let dim = 384;
    let num_vectors = 5000;

    let vectors: Vec<(String, Vec<f32>)> = (0..num_vectors)
        .map(|i| (format!("vec-{}", i), random_vector(dim)))
        .collect();

    let query = random_vector(dim);

    // Test different ef_search values (affects search quality vs speed tradeoff)
    for ef in [10, 50, 100, 200].iter() {
        group.bench_function(format!("ef_{}", ef), |b| {
            let config = AnnConfig {
                enabled: true,
                min_vectors_for_ann: 100,
                ef_search: *ef,
                ..Default::default()
            };
            let mut ann = AnnIndex::new(dim, config);
            for (id, vec) in &vectors {
                let _ = ann.insert(id.clone(), vec.clone());
            }
            ann.build();

            b.iter(|| {
                let _ = ann.search(black_box(&query), black_box(10));
            });
        });
    }

    group.finish();
}

/// Benchmark quantization performance
fn bench_quantization(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantization");

    let dim = 384;
    let scale = 127.0;

    for size in [100, 1000, 5000].iter() {
        let vectors: Vec<Array1<f32>> = (0..*size)
            .map(|_| Array1::from(random_vector(dim)))
            .collect();

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_function(format!("quantize_{}", size), |b| {
            b.iter(|| {
                for vec in &vectors {
                    let _ = index::UfpIndex::quantize(black_box(vec), black_box(scale));
                }
            });
        });
    }

    group.finish();
}

/// Benchmark search latency as index grows
fn bench_ann_degradation(c: &mut Criterion) {
    let mut group = c.benchmark_group("ann_degradation");
    let dim = 384;

    let config = AnnConfig {
        enabled: true,
        min_vectors_for_ann: 100,
        ..Default::default()
    };

    for size in [100, 1000, 10000].iter() {
        let vectors: Vec<(String, Vec<f32>)> = (0..*size)
            .map(|i| (format!("vec-{}", i), random_vector(dim)))
            .collect();

        let mut ann = AnnIndex::new(dim, config);
        for (id, vec) in &vectors {
            let _ = ann.insert(id.clone(), vec.clone());
        }
        ann.build();

        let query = random_vector(dim);

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_function(format!("search_at_{}", size), |b| {
            b.iter(|| {
                let _ = ann.search(black_box(&query), black_box(10));
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_ann_insert,
    bench_ann_build,
    bench_ann_vs_linear,
    bench_ann_topk,
    bench_ann_parameters,
    bench_quantization,
    bench_ann_degradation
);
criterion_main!(benches);
