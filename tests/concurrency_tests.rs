//! Concurrency and thread safety tests for UCFP

#![allow(clippy::type_complexity)]
#![allow(static_mut_refs)]

use std::sync::{Arc, Mutex};
use std::thread;
use ucfp::{
    canonicalize, process_pipeline, CanonicalizeConfig, IngestConfig, IngestMetadata,
    IngestPayload, IngestSource, PerceptualConfig, PipelineStageConfig, RawIngestRecord,
};

fn create_test_record(id: &str, text: &str) -> RawIngestRecord {
    RawIngestRecord {
        id: id.into(),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: Some("concurrent-tenant".into()),
            doc_id: Some(format!("concurrent-doc-{id}")),
            received_at: None,
            original_source: None,
            attributes: None,
        },
        payload: Some(IngestPayload::Text(text.into())),
    }
}

#[test]
fn concurrent_canonicalize_same_config() {
    let cfg = CanonicalizeConfig::default();
    let text = "Concurrent canonicalization test text";
    let config = Arc::new(cfg);

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let config = Arc::clone(&config);
            let text = text.to_string();
            thread::spawn(move || {
                canonicalize(format!("thread-{i}"), &text, &config)
                    .expect("canonicalize should succeed")
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // All threads should produce the same canonical text
    let first = &results[0];
    for (i, result) in results.iter().enumerate().skip(1) {
        assert_eq!(
            first.canonical_text, result.canonical_text,
            "Thread {i} produced different canonical text",
        );
        assert_eq!(
            first.sha256_hex, result.sha256_hex,
            "Thread {i} produced different hash",
        );
    }
}

#[test]
fn concurrent_perceptualize_same_config() {
    let canonical_cfg = CanonicalizeConfig::default();
    let perceptual_cfg = PerceptualConfig::default();
    let doc = canonicalize(
        "concurrent",
        "This is a test document with sufficient length to ensure there are enough tokens for perceptual fingerprint generation with a k value of nine shingles",
        &canonical_cfg,
    )
    .expect("canonicalize");
    let tokens: Vec<&str> = doc.tokens.iter().map(|t| t.text.as_str()).collect();

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let tokens: Vec<String> = tokens.iter().map(|&s| s.to_string()).collect();
            let cfg = perceptual_cfg.clone();
            thread::spawn(move || {
                let token_refs: Vec<&str> = tokens.iter().map(|s| s.as_str()).collect();
                ucfp::perceptualize_tokens(&token_refs, &cfg).expect("perceptualize should succeed")
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // All threads should produce identical fingerprints
    let first = &results[0];
    for (i, result) in results.iter().enumerate().skip(1) {
        assert_eq!(
            first.minhash, result.minhash,
            "Thread {i} produced different MinHash",
        );
    }
}

#[test]
fn concurrent_pipeline_processing() {
    let canonical_cfg = CanonicalizeConfig::default();
    let perceptual_cfg = PerceptualConfig::default();

    let texts: Vec<String> = (0..20)
        .map(|i| format!("Document number {i} contains a substantial amount of text content to ensure proper tokenization and perceptual fingerprint generation with sufficient tokens for the default shingle size",))
        .collect();

    let handles: Vec<_> = texts
        .into_iter()
        .enumerate()
        .map(|(i, text)| {
            let canonical_cfg = canonical_cfg.clone();
            let perceptual_cfg = perceptual_cfg.clone();
            thread::spawn(move || {
                let record = create_test_record(&format!("concurrent-{i}"), &text);
                let (doc, fingerprint, _) = process_pipeline(
                    record,
                    PipelineStageConfig::Perceptual,
                    &IngestConfig::default(),
                    &canonical_cfg,
                    Some(&perceptual_cfg),
                    None,
                )
                .expect("process should succeed");
                (doc, fingerprint.unwrap())
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // Verify all succeeded and produced valid outputs
    for (i, (doc, fingerprint)) in results.iter().enumerate() {
        assert!(
            !doc.canonical_text.is_empty(),
            "Thread {i} produced empty canonical text",
        );
        assert!(
            !fingerprint.minhash.is_empty(),
            "Thread {i} produced empty MinHash",
        );
    }
}

#[test]
fn thread_safe_shared_config() {
    // Test that configs can be safely shared across threads
    let canonical_cfg = Arc::new(CanonicalizeConfig::default());
    let perceptual_cfg = Arc::new(PerceptualConfig::default());

    let results = Arc::new(Mutex::new(Vec::new()));

    let handles: Vec<_> = (0..20)
        .map(|i| {
            let canonical_cfg = Arc::clone(&canonical_cfg);
            let perceptual_cfg = Arc::clone(&perceptual_cfg);
            let results = Arc::clone(&results);
            let text = format!("Shared config test document number {i} with extended text content to provide sufficient tokens for perceptual fingerprint generation using the default k value of nine shingles");

            thread::spawn(move || {
                let record = create_test_record(&format!("shared-{i}"), &text);
                let result = process_pipeline(
                    record,
                    PipelineStageConfig::Perceptual,
                    &IngestConfig::default(),
                    &canonical_cfg,
                    Some(&perceptual_cfg),
                    None,
                );

                results.lock().unwrap().push((i, result.is_ok()));
                result.map(|(doc, fp, _)| (doc, fp.unwrap()))
            })
        })
        .collect();

    // Wait for all threads
    for handle in handles {
        let _ = handle.join();
    }

    // Verify all succeeded
    let final_results = results.lock().unwrap();
    for (i, success) in final_results.iter() {
        assert!(*success, "Thread {i} failed");
    }

    assert_eq!(final_results.len(), 20);
}

#[test]
fn no_data_races_on_independent_documents() {
    // Process many documents concurrently to check for data races
    let num_threads = 50;
    let docs_per_thread = 10;

    let canonical_cfg = CanonicalizeConfig::default();
    let perceptual_cfg = PerceptualConfig::default();

    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let canonical_cfg = canonical_cfg.clone();
            let perceptual_cfg = perceptual_cfg.clone();

            thread::spawn(move || {
                let mut local_results = Vec::new();

                for doc_id in 0..docs_per_thread {
                    let text = format!(
                        "Thread {thread_id} Document {doc_id} with unique content to avoid collisions",
                    );
                    let record =
                        create_test_record(&format!("thread-{thread_id}-doc-{doc_id}"), &text);

                    let result = process_pipeline(
                        record,
                        PipelineStageConfig::Perceptual,
                        &IngestConfig::default(),
                        &canonical_cfg,
                        Some(&perceptual_cfg),
                        None,
                    );

                    local_results.push((thread_id, doc_id, result.is_ok()));
                }

                local_results
            })
        })
        .collect();

    let all_results: Vec<_> = handles
        .into_iter()
        .flat_map(|h| h.join().unwrap())
        .collect();

    // Verify all documents processed successfully
    let success_count = all_results
        .iter()
        .filter(|(_, _, success)| *success)
        .count();
    let total = num_threads * docs_per_thread;

    assert_eq!(
        success_count, total,
        "Only {success_count}/{total} documents processed successfully",
    );

    println!("Successfully processed {total} documents across {num_threads} threads",);
}

#[test]
fn concurrent_tokenization() {
    let text = "The quick brown fox jumps over the lazy dog. This is a test.".to_string();

    let handles: Vec<_> = (0..20)
        .map(|i| {
            let text = text.clone();
            thread::spawn(move || {
                let tokens = ucfp::tokenize(&text);
                (i, tokens.len())
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // All threads should get the same number of tokens
    let first_count = results[0].1;
    for (i, count) in results.iter().skip(1) {
        assert_eq!(
            first_count, *count,
            "Thread {i} got {count} tokens instead of {first_count}",
        );
    }
}

#[test]
fn thread_safe_singleton_config_access() {
    // Use global/config with_once patterns safely
    use std::sync::Once;

    static INIT: Once = Once::new();
    static mut CONFIG: Option<CanonicalizeConfig> = None;

    INIT.call_once(|| unsafe {
        CONFIG = Some(CanonicalizeConfig::default());
    });

    let handles: Vec<_> = (0..10)
        .map(|i| {
            thread::spawn(move || {
                // Safe to read after initialization
                let cfg = unsafe { CONFIG.as_ref().unwrap() };
                canonicalize(format!("singleton-{i}"), "Test text", cfg).expect("canonicalize")
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // All should succeed
    assert_eq!(results.len(), 10);
}

#[test]
fn concurrent_read_during_processing() {
    // Simulate read-heavy workload while processing
    let canonical_cfg = Arc::new(CanonicalizeConfig::default());
    let text = Arc::new("Test text for concurrent read".to_string());

    // Spawn readers
    let reader_handles: Vec<_> = (0..10)
        .map(|_| {
            let text = Arc::clone(&text);
            thread::spawn(move || {
                for _ in 0..100 {
                    let _ = text.len(); // Simulate read
                    thread::yield_now();
                }
            })
        })
        .collect();

    // Spawn processors
    let processor_handles: Vec<_> = (0..5)
        .map(|i| {
            let cfg = Arc::clone(&canonical_cfg);
            let text = Arc::clone(&text);
            thread::spawn(move || {
                canonicalize(format!("reader-test-{i}"), &text, &cfg).expect("canonicalize")
            })
        })
        .collect();

    // Wait for all
    for h in reader_handles {
        h.join().unwrap();
    }

    let results: Vec<_> = processor_handles
        .into_iter()
        .map(|h| h.join().unwrap())
        .collect();

    // All processors should succeed despite concurrent reads
    assert_eq!(results.len(), 5);
}

#[test]
fn stress_test_thread_pool_simulation() {
    use std::sync::mpsc;

    let (tx, rx): (
        mpsc::Sender<(usize, String)>,
        mpsc::Receiver<(usize, String)>,
    ) = mpsc::channel();
    let rx = Arc::new(Mutex::new(rx));
    let canonical_cfg = CanonicalizeConfig::default();
    let perceptual_cfg = PerceptualConfig::default();

    // Simulate a thread pool
    let num_workers = 4;
    let work_items = 100;

    let worker_handles: Vec<_> = (0..num_workers)
        .map(|worker_id| {
            let rx = Arc::clone(&rx);
            let canonical_cfg = canonical_cfg.clone();
            let perceptual_cfg = perceptual_cfg.clone();

            thread::spawn(move || {
                let mut processed = 0;
                loop {
                    let msg = rx.lock().unwrap().recv();
                    match msg {
                        Ok((id, text)) => {
                            let record =
                                create_test_record(&format!("pool-{worker_id}-{id}"), &text);
                            if process_pipeline(
                                record,
                                PipelineStageConfig::Perceptual,
                                &IngestConfig::default(),
                                &canonical_cfg,
                                Some(&perceptual_cfg),
                                None,
                            )
                            .is_ok()
                            {
                                processed += 1;
                            }
                        }
                        Err(_) => break,
                    }
                }
                processed
            })
        })
        .collect();

    // Send work
    for i in 0..work_items {
        let text = format!("Work item number {i} with substantial text content to ensure enough tokens exist for perceptual fingerprint generation with the default k value of nine shingles");
        tx.send((i, text)).expect("send work");
    }

    // Close channel
    drop(tx);

    // Collect results
    let total_processed: usize = worker_handles.into_iter().map(|h| h.join().unwrap()).sum();

    assert_eq!(
        total_processed, work_items,
        "Only processed {total_processed}/{work_items} items",
    );

    println!("Thread pool simulation: {num_workers} workers processed {total_processed} items",);
}
