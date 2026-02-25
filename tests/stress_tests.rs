//! Stress tests for UCFP - verifies system behavior under load

use std::time::{Duration, Instant};
use ucfp::{
    canonicalize, process_pipeline, CanonicalizeConfig, IngestConfig, IngestMetadata,
    IngestPayload, IngestSource, PerceptualConfig, PipelineStageConfig, RawIngestRecord,
};

fn generate_text(word_count: usize) -> String {
    let words = [
        "the", "quick", "brown", "fox", "jumps", "over", "lazy", "dog",
    ];
    (0..word_count)
        .map(|i| words[i % words.len()])
        .collect::<Vec<_>>()
        .join(" ")
}

fn create_record(id: &str, text: &str) -> RawIngestRecord {
    RawIngestRecord {
        id: id.into(),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: Some("stress".into()),
            doc_id: Some(format!("doc-{id}")),
            received_at: None,
            original_source: None,
            attributes: None,
        },
        payload: Some(IngestPayload::Text(text.into())),
    }
}

/// Large document processing (10k words)
#[test]
fn stress_large_document() {
    let text = generate_text(10000);
    let record = create_record("large", &text);

    let start = Instant::now();
    let result = process_pipeline(
        record,
        PipelineStageConfig::Perceptual,
        &IngestConfig::default(),
        &CanonicalizeConfig::default(),
        Some(&PerceptualConfig::default()),
        None,
    );
    let elapsed = start.elapsed();

    assert!(result.is_ok(), "Large document should process");
    println!("Processed 10k words in {elapsed:?}");
}

/// Many small documents
#[test]
fn stress_many_documents() {
    let count = 1000;
    let text = "This text has enough tokens for perceptual fingerprinting with default settings";

    let start = Instant::now();
    for i in 0..count {
        let record = create_record(&format!("doc-{i}"), text);
        let result = process_pipeline(
            record,
            PipelineStageConfig::Perceptual,
            &IngestConfig::default(),
            &CanonicalizeConfig::default(),
            Some(&PerceptualConfig::default()),
            None,
        );
        assert!(result.is_ok(), "Document {i} should process");
    }
    let elapsed = start.elapsed();
    let avg = elapsed / count as u32;

    println!("Processed {count} docs in {elapsed:?} (avg {avg:?})");
}

/// Repeated processing of same document (no memory growth)
#[test]
fn stress_repeated_processing() {
    let record = create_record("repeated", &generate_text(500));
    let iterations = 1000;

    let start = Instant::now();
    for _ in 0..iterations {
        let _ = process_pipeline(
            record.clone(),
            PipelineStageConfig::Perceptual,
            &IngestConfig::default(),
            &CanonicalizeConfig::default(),
            Some(&PerceptualConfig::default()),
            None,
        )
        .expect("should succeed");
    }
    let elapsed = start.elapsed();

    println!("{iterations} iterations in {elapsed:?}");
}

/// Memory doesn't blow up with many allocations
#[test]
fn stress_memory_allocation() {
    let count = 500;
    let text = generate_text(200);

    for i in 0..count {
        let doc = canonicalize(format!("mem-{i}"), &text, &CanonicalizeConfig::default())
            .expect("canonicalize");
        drop(doc); // Force drop

        if i % 100 == 0 {
            println!("Processed {i} documents");
        }
    }
}

/// Processing completes within reasonable time
#[test]
fn stress_performance_threshold() {
    let text = generate_text(1000);
    let record = create_record("perf", &text);

    let start = Instant::now();
    let _ = process_pipeline(
        record,
        PipelineStageConfig::Perceptual,
        &IngestConfig::default(),
        &CanonicalizeConfig::default(),
        Some(&PerceptualConfig::default()),
        None,
    )
    .expect("should succeed");
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_secs(5),
        "Processing took {elapsed:?}, expected < 5s"
    );
}
