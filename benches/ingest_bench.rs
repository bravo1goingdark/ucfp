use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use ingest::{ingest, IngestConfig, IngestMetadata, IngestPayload, IngestSource, RawIngestRecord};
use std::hint::black_box;

mod common;
use common::generate_text_byte_count;

fn sample_metadata() -> IngestMetadata {
    IngestMetadata {
        tenant_id: Some("bench-tenant".to_string()),
        doc_id: Some("bench-doc-001".to_string()),
        received_at: None,
        original_source: None,
        attributes: None,
    }
}

fn create_text_record(text: &str) -> RawIngestRecord {
    RawIngestRecord {
        id: "bench-record".into(),
        source: IngestSource::RawText,
        metadata: sample_metadata(),
        payload: Some(IngestPayload::Text(text.to_string())),
    }
}

fn create_bytes_record(bytes: &[u8]) -> RawIngestRecord {
    RawIngestRecord {
        id: "bench-record".into(),
        source: IngestSource::RawText,
        metadata: sample_metadata(),
        payload: Some(IngestPayload::TextBytes(bytes.to_vec())),
    }
}

/// Benchmark ingest with different payload sizes (text)
fn bench_ingest_text_sizes(c: &mut Criterion) {
    let config = IngestConfig::default();
    let mut group = c.benchmark_group("ingest_text");

    for size in [100, 1000, 10000, 100000].iter() {
        let text = generate_text_byte_count(*size);
        let record = create_text_record(&text);

        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_function(format!("bytes_{}", size), |b| {
            b.iter(|| {
                let _ = ingest(black_box(record.clone()), black_box(&config))
                    .expect("ingest should succeed");
            });
        });
    }

    group.finish();
}

/// Benchmark ingest with different payload types
fn bench_ingest_payload_types(c: &mut Criterion) {
    let config = IngestConfig::default();
    let mut group = c.benchmark_group("ingest_payload_types");
    let text = generate_text_byte_count(1000);
    let bytes = text.as_bytes().to_vec();

    // Text payload
    let text_record = create_text_record(&text);
    group.bench_function("text", |b| {
        b.iter(|| {
            let _ = ingest(black_box(text_record.clone()), black_box(&config))
                .expect("ingest should succeed");
        });
    });

    // TextBytes payload
    let bytes_record = create_bytes_record(&bytes);
    group.bench_function("text_bytes", |b| {
        b.iter(|| {
            let _ = ingest(black_box(bytes_record.clone()), black_box(&config))
                .expect("ingest should succeed");
        });
    });

    group.finish();
}

/// Benchmark metadata normalization
fn bench_ingest_metadata(c: &mut Criterion) {
    let config = IngestConfig::default();
    let mut group = c.benchmark_group("ingest_metadata");
    let text = generate_text_byte_count(1000);

    // Without metadata
    let no_meta_record = RawIngestRecord {
        id: "bench-record".into(),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: None,
            doc_id: None,
            received_at: None,
            original_source: None,
            attributes: None,
        },
        payload: Some(IngestPayload::Text(text.clone())),
    };
    group.bench_function("no_metadata", |b| {
        b.iter(|| {
            let _ = ingest(black_box(no_meta_record.clone()), black_box(&config))
                .expect("ingest should succeed");
        });
    });

    // With metadata
    let with_meta_record = create_text_record(&text);
    group.bench_function("with_metadata", |b| {
        b.iter(|| {
            let _ = ingest(black_box(with_meta_record.clone()), black_box(&config))
                .expect("ingest should succeed");
        });
    });

    group.finish();
}

/// Benchmark validation overhead
fn bench_ingest_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("ingest_validation");
    let text = generate_text_byte_count(1000);

    // Strict validation
    let strict_config = IngestConfig {
        max_payload_bytes: Some(10000),
        ..Default::default()
    };
    let record = create_text_record(&text);
    group.bench_function("strict", |b| {
        b.iter(|| {
            let _ = ingest(black_box(record.clone()), black_box(&strict_config))
                .expect("ingest should succeed");
        });
    });

    // Lenient validation
    let lenient_config = IngestConfig {
        max_payload_bytes: Some(100_000_000), // 100MB
        ..Default::default()
    };
    group.bench_function("lenient", |b| {
        b.iter(|| {
            let _ = ingest(black_box(record.clone()), black_box(&lenient_config))
                .expect("ingest should succeed");
        });
    });

    group.finish();
}

/// Benchmark different ingest sources
fn bench_ingest_sources(c: &mut Criterion) {
    let config = IngestConfig::default();
    let mut group = c.benchmark_group("ingest_sources");
    let text = generate_text_byte_count(1000);

    for source in &[IngestSource::RawText, IngestSource::Api] {
        let record = RawIngestRecord {
            id: "bench-record".into(),
            source: source.clone(),
            metadata: sample_metadata(),
            payload: Some(IngestPayload::Text(text.clone())),
        };

        group.bench_function(format!("{:?}", source), |b| {
            b.iter(|| {
                let _ = ingest(black_box(record.clone()), black_box(&config))
                    .expect("ingest should succeed");
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_ingest_text_sizes,
    bench_ingest_payload_types,
    bench_ingest_metadata,
    bench_ingest_validation,
    bench_ingest_sources
);
criterion_main!(benches);
