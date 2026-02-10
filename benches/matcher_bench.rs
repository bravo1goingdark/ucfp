use canonical::CanonicalizeConfig;
use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use index::{IndexRecord, INDEX_SCHEMA_VERSION};
use ingest::IngestConfig;
use matcher::{MatchConfig, MatchMode, MatchRequest, Matcher};
use perceptual::PerceptualConfig;
use semantic::SemanticConfig;
use serde_json::json;

mod common;
use common::{create_sample_records, setup_in_memory_index};

/// Setup matcher with pre-populated index
fn setup_matcher_with_index(record_count: usize) -> Matcher {
    let index = setup_in_memory_index();
    let records = create_sample_records(record_count);

    // Insert records
    for record in records {
        index.upsert(&record).expect("upsert should succeed");
    }

    let ingest_cfg = IngestConfig::default();
    let canonical_cfg = CanonicalizeConfig::default();
    let perceptual_cfg = PerceptualConfig {
        k: 3,
        ..Default::default()
    };
    let semantic_cfg = SemanticConfig::default();

    Matcher::new(
        index,
        ingest_cfg,
        canonical_cfg,
        perceptual_cfg,
        semantic_cfg,
    )
}

/// Benchmark match_document with different match modes
fn bench_match_modes(c: &mut Criterion) {
    let mut group = c.benchmark_group("match_modes");

    // Setup matcher with 1000 records
    let matcher = setup_matcher_with_index(1000);

    for mode in &[
        MatchMode::Semantic,
        MatchMode::Perceptual,
        MatchMode::Hybrid,
    ] {
        let config = MatchConfig {
            mode: *mode,
            ..MatchConfig::default()
        };

        let request = MatchRequest {
            tenant_id: "bench-tenant".to_string(),
            query_text: "bench query document".to_string(),
            config,
            attributes: None,
            pipeline_version: None,
            fingerprint_versions: None,
            query_canonical_hash: None,
        };

        group.bench_function(format!("{:?}", mode), |b| {
            b.iter(|| {
                let _ = matcher
                    .match_document(black_box(&request))
                    .expect("match should succeed");
            });
        });
    }

    group.finish();
}

/// Benchmark match_document with different index sizes
fn bench_match_scale(c: &mut Criterion) {
    let mut group = c.benchmark_group("match_scale");

    for &size in [100, 1000, 10000].iter() {
        let matcher = setup_matcher_with_index(size);

        let config = MatchConfig {
            mode: MatchMode::Perceptual,
            ..MatchConfig::default()
        };

        let request = MatchRequest {
            tenant_id: "bench-tenant".to_string(),
            query_text: "bench query document".to_string(),
            config,
            attributes: None,
            pipeline_version: None,
            fingerprint_versions: None,
            query_canonical_hash: None,
        };

        group.throughput(Throughput::Elements(size as u64));
        group.bench_function(format!("records_{}", size), |b| {
            b.iter(|| {
                let _ = matcher
                    .match_document(black_box(&request))
                    .expect("match should succeed");
            });
        });
    }

    group.finish();
}

/// Benchmark different strategies
fn bench_match_strategies(c: &mut Criterion) {
    let mut group = c.benchmark_group("match_strategies");
    let matcher = setup_matcher_with_index(1000);

    // Test different strategies
    let strategies = vec![
        (
            "semantic_cosine",
            MatchConfig::semantic_default("bench", "v1"),
        ),
        (
            "hybrid_weighted",
            MatchConfig {
                mode: MatchMode::Hybrid,
                strategy: matcher::MatchExpr::Weighted {
                    semantic_weight: 0.7,
                    min_overall: 0.5,
                },
                ..MatchConfig::default()
            },
        ),
    ];

    for (name, config) in strategies {
        let request = MatchRequest {
            tenant_id: "bench-tenant".to_string(),
            query_text: "bench query document".to_string(),
            config,
            attributes: None,
            pipeline_version: None,
            fingerprint_versions: None,
            query_canonical_hash: None,
        };

        group.bench_function(name, |b| {
            b.iter(|| {
                let _ = matcher
                    .match_document(black_box(&request))
                    .expect("match should succeed");
            });
        });
    }

    group.finish();
}

/// Benchmark query pipeline (building query record)
fn bench_query_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_pipeline");
    let matcher = setup_matcher_with_index(100);

    // Test building query from text
    let config = MatchConfig::default();
    let request = MatchRequest {
        tenant_id: "bench-tenant".to_string(),
        query_text: "This is a test query document for benchmarking the matcher pipeline."
            .to_string(),
        config,
        attributes: None,
        pipeline_version: None,
        fingerprint_versions: None,
        query_canonical_hash: None,
    };

    group.bench_function("full_pipeline", |b| {
        b.iter(|| {
            let _ = matcher
                .match_document(black_box(&request))
                .expect("match should succeed");
        });
    });

    group.finish();
}

/// Benchmark tenant filtering overhead
fn bench_tenant_filtering(c: &mut Criterion) {
    let mut group = c.benchmark_group("tenant_filtering");

    // Create index with multiple tenants
    let index = setup_in_memory_index();
    let tenants = vec!["tenant-a", "tenant-b", "tenant-c", "tenant-d"];

    for tenant in &tenants {
        for j in 0..250 {
            let record = IndexRecord {
                schema_version: INDEX_SCHEMA_VERSION,
                canonical_hash: format!("hash-{}-{}", tenant, j),
                perceptual: Some(vec![j as u64, (j + 1) as u64]),
                embedding: Some(vec![j as i8, (j + 1) as i8]),
                metadata: json!({"tenant": tenant, "id": j}),
            };
            index.upsert(&record).expect("upsert should succeed");
        }
    }

    let ingest_cfg = IngestConfig::default();
    let canonical_cfg = CanonicalizeConfig::default();
    let perceptual_cfg = PerceptualConfig {
        k: 3,
        ..Default::default()
    };
    let semantic_cfg = SemanticConfig::default();

    let matcher = Matcher::new(
        index,
        ingest_cfg,
        canonical_cfg,
        perceptual_cfg,
        semantic_cfg,
    );

    for tenant in &tenants {
        let request = MatchRequest {
            tenant_id: tenant.to_string(),
            query_text: "test query".to_string(),
            config: MatchConfig::default(),
            attributes: None,
            pipeline_version: None,
            fingerprint_versions: None,
            query_canonical_hash: None,
        };

        group.bench_function(*tenant, |b| {
            b.iter(|| {
                let _ = matcher
                    .match_document(black_box(&request))
                    .expect("match should succeed");
            });
        });
    }

    group.finish();
}

/// Benchmark match with different result limits
fn bench_result_limits(c: &mut Criterion) {
    let mut group = c.benchmark_group("result_limits");
    let matcher = setup_matcher_with_index(1000);

    for limit in [1, 5, 10, 50, 100].iter() {
        let config = MatchConfig {
            max_results: *limit,
            ..MatchConfig::default()
        };

        let request = MatchRequest {
            tenant_id: "bench-tenant".to_string(),
            query_text: "bench query".to_string(),
            config,
            attributes: None,
            pipeline_version: None,
            fingerprint_versions: None,
            query_canonical_hash: None,
        };

        group.bench_function(format!("limit_{}", limit), |b| {
            b.iter(|| {
                let _ = matcher
                    .match_document(black_box(&request))
                    .expect("match should succeed");
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_match_modes,
    bench_match_scale,
    bench_match_strategies,
    bench_query_pipeline,
    bench_tenant_filtering,
    bench_result_limits
);
criterion_main!(benches);
