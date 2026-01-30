use ucfp::{
    process_document, process_record_with_perceptual_configs, CanonicalizeConfig, IngestConfig,
    IngestError, IngestMetadata, IngestPayload, IngestSource, PerceptualConfig, PerceptualError,
    PipelineError, RawIngestRecord,
};

fn base_metadata() -> IngestMetadata {
    IngestMetadata {
        tenant_id: Some("tenant-err".into()),
        doc_id: Some("doc-err".into()),
        received_at: None,
        original_source: None,
        attributes: None,
    }
}

#[test]
fn empty_text_payload_returns_ingest_error() {
    let raw = RawIngestRecord {
        id: "err-empty".into(),
        source: IngestSource::RawText,
        metadata: base_metadata(),
        payload: Some(IngestPayload::Text("   ".into())),
    };

    let result = process_document(raw, &PerceptualConfig::default());
    assert!(matches!(
        result,
        Err(PipelineError::Ingest(IngestError::EmptyNormalizedText))
    ));
}

#[test]
fn binary_payload_for_file_source_is_rejected_by_canonical_stage() {
    let raw = RawIngestRecord {
        id: "err-binary".into(),
        source: IngestSource::File {
            filename: "image.bin".into(),
            content_type: Some("application/octet-stream".into()),
        },
        metadata: base_metadata(),
        payload: Some(IngestPayload::Binary(vec![0, 1, 2])),
    };

    let result = process_document(raw, &PerceptualConfig::default());
    assert!(matches!(result, Err(PipelineError::NonTextPayload)));
}

#[test]
fn perceptual_invalid_config_bubbles_up() {
    let raw = RawIngestRecord {
        id: "err-perceptual".into(),
        source: IngestSource::RawText,
        metadata: base_metadata(),
        payload: Some(IngestPayload::Text(
            "The quick brown fox jumps over the lazy dog".into(),
        )),
    };

    let ingest_cfg = IngestConfig::default();
    let canonical_cfg = CanonicalizeConfig::default();
    let perceptual_cfg = PerceptualConfig {
        k: 0,
        ..Default::default()
    };

    let result =
        process_record_with_perceptual_configs(raw, &ingest_cfg, &canonical_cfg, &perceptual_cfg);

    assert!(matches!(
        result,
        Err(PipelineError::Perceptual(
            PerceptualError::InvalidConfigK { .. }
        ))
    ));
}

// Expanded error handling tests

#[test]
fn error_empty_normalized_text_various_whitespace() {
    let whitespace_variations = vec![
        "", " ", "  ", "   ", "\t", "\n", "\r\n", " \t \n ", "\t\t\t",
    ];

    for ws in whitespace_variations {
        let raw = RawIngestRecord {
            id: format!("err-ws-{}", ws.len()),
            source: IngestSource::RawText,
            metadata: base_metadata(),
            payload: Some(IngestPayload::Text(ws.into())),
        };

        let result = process_document(raw, &PerceptualConfig::default());
        assert!(
            matches!(
                result,
                Err(PipelineError::Ingest(IngestError::EmptyNormalizedText))
            ),
            "Should error on whitespace: {ws:?}",
        );
    }
}

#[test]
fn error_missing_payload_for_text_source() {
    let raw = RawIngestRecord {
        id: "err-missing-payload".into(),
        source: IngestSource::RawText,
        metadata: base_metadata(),
        payload: None,
    };

    let result = process_document(raw, &PerceptualConfig::default());
    assert!(matches!(
        result,
        Err(PipelineError::Ingest(IngestError::MissingPayload))
    ));
}

#[test]
fn error_perceptual_config_k_too_large() {
    let raw = RawIngestRecord {
        id: "err-k-large".into(),
        source: IngestSource::RawText,
        metadata: base_metadata(),
        payload: Some(IngestPayload::Text("Short".into())),
    };

    let ingest_cfg = IngestConfig::default();
    let canonical_cfg = CanonicalizeConfig::default();
    let perceptual_cfg = PerceptualConfig {
        k: 100, // Larger than token count
        ..Default::default()
    };

    let result =
        process_record_with_perceptual_configs(raw, &ingest_cfg, &canonical_cfg, &perceptual_cfg);

    assert!(matches!(
        result,
        Err(PipelineError::Perceptual(
            PerceptualError::NotEnoughTokens { .. }
        ))
    ));
}

#[test]
fn error_perceptual_config_w_zero() {
    let raw = RawIngestRecord {
        id: "err-w-zero".into(),
        source: IngestSource::RawText,
        metadata: base_metadata(),
        payload: Some(IngestPayload::Text(
            "The quick brown fox jumps over the lazy dog and runs through the forest".into(),
        )),
    };

    let ingest_cfg = IngestConfig::default();
    let canonical_cfg = CanonicalizeConfig::default();
    let perceptual_cfg = PerceptualConfig {
        k: 3,
        w: 0, // Invalid
        ..Default::default()
    };

    let result =
        process_record_with_perceptual_configs(raw, &ingest_cfg, &canonical_cfg, &perceptual_cfg);

    assert!(matches!(
        result,
        Err(PipelineError::Perceptual(
            PerceptualError::InvalidConfigW { .. }
        ))
    ));
}

#[test]
fn error_perceptual_config_version_zero() {
    let raw = RawIngestRecord {
        id: "err-version-zero".into(),
        source: IngestSource::RawText,
        metadata: base_metadata(),
        payload: Some(IngestPayload::Text(
            "The quick brown fox jumps over the lazy dog".into(),
        )),
    };

    let ingest_cfg = IngestConfig::default();
    let canonical_cfg = CanonicalizeConfig::default();
    let perceptual_cfg = PerceptualConfig {
        version: 0, // Reserved
        ..Default::default()
    };

    let result =
        process_record_with_perceptual_configs(raw, &ingest_cfg, &canonical_cfg, &perceptual_cfg);

    assert!(matches!(
        result,
        Err(PipelineError::Perceptual(
            PerceptualError::InvalidConfigVersion { .. }
        ))
    ));
}

#[test]
fn error_canonical_config_version_zero() {
    let result = ucfp::canonicalize(
        "test-doc",
        "Some valid text",
        &CanonicalizeConfig {
            version: 0,
            ..Default::default()
        },
    );

    assert!(matches!(
        result,
        Err(ucfp::CanonicalError::InvalidConfig(_))
    ));
}

#[test]
fn error_canonical_empty_doc_id() {
    let result = ucfp::canonicalize("", "Some valid text", &CanonicalizeConfig::default());

    assert!(matches!(result, Err(ucfp::CanonicalError::MissingDocId)));
}

#[test]
fn error_canonical_whitespace_only_doc_id() {
    let result = ucfp::canonicalize("   ", "Some valid text", &CanonicalizeConfig::default());

    assert!(matches!(result, Err(ucfp::CanonicalError::MissingDocId)));
}

#[test]
fn error_canonical_empty_input() {
    let result = ucfp::canonicalize("test-doc", "", &CanonicalizeConfig::default());

    assert!(matches!(result, Err(ucfp::CanonicalError::EmptyInput)));
}

#[test]
fn error_url_source_without_url() {
    let raw = RawIngestRecord {
        id: "err-url".into(),
        source: IngestSource::Url("".into()),
        metadata: base_metadata(),
        payload: Some(IngestPayload::Text("This is sample content for testing URL source processing with sufficient tokens for fingerprinting".into())),
    };

    // Should still work, URL source doesn't validate URL format
    let result = process_document(raw, &PerceptualConfig::default());
    assert!(result.is_ok());
}

#[test]
fn error_binary_empty_payload() {
    let raw = RawIngestRecord {
        id: "err-empty-binary".into(),
        source: IngestSource::File {
            filename: "empty.bin".into(),
            content_type: Some("application/octet-stream".into()),
        },
        metadata: base_metadata(),
        payload: Some(IngestPayload::Binary(vec![])),
    };

    let result = process_document(raw, &PerceptualConfig::default());
    // Empty binary payloads might be handled differently depending on implementation
    // This test documents the current behavior
    assert!(
        result.is_err() || result.is_ok(),
        "Documenting behavior for empty binary payload"
    );
}

#[test]
fn error_pipeline_error_display() {
    let ingest_err = PipelineError::Ingest(IngestError::EmptyNormalizedText);
    let perceptual_err = PipelineError::Perceptual(PerceptualError::InvalidConfigK { k: 0 });

    // Verify error messages are meaningful
    let ingest_msg = format!("{ingest_err}");
    let perceptual_msg = format!("{perceptual_err}");

    assert!(!ingest_msg.is_empty());
    assert!(!perceptual_msg.is_empty());
}

#[test]
fn error_ingest_error_variants() {
    // Test all ingest error variants can be created and displayed
    let errors = vec![
        IngestError::EmptyNormalizedText,
        IngestError::InvalidMetadata("test".into()),
        IngestError::InvalidUtf8("invalid utf8".into()),
        IngestError::MissingPayload,
        IngestError::EmptyBinaryPayload,
        IngestError::PayloadTooLarge("payload too large".into()),
    ];

    for err in errors {
        let msg = format!("{err}");
        assert!(!msg.is_empty(), "Error variant should have display message");
    }
}
