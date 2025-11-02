use chrono::{DateTime, NaiveDate, Utc};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ucfp::{
    canonicalize, process_record_with_perceptual, CanonicalizeConfig, IngestMetadata,
    IngestPayload, IngestSource, PerceptualConfig, RawIngestRecord,
};

const BIG_TEXT: &str = include_str!("../crates/ufp_canonical/examples/big_text.txt");

fn canonical_bench(c: &mut Criterion) {
    let cfg = CanonicalizeConfig::default();
    c.bench_function("canonicalize_big_text", |b| {
        b.iter(|| {
            let doc = canonicalize(black_box(BIG_TEXT), &cfg);
            black_box(doc);
        });
    });
}

fn perceptual_bench(c: &mut Criterion) {
    let canonical_cfg = CanonicalizeConfig::default();
    let doc = canonicalize(BIG_TEXT, &canonical_cfg);
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
            tenant_id: "bench-tenant".into(),
            doc_id: "bench-doc".into(),
            received_at: demo_timestamp(),
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

criterion_group!(pipeline_benches, canonical_bench, perceptual_bench, pipeline_bench);
criterion_main!(pipeline_benches);
