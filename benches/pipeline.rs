//! Pipeline integration benchmarks
//!
//! Tests full pipeline throughput, not individual stages (see canonicalize_bench, perceptual_bench).

use chrono::{DateTime, NaiveDate, Utc};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box;
use ucfp::{
    process_pipeline, CanonicalizeConfig, IngestConfig, IngestMetadata, IngestPayload,
    IngestSource, PerceptualConfig, PipelineStageConfig, RawIngestRecord,
};

const BIG_TEXT: &str = include_str!("../crates/canonical/examples/big_text.txt");
const SMALL_TEXT: &str = "The quick brown fox jumps over the lazy dog.";
const MEDIUM_TEXT: &str = r#"
Rust is a multi-paradigm, general-purpose programming language that emphasizes 
performance, type safety, and concurrency. It enforces memory safety without a 
garbage collector. To enforce memory safety and prevent data races, its borrow 
checker tracks object lifetimes during compilation.
"#;

fn pipeline_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline");
    let canonical_cfg = CanonicalizeConfig::default();
    let perceptual_cfg = PerceptualConfig::default();
    let ingest_cfg = IngestConfig::default();

    let inputs = [
        ("small", SMALL_TEXT),
        ("medium", MEDIUM_TEXT),
        ("big", BIG_TEXT),
    ];

    for (name, text) in inputs.iter() {
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_with_input(BenchmarkId::new("full", *name), text, |b, text| {
            let raw = create_raw_record(text);
            b.iter(|| {
                let result = process_pipeline(
                    black_box(raw.clone()),
                    PipelineStageConfig::Perceptual,
                    &ingest_cfg,
                    &canonical_cfg,
                    Some(&perceptual_cfg),
                    None,
                )
                .unwrap();
                black_box(result);
            });
        });
    }

    group.finish();
}

fn parallel_vs_serial(c: &mut Criterion) {
    let mut group = c.benchmark_group("perceptual_parallel");
    let canonical_cfg = CanonicalizeConfig::default();
    let doc = ucfp::canonicalize("bench", BIG_TEXT, &canonical_cfg).expect("canonical");
    let tokens: Vec<&str> = doc.tokens.iter().map(|t| t.text.as_str()).collect();

    let serial_cfg = PerceptualConfig {
        use_parallel: false,
        ..Default::default()
    };
    let parallel_cfg = PerceptualConfig {
        use_parallel: true,
        ..Default::default()
    };

    group.bench_function("serial", |b| {
        b.iter(|| {
            let fp = ucfp::perceptualize_tokens(black_box(&tokens), &serial_cfg).expect("ok");
            black_box(fp);
        });
    });

    group.bench_function("parallel", |b| {
        b.iter(|| {
            let fp = ucfp::perceptualize_tokens(black_box(&tokens), &parallel_cfg).expect("ok");
            black_box(fp);
        });
    });

    group.finish();
}

fn create_raw_record(text: &str) -> RawIngestRecord {
    RawIngestRecord {
        id: format!("bench-{}", text.len()),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: Some("bench".into()),
            doc_id: Some("bench".into()),
            received_at: Some(demo_timestamp()),
            original_source: None,
            attributes: None,
        },
        payload: Some(IngestPayload::Text(text.to_string())),
    }
}

fn demo_timestamp() -> DateTime<Utc> {
    let date = NaiveDate::from_ymd_opt(2025, 1, 1).expect("valid");
    let time = date.and_hms_opt(0, 0, 0).expect("valid");
    DateTime::<Utc>::from_naive_utc_and_offset(time, Utc)
}

criterion_group!(benches, pipeline_bench, parallel_vs_serial);
criterion_main!(benches);
