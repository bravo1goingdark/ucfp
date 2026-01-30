use chrono::{DateTime, NaiveDate, Utc};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ucfp::{
    canonicalize, process_record_with_perceptual, CanonicalizeConfig, IngestMetadata,
    IngestPayload, IngestSource, PerceptualConfig, RawIngestRecord,
};

const BIG_TEXT: &str = include_str!("../crates/canonical/examples/big_text.txt");

// Text samples of various sizes for throughput testing
const SMALL_TEXT: &str = "The quick brown fox jumps over the lazy dog.";
const MEDIUM_TEXT: &str = r#"
Rust is a multi-paradigm, general-purpose programming language that emphasizes 
performance, type safety, and concurrency. It enforces memory safety—meaning that 
all references point to valid memory—without a garbage collector. To simultaneously 
enforce memory safety and prevent data races, its "borrow checker" tracks the object 
lifetime of all references during compilation. Rust was influenced by ideas from 
functional programming, including immutability, higher-order functions, and algebraic 
data types. It is popular for systems programming.
"#;

fn canonical_bench(c: &mut Criterion) {
    let cfg = CanonicalizeConfig::default();
    c.bench_function("canonicalize_big_text", |b| {
        b.iter(|| {
            let doc = canonicalize("bench-canonical", black_box(BIG_TEXT), &cfg)
                .expect("bench canonical");
            black_box(doc);
        });
    });
}

fn perceptual_bench(c: &mut Criterion) {
    let canonical_cfg = CanonicalizeConfig::default();
    let doc = canonicalize("bench-canonical", BIG_TEXT, &canonical_cfg)
        .expect("canonicalization succeeds");
    let tokens: Vec<&str> = doc.tokens.iter().map(|t| t.text.as_str()).collect();
    let perceptual_cfg = PerceptualConfig::default();

    c.bench_function("perceptualize_big_text", |b| {
        b.iter(|| {
            let fp = process_perceptual(black_box(&tokens), &perceptual_cfg);
            black_box(fp);
        });
    });
}

fn pipeline_bench(c: &mut Criterion) {
    let canonical_cfg = CanonicalizeConfig::default();
    let perceptual_cfg = PerceptualConfig::default();

    c.bench_function("process_record_with_perceptual_big_text", |b| {
        b.iter(|| {
            let raw = demo_raw_record();
            let result =
                process_record_with_perceptual(raw, &canonical_cfg, &perceptual_cfg).unwrap();
            black_box(result);
        });
    });
}

fn process_perceptual(tokens: &[&str], cfg: &PerceptualConfig) -> ucfp::PerceptualFingerprint {
    ucfp::perceptualize_tokens(tokens, cfg).expect("perceptualization should succeed")
}

fn demo_raw_record() -> RawIngestRecord {
    RawIngestRecord {
        id: "bench-big-text".into(),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: Some("bench-tenant".into()),
            doc_id: Some("bench-doc".into()),
            received_at: Some(demo_timestamp()),
            original_source: Some("benches/pipeline.rs".into()),
            attributes: None,
        },
        payload: Some(IngestPayload::Text(BIG_TEXT.to_string())),
    }
}

fn demo_timestamp() -> DateTime<Utc> {
    let date = NaiveDate::from_ymd_opt(2025, 1, 1).expect("valid bench date");
    let time = date
        .and_hms_opt(0, 0, 0)
        .expect("valid bench timestamp components");
    DateTime::<Utc>::from_naive_utc_and_offset(time, Utc)
}

// Extended benchmarks

fn canonicalize_various_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("canonicalize_throughput");
    let cfg = CanonicalizeConfig::default();

    let inputs = [
        ("small", SMALL_TEXT),
        ("medium", MEDIUM_TEXT),
        ("big", BIG_TEXT),
    ];

    for (name, text) in inputs.iter() {
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_with_input(BenchmarkId::new("canonicalize", *name), text, |b, text| {
            b.iter(|| {
                let doc = canonicalize("bench", black_box(text), &cfg).expect("canonicalize");
                black_box(doc);
            });
        });
    }

    group.finish();
}

fn perceptual_various_k_values(c: &mut Criterion) {
    let mut group = c.benchmark_group("perceptual_k_comparison");
    let canonical_cfg = CanonicalizeConfig::default();
    let doc = canonicalize("bench", BIG_TEXT, &canonical_cfg).expect("canonical");
    let tokens: Vec<&str> = doc.tokens.iter().map(|t| t.text.as_str()).collect();

    for k in [3, 5, 7, 9, 11].iter() {
        let cfg = PerceptualConfig {
            k: *k,
            ..Default::default()
        };

        group.bench_with_input(
            BenchmarkId::new("perceptualize", format!("k={k}")),
            &tokens,
            |b, tokens| {
                b.iter(|| {
                    let fp =
                        ucfp::perceptualize_tokens(black_box(tokens), &cfg).expect("perceptualize");
                    black_box(fp);
                });
            },
        );
    }

    group.finish();
}

fn perceptual_parallel_vs_serial(c: &mut Criterion) {
    let mut group = c.benchmark_group("perceptual_parallel");
    let canonical_cfg = CanonicalizeConfig::default();
    let doc = canonicalize("bench", BIG_TEXT, &canonical_cfg).expect("canonical");
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
            let fp =
                ucfp::perceptualize_tokens(black_box(&tokens), &serial_cfg).expect("perceptualize");
            black_box(fp);
        });
    });

    group.bench_function("parallel", |b| {
        b.iter(|| {
            let fp = ucfp::perceptualize_tokens(black_box(&tokens), &parallel_cfg)
                .expect("perceptualize");
            black_box(fp);
        });
    });

    group.finish();
}

fn pipeline_various_text_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_throughput");
    let canonical_cfg = CanonicalizeConfig::default();
    let perceptual_cfg = PerceptualConfig::default();

    let inputs = [
        ("small", SMALL_TEXT),
        ("medium", MEDIUM_TEXT),
        ("big", BIG_TEXT),
    ];

    for (name, text) in inputs.iter() {
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_with_input(BenchmarkId::new("full_pipeline", *name), text, |b, text| {
            let raw = create_raw_record(text);
            b.iter(|| {
                let result = process_record_with_perceptual(
                    black_box(raw.clone()),
                    &canonical_cfg,
                    &perceptual_cfg,
                )
                .unwrap();
                black_box(result);
            });
        });
    }

    group.finish();
}

fn create_raw_record(text: &str) -> RawIngestRecord {
    RawIngestRecord {
        id: format!("bench-{}", text.len()),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: Some("bench".into()),
            doc_id: Some("bench".into()),
            received_at: None,
            original_source: None,
            attributes: None,
        },
        payload: Some(IngestPayload::Text(text.to_string())),
    }
}

fn tokenization_speed(c: &mut Criterion) {
    let mut group = c.benchmark_group("tokenization");

    let inputs = [
        ("small", SMALL_TEXT),
        ("medium", MEDIUM_TEXT),
        ("big", BIG_TEXT),
    ];

    for (name, text) in inputs.iter() {
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_with_input(BenchmarkId::new("tokenize", *name), text, |b, text| {
            b.iter(|| {
                let tokens = ucfp::tokenize(black_box(text));
                black_box(tokens);
            });
        });
    }

    group.finish();
}

criterion_group!(
    pipeline_benches,
    canonical_bench,
    perceptual_bench,
    pipeline_bench
);

criterion_group!(
    extended_benches,
    canonicalize_various_sizes,
    perceptual_various_k_values,
    perceptual_parallel_vs_serial,
    pipeline_various_text_sizes,
    tokenization_speed
);

criterion_main!(pipeline_benches, extended_benches);
