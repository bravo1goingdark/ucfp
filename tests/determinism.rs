use ucfp::{
    process_pipeline, CanonicalizeConfig, IngestConfig, IngestMetadata, IngestPayload,
    IngestSource, PerceptualConfig, PipelineStageConfig, RawIngestRecord,
};

fn canonical_defaults() -> CanonicalizeConfig {
    CanonicalizeConfig::default()
}

fn ingest_defaults() -> IngestConfig {
    IngestConfig::default()
}

#[test]
fn fingerprints_equivalent_inputs_match() {
    let canonical_cfg = canonical_defaults();
    let ingest_cfg = ingest_defaults();
    let perceptual_cfg = PerceptualConfig {
        k: 2,
        ..Default::default()
    };

    let record_a = RawIngestRecord {
        id: "determinism-a".into(),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: Some("tenant-det".into()),
            doc_id: Some("doc-det".into()),
            received_at: None,
            original_source: None,
            attributes: None,
        },
        payload: Some(IngestPayload::Text(" Hello   world!  ".into())),
    };

    let record_b = RawIngestRecord {
        id: "determinism-b".into(),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: Some("tenant-det".into()),
            doc_id: Some("doc-det".into()),
            received_at: None,
            original_source: None,
            attributes: None,
        },
        payload: Some(IngestPayload::Text("hello WORLD!".into())),
    };

    // Use explicit configs to ensure deterministic defaults are exercised.
    let fp_a =
        process_document_with_all_configs(record_a, &ingest_cfg, &canonical_cfg, &perceptual_cfg)
            .expect("first fingerprint");
    let fp_b =
        process_document_with_all_configs(record_b, &ingest_cfg, &canonical_cfg, &perceptual_cfg)
            .expect("second fingerprint");

    assert_eq!(fp_a.minhash, fp_b.minhash);
    assert_eq!(fp_a.meta.config_version, fp_b.meta.config_version);
}

fn process_document_with_all_configs(
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

// Expanded determinism tests

#[test]
fn determinism_multiple_runs_same_input() {
    let canonical_cfg = canonical_defaults();
    let ingest_cfg = ingest_defaults();
    let perceptual_cfg = PerceptualConfig::default();

    let record = RawIngestRecord {
        id: "determinism-multi".into(),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: Some("tenant-det".into()),
            doc_id: Some("doc-det".into()),
            received_at: None,
            original_source: None,
            attributes: None,
        },
        payload: Some(IngestPayload::Text(
            "The quick brown fox jumps over the lazy dog.".into(),
        )),
    };

    // Run multiple times and verify identical results
    let fp1 = process_document_with_all_configs(
        record.clone(),
        &ingest_cfg,
        &canonical_cfg,
        &perceptual_cfg,
    )
    .expect("first run");

    let fp2 = process_document_with_all_configs(
        record.clone(),
        &ingest_cfg,
        &canonical_cfg,
        &perceptual_cfg,
    )
    .expect("second run");

    let fp3 =
        process_document_with_all_configs(record, &ingest_cfg, &canonical_cfg, &perceptual_cfg)
            .expect("third run");

    assert_eq!(fp1.minhash, fp2.minhash);
    assert_eq!(fp2.minhash, fp3.minhash);
}

#[test]
fn determinism_unicode_equivalence() {
    let canonical_cfg = canonical_defaults();
    let ingest_cfg = ingest_defaults();
    let perceptual_cfg = PerceptualConfig::default();

    // Precomposed vs decomposed forms of the same text
    let record_precomposed = RawIngestRecord {
        id: "unicode-precomposed".into(),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: Some("tenant-uni".into()),
            doc_id: Some("doc-uni".into()),
            received_at: None,
            original_source: None,
            attributes: None,
        },
        payload: Some(IngestPayload::Text(
            "Café au lait is a delicious French coffee drink with milk".into(),
        )), // U+00E9
    };

    let record_decomposed = RawIngestRecord {
        id: "unicode-decomposed".into(),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: Some("tenant-uni".into()),
            doc_id: Some("doc-uni".into()),
            received_at: None,
            original_source: None,
            attributes: None,
        },
        // "Cafe" + combining acute accent (U+0301)
        payload: Some(IngestPayload::Text(
            "Cafe\u{0301} au lait is a delicious French coffee drink with milk".into(),
        )),
    };

    let fp_precomposed = process_document_with_all_configs(
        record_precomposed,
        &ingest_cfg,
        &canonical_cfg,
        &perceptual_cfg,
    )
    .expect("precomposed");

    let fp_decomposed = process_document_with_all_configs(
        record_decomposed,
        &ingest_cfg,
        &canonical_cfg,
        &perceptual_cfg,
    )
    .expect("decomposed");

    assert_eq!(fp_precomposed.minhash, fp_decomposed.minhash);
}

#[test]
fn determinism_different_seed_different_fingerprint() {
    let canonical_cfg = canonical_defaults();
    let ingest_cfg = ingest_defaults();

    let record = RawIngestRecord {
        id: "determinism-seed".into(),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: Some("tenant-seed".into()),
            doc_id: Some("doc-seed".into()),
            received_at: None,
            original_source: None,
            attributes: None,
        },
        payload: Some(IngestPayload::Text(
            "This is the same text used with different random seeds for testing determinism".into(),
        )),
    };

    let perceptual_cfg_1 = PerceptualConfig {
        seed: 12345,
        ..Default::default()
    };

    let perceptual_cfg_2 = PerceptualConfig {
        seed: 54321,
        ..Default::default()
    };

    let fp1 = process_document_with_all_configs(
        record.clone(),
        &ingest_cfg,
        &canonical_cfg,
        &perceptual_cfg_1,
    )
    .expect("first seed");

    let fp2 = process_document_with_all_configs(
        record.clone(),
        &ingest_cfg,
        &canonical_cfg,
        &perceptual_cfg_2,
    )
    .expect("second seed");

    // Different seeds should produce different fingerprints
    assert_ne!(fp1.minhash, fp2.minhash);
}

#[test]
fn determinism_same_seed_same_fingerprint() {
    let canonical_cfg = canonical_defaults();
    let ingest_cfg = ingest_defaults();
    let seed = 42_u64;

    let record = RawIngestRecord {
        id: "determinism-same-seed".into(),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: Some("tenant-same".into()),
            doc_id: Some("doc-same".into()),
            received_at: None,
            original_source: None,
            attributes: None,
        },
        payload: Some(IngestPayload::Text(
            "Testing perceptual fingerprinting with a specific random seed value for determinism"
                .into(),
        )),
    };

    let perceptual_cfg = PerceptualConfig {
        seed,
        ..Default::default()
    };

    let fp1 = process_document_with_all_configs(
        record.clone(),
        &ingest_cfg,
        &canonical_cfg,
        &perceptual_cfg,
    )
    .expect("first run");

    let fp2 =
        process_document_with_all_configs(record, &ingest_cfg, &canonical_cfg, &perceptual_cfg)
            .expect("second run");

    assert_eq!(fp1.minhash, fp2.minhash);
    assert_eq!(fp1.meta.seed, fp2.meta.seed);
    assert_eq!(fp1.meta.seed, seed);
}

#[test]
fn determinism_whitespace_variations() {
    let canonical_cfg = canonical_defaults();
    let ingest_cfg = ingest_defaults();
    let perceptual_cfg = PerceptualConfig::default();

    let variations = [
        "The quick brown fox jumps over the lazy dog",
        "The quick  brown fox jumps over the lazy dog",
        "The quick   brown fox jumps over the lazy dog",
        " The quick brown fox jumps over the lazy dog ",
        "  The quick   brown fox jumps over the lazy dog  ",
        "The quick\tbrown fox jumps over the lazy dog",
        "The quick\nbrown fox jumps over the lazy dog",
        "The quick\r\nbrown fox jumps over the lazy dog",
    ];

    let mut fingerprints = Vec::new();

    for (i, text) in variations.iter().enumerate() {
        let record = RawIngestRecord {
            id: format!("whitespace-{i}"),
            source: IngestSource::RawText,
            metadata: IngestMetadata {
                tenant_id: Some("tenant-ws".into()),
                doc_id: Some("doc-ws".into()),
                received_at: None,
                original_source: None,
                attributes: None,
            },
            payload: Some(IngestPayload::Text(text.to_string())),
        };

        let fp =
            process_document_with_all_configs(record, &ingest_cfg, &canonical_cfg, &perceptual_cfg)
                .unwrap_or_else(|_| panic!("processing variation {i}"));

        fingerprints.push(fp.minhash.clone());
    }

    // All whitespace variations should produce the same fingerprint
    let first = &fingerprints[0];
    for (i, fp) in fingerprints.iter().enumerate().skip(1) {
        assert_eq!(
            first, fp,
            "Whitespace variation {i} should match first fingerprint",
        );
    }
}

#[test]
fn determinism_case_insensitive() {
    let canonical_cfg = canonical_defaults();
    let ingest_cfg = ingest_defaults();
    let perceptual_cfg = PerceptualConfig::default();

    let cases = [
        "The Quick Brown Fox Jumps Over The Lazy Dog",
        "the quick brown fox jumps over the lazy dog",
        "THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG",
        "ThE QuIcK BrOwN FoX JuMpS OvEr ThE LaZy DoG",
        "tHe qUiCk bRoWn fOx jUmPs oVeR tHe lAzY dOg",
    ];

    let mut fingerprints = Vec::new();

    for (i, text) in cases.iter().enumerate() {
        let record = RawIngestRecord {
            id: format!("case-{i}"),
            source: IngestSource::RawText,
            metadata: IngestMetadata {
                tenant_id: Some("tenant-case".into()),
                doc_id: Some("doc-case".into()),
                received_at: None,
                original_source: None,
                attributes: None,
            },
            payload: Some(IngestPayload::Text(text.to_string())),
        };

        let fp =
            process_document_with_all_configs(record, &ingest_cfg, &canonical_cfg, &perceptual_cfg)
                .unwrap_or_else(|_| panic!("processing case {i}"));

        fingerprints.push(fp.minhash.clone());
    }

    // All case variations should produce the same fingerprint (with default lowercase config)
    let first = &fingerprints[0];
    for (i, fp) in fingerprints.iter().enumerate().skip(1) {
        assert_eq!(
            first, fp,
            "Case variation {i} should match first fingerprint",
        );
    }
}

#[test]
fn determinism_config_version_tracking() {
    let canonical_cfg_v1 = CanonicalizeConfig {
        version: 1,
        ..Default::default()
    };
    let canonical_cfg_v2 = CanonicalizeConfig {
        version: 2,
        ..Default::default()
    };
    let _ingest_cfg = ingest_defaults();
    let _perceptual_cfg = PerceptualConfig::default();

    let _record = RawIngestRecord {
        id: "determinism-version".into(),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: Some("tenant-ver".into()),
            doc_id: Some("doc-ver".into()),
            received_at: None,
            original_source: None,
            attributes: None,
        },
        payload: Some(IngestPayload::Text("Version tracking test".into())),
    };

    let doc_v1 = ucfp::canonicalize("version-test", "Version tracking test", &canonical_cfg_v1)
        .expect("canonicalize v1");

    let doc_v2 = ucfp::canonicalize("version-test", "Version tracking test", &canonical_cfg_v2)
        .expect("canonicalize v2");

    // Same text but different config versions should produce different hashes
    assert_ne!(doc_v1.sha256_hex, doc_v2.sha256_hex);
    assert_eq!(doc_v1.canonical_version, 1);
    assert_eq!(doc_v2.canonical_version, 2);
}
