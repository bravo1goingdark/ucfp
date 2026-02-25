//! Determinism tests for UCFP pipeline
//!
//! Verifies that identical inputs produce identical outputs.

use ucfp::{
    canonicalize, process_pipeline, CanonicalizeConfig, IngestConfig, IngestMetadata,
    IngestPayload, IngestSource, PerceptualConfig, PipelineStageConfig, RawIngestRecord,
};

fn process_document(
    raw: RawIngestRecord,
    ingest_cfg: &IngestConfig,
    canonical_cfg: &CanonicalizeConfig,
    perceptual_cfg: &PerceptualConfig,
) -> Result<ucfp::PerceptualFingerprint, ucfp::PipelineError> {
    let (_, fingerprint, _) = process_pipeline(
        raw,
        PipelineStageConfig::Perceptual,
        ingest_cfg,
        canonical_cfg,
        Some(perceptual_cfg),
        None,
    )?;
    fingerprint.ok_or_else(|| {
        ucfp::PipelineError::Perceptual(ucfp::PerceptualError::InvalidConfigVersion { version: 0 })
    })
}

fn base_record(text: &str) -> RawIngestRecord {
    RawIngestRecord {
        id: "determinism-test".into(),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: Some("tenant".into()),
            doc_id: Some("doc".into()),
            received_at: None,
            original_source: None,
            attributes: None,
        },
        payload: Some(IngestPayload::Text(text.into())),
    }
}

/// Core determinism: same input = same output across multiple runs
#[test]
fn determinism_across_multiple_runs() {
    let canonical_cfg = CanonicalizeConfig::default();
    let ingest_cfg = IngestConfig::default();
    let perceptual_cfg = PerceptualConfig::default();
    let record = base_record(
        "The quick brown fox jumps over the lazy dog with sufficient tokens for processing",
    );

    let runs: Vec<_> = (0..5)
        .map(|_| {
            process_document(record.clone(), &ingest_cfg, &canonical_cfg, &perceptual_cfg)
                .expect("processing should succeed")
        })
        .collect();

    // All runs should produce identical fingerprints
    let first = &runs[0];
    for (i, fp) in runs.iter().enumerate().skip(1) {
        assert_eq!(
            first.minhash, fp.minhash,
            "Run {i} produced different fingerprint"
        );
    }
}

/// Whitespace normalization produces identical fingerprints
#[test]
fn determinism_whitespace_variations() {
    let canonical_cfg = CanonicalizeConfig::default();
    let ingest_cfg = IngestConfig::default();
    let perceptual_cfg = PerceptualConfig::default();

    let variations = [
        "The quick brown fox jumps over the lazy dog",
        "The quick  brown fox jumps over the lazy dog",
        " The quick brown fox jumps over the lazy dog ",
        "The quick\tbrown fox jumps over the lazy dog",
        "The quick\nbrown fox jumps over the lazy dog",
    ];

    let fingerprints: Vec<_> = variations
        .iter()
        .map(|text| {
            process_document(
                base_record(text),
                &ingest_cfg,
                &canonical_cfg,
                &perceptual_cfg,
            )
            .expect("processing should succeed")
        })
        .collect();

    let first = &fingerprints[0];
    for (i, fp) in fingerprints.iter().enumerate().skip(1) {
        assert_eq!(
            first.minhash, fp.minhash,
            "Whitespace variation {i} produced different fingerprint"
        );
    }
}

/// Case variations produce identical fingerprints (with default lowercase)
#[test]
fn determinism_case_insensitive() {
    let canonical_cfg = CanonicalizeConfig::default();
    let ingest_cfg = IngestConfig::default();
    let perceptual_cfg = PerceptualConfig::default();

    // Need at least 9 tokens for k=9
    let cases = [
        "The Quick Brown Fox jumps over the lazy dog and runs away",
        "the quick brown fox jumps over the lazy dog and runs away",
        "THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG AND RUNS AWAY",
    ];

    let fingerprints: Vec<_> = cases
        .iter()
        .map(|text| {
            process_document(
                base_record(text),
                &ingest_cfg,
                &canonical_cfg,
                &perceptual_cfg,
            )
            .expect("processing should succeed")
        })
        .collect();

    let first = &fingerprints[0];
    for (i, fp) in fingerprints.iter().enumerate().skip(1) {
        assert_eq!(
            first.minhash, fp.minhash,
            "Case variation {i} produced different fingerprint"
        );
    }
}

/// Unicode equivalence (NFC vs NFD) produces identical fingerprints
#[test]
fn determinism_unicode_equivalence() {
    let canonical_cfg = CanonicalizeConfig::default();
    let ingest_cfg = IngestConfig::default();
    let perceptual_cfg = PerceptualConfig::default();

    // Precomposed vs decomposed forms - need at least 9 tokens for k=9
    let precomposed = "Café au lait is a delicious drink with coffee and milk";
    let decomposed = "Cafe\u{0301} au lait is a delicious drink with coffee and milk"; // e + combining acute

    let fp1 = process_document(
        base_record(precomposed),
        &ingest_cfg,
        &canonical_cfg,
        &perceptual_cfg,
    )
    .expect("precomposed should work");

    let fp2 = process_document(
        base_record(decomposed),
        &ingest_cfg,
        &canonical_cfg,
        &perceptual_cfg,
    )
    .expect("decomposed should work");

    assert_eq!(
        fp1.minhash, fp2.minhash,
        "Unicode forms should produce same fingerprint"
    );
}

/// Different seeds produce different fingerprints
#[test]
fn determinism_different_seed_different_fingerprint() {
    let canonical_cfg = CanonicalizeConfig::default();
    let ingest_cfg = IngestConfig::default();
    let record = base_record("Testing with different seeds for determinism and using many tokens to ensure sufficient length");

    let cfg1 = PerceptualConfig {
        seed: 12345,
        ..Default::default()
    };
    let cfg2 = PerceptualConfig {
        seed: 54321,
        ..Default::default()
    };

    let fp1 = process_document(record.clone(), &ingest_cfg, &canonical_cfg, &cfg1).unwrap();
    let fp2 = process_document(record, &ingest_cfg, &canonical_cfg, &cfg2).unwrap();

    assert_ne!(
        fp1.minhash, fp2.minhash,
        "Different seeds should produce different fingerprints"
    );
}

/// Same seed produces same fingerprint across instances
#[test]
fn determinism_same_seed_consistent() {
    let canonical_cfg = CanonicalizeConfig::default();
    let ingest_cfg = IngestConfig::default();
    let record =
        base_record("Testing same seed consistency with many tokens to ensure sufficient length");

    let cfg = PerceptualConfig {
        seed: 42,
        ..Default::default()
    };

    let fp1 = process_document(record.clone(), &ingest_cfg, &canonical_cfg, &cfg).unwrap();
    let fp2 = process_document(record, &ingest_cfg, &canonical_cfg, &cfg).unwrap();

    assert_eq!(fp1.minhash, fp2.minhash);
    assert_eq!(fp1.meta.seed, 42);
}

/// Config version affects canonical hash
#[test]
fn determinism_config_version_tracking() {
    let cfg_v1 = CanonicalizeConfig {
        version: 1,
        ..Default::default()
    };
    let cfg_v2 = CanonicalizeConfig {
        version: 2,
        ..Default::default()
    };

    let doc_v1 = canonicalize("version-test", "Same text", &cfg_v1).expect("v1");
    let doc_v2 = canonicalize("version-test", "Same text", &cfg_v2).expect("v2");

    // Same text but different config versions should produce different hashes
    assert_ne!(doc_v1.sha256_hex, doc_v2.sha256_hex);
}
