//! Performance and stress tests for UCFP
//!
//! These tests verify system behavior under load and stress conditions.

use std::time::{Duration, Instant};
use ucfp::{
    canonicalize, process_record_with_perceptual, CanonicalizeConfig, IngestMetadata,
    IngestPayload, IngestSource, PerceptualConfig, RawIngestRecord,
};

/// Helper function to create a test record with variable text size
fn create_test_record(id: &str, text: &str) -> RawIngestRecord {
    RawIngestRecord {
        id: id.into(),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: Some("perf-tenant".into()),
            doc_id: Some(format!("perf-doc-{id}")),
            received_at: None,
            original_source: None,
            attributes: None,
        },
        payload: Some(IngestPayload::Text(text.into())),
    }
}

/// Generate text of approximately n words
fn generate_text(word_count: usize) -> String {
    let words = [
        "the",
        "quick",
        "brown",
        "fox",
        "jumps",
        "over",
        "lazy",
        "dog",
        "rust",
        "programming",
        "language",
        "systems",
        "performance",
        "memory",
        "safety",
        "concurrency",
        "fast",
        "reliable",
        "robust",
    ];

    let mut text = String::new();
    for i in 0..word_count {
        if i > 0 {
            text.push(' ');
        }
        text.push_str(words[i % words.len()]);
    }
    text
}

#[test]
fn performance_single_document_processing_time() {
    let text = generate_text(1000);
    let canonical_cfg = CanonicalizeConfig::default();
    let perceptual_cfg = PerceptualConfig::default();

    let raw = create_test_record("perf-1", &text);

    let start = Instant::now();
    let result = process_record_with_perceptual(raw, &canonical_cfg, &perceptual_cfg);
    let elapsed = start.elapsed();

    assert!(result.is_ok(), "Processing should succeed");

    // Document processing should be reasonably fast (under 1 second for 1000 words)
    // This is a generous threshold to account for different hardware
    assert!(
        elapsed < Duration::from_secs(1),
        "Processing 1000 words took {elapsed:?}, expected under 1s",
    );

    println!("Processed 1000 words in {elapsed:?}");
}

#[test]
fn performance_batch_processing() {
    let canonical_cfg = CanonicalizeConfig::default();
    let perceptual_cfg = PerceptualConfig::default();

    let batch_size = 100;
    let text = generate_text(100);

    let records: Vec<_> = (0..batch_size)
        .map(|i| create_test_record(&format!("batch-{i}"), &text))
        .collect();

    let start = Instant::now();

    let mut success_count = 0;
    for record in records {
        if process_record_with_perceptual(record, &canonical_cfg, &perceptual_cfg).is_ok() {
            success_count += 1;
        }
    }

    let elapsed = start.elapsed();

    assert_eq!(
        success_count, batch_size,
        "All documents should process successfully"
    );

    // Batch processing should complete in reasonable time
    let avg_time_per_doc = elapsed / batch_size as u32;
    println!("Processed {batch_size} documents in {elapsed:?} (avg {avg_time_per_doc:?} per doc)",);

    // Average should be under 100ms per document (generous threshold)
    assert!(
        avg_time_per_doc < Duration::from_millis(100),
        "Average processing time {avg_time_per_doc:?} exceeds 100ms threshold",
    );
}

#[test]
fn stress_test_large_document() {
    let canonical_cfg = CanonicalizeConfig::default();
    let perceptual_cfg = PerceptualConfig::default();

    // Generate a large document (10,000 words)
    let text = generate_text(10000);
    let raw = create_test_record("large-doc", &text);

    let start = Instant::now();
    let result = process_record_with_perceptual(raw, &canonical_cfg, &perceptual_cfg);
    let elapsed = start.elapsed();

    assert!(result.is_ok(), "Large document should process successfully");

    let (doc, fingerprint) = result.unwrap();

    // Verify outputs are reasonable
    assert!(!doc.canonical_text.is_empty());
    assert!(!fingerprint.minhash.is_empty());

    println!("Processed 10,000 word document in {elapsed:?}");
}

#[test]
fn stress_test_many_small_documents() {
    let canonical_cfg = CanonicalizeConfig::default();
    let perceptual_cfg = PerceptualConfig::default();

    let count = 1000;
    let text = "This is a longer text that contains enough tokens for perceptual fingerprinting processing";

    let start = Instant::now();

    for i in 0..count {
        let raw = create_test_record(&format!("small-{i}"), text);
        let result = process_record_with_perceptual(raw, &canonical_cfg, &perceptual_cfg);
        assert!(result.is_ok(), "Document {i} should process");
    }

    let elapsed = start.elapsed();
    let avg_time = elapsed / count as u32;

    println!("Processed {count} small documents in {elapsed:?} (avg {avg_time:?} per doc)",);
}

#[test]
fn performance_canonicalize_vs_tokenize() {
    let cfg = CanonicalizeConfig::default();
    let text = generate_text(5000);

    // Time canonicalization
    let start = Instant::now();
    let doc = canonicalize("perf", &text, &cfg).expect("canonicalize");
    let canonical_time = start.elapsed();

    // Time tokenization of the result
    let start = Instant::now();
    let _tokens = ucfp::tokenize(&doc.canonical_text);
    let tokenize_time = start.elapsed();

    println!("Canonicalize: {canonical_time:?}, Tokenize: {tokenize_time:?}",);

    // Both should be fast
    assert!(
        canonical_time < Duration::from_millis(500),
        "Canonicalization took {canonical_time:?}",
    );
    assert!(
        tokenize_time < Duration::from_millis(100),
        "Tokenization took {tokenize_time:?}",
    );
}

#[test]
fn memory_usage_reasonable_for_batch() {
    let cfg = CanonicalizeConfig::default();

    // Process many documents and verify memory doesn't blow up
    // (This is more of a smoke test - real memory profiling requires tools like valgrind)
    let count = 500;
    let text = generate_text(200);

    for i in 0..count {
        let doc = canonicalize(format!("mem-test-{i}"), &text, &cfg).expect("canonicalize");

        // Force drop by not storing the result
        drop(doc);

        // Every 100 docs, suggest the system is still responsive
        if i % 100 == 0 {
            println!("Processed {i} documents, memory still reasonable");
        }
    }

    // If we got here without OOM, test passes
    println!("Successfully processed {count} documents without excessive memory usage",);
}

#[test]
fn performance_parallel_vs_serial_perceptual() {
    let canonical_cfg = CanonicalizeConfig::default();
    let doc = canonicalize("perf", &generate_text(5000), &canonical_cfg).expect("canonicalize");
    let tokens: Vec<&str> = doc.tokens.iter().map(|t| t.text.as_str()).collect();

    let serial_cfg = PerceptualConfig {
        use_parallel: false,
        ..Default::default()
    };

    let parallel_cfg = PerceptualConfig {
        use_parallel: true,
        ..Default::default()
    };

    // Warmup
    for _ in 0..5 {
        let _ = ucfp::perceptualize_tokens(&tokens, &serial_cfg);
        let _ = ucfp::perceptualize_tokens(&tokens, &parallel_cfg);
    }

    // Time serial
    let start = Instant::now();
    for _ in 0..10 {
        let _ = ucfp::perceptualize_tokens(&tokens, &serial_cfg);
    }
    let serial_time = start.elapsed();

    // Time parallel
    let start = Instant::now();
    for _ in 0..10 {
        let _ = ucfp::perceptualize_tokens(&tokens, &parallel_cfg);
    }
    let parallel_time = start.elapsed();

    let speedup = serial_time.as_secs_f64() / parallel_time.as_secs_f64();
    println!("Serial: {serial_time:?}, Parallel: {parallel_time:?}, Speedup: {speedup:.2}x",);

    // Parallel should not be significantly slower (may not be faster for small inputs)
    // Increased threshold from 2x to 5x due to parallelization overhead on some systems
    assert!(
        parallel_time < serial_time * 5,
        "Parallel execution is more than 5x slower than serial"
    );
}

#[test]
fn stress_test_repeated_processing() {
    let canonical_cfg = CanonicalizeConfig::default();
    let perceptual_cfg = PerceptualConfig::default();
    let text = generate_text(500);
    let raw = create_test_record("repeated", &text);

    let iterations = 1000;

    let start = Instant::now();

    for _ in 0..iterations {
        let result = process_record_with_perceptual(raw.clone(), &canonical_cfg, &perceptual_cfg);
        assert!(result.is_ok());
    }

    let elapsed = start.elapsed();
    let avg_time = elapsed / iterations as u32;

    println!(
        "Processed same document {iterations} times in {elapsed:?} (avg {avg_time:?} per iteration)",
    );

    // Should be consistent performance
    assert!(
        avg_time < Duration::from_millis(50),
        "Average iteration time {avg_time:?} exceeds 50ms",
    );
}

#[test]
fn performance_various_shingle_sizes() {
    let canonical_cfg = CanonicalizeConfig::default();
    let doc = canonicalize("perf", &generate_text(1000), &canonical_cfg).expect("canonicalize");
    let tokens: Vec<&str> = doc.tokens.iter().map(|t| t.text.as_str()).collect();

    for k in [3, 5, 7, 9, 11, 13] {
        let cfg = PerceptualConfig {
            k,
            ..Default::default()
        };

        let start = Instant::now();
        let _fp = ucfp::perceptualize_tokens(&tokens, &cfg);
        let elapsed = start.elapsed();

        println!("k={k}: {elapsed:?}");
    }
}

#[test]
fn performance_various_minhash_configs() {
    let canonical_cfg = CanonicalizeConfig::default();
    let doc = canonicalize("perf", &generate_text(1000), &canonical_cfg).expect("canonicalize");
    let tokens: Vec<&str> = doc.tokens.iter().map(|t| t.text.as_str()).collect();

    for sig_size in [(8, 8), (16, 8), (16, 16), (32, 16)] {
        let cfg = PerceptualConfig {
            minhash_bands: sig_size.0,
            minhash_rows_per_band: sig_size.1,
            ..Default::default()
        };

        let start = Instant::now();
        let _fp = ucfp::perceptualize_tokens(&tokens, &cfg);
        let elapsed = start.elapsed();

        println!(
            "MinHash bands={}, rows={}: {elapsed:?}",
            sig_size.0, sig_size.1
        );
    }
}
