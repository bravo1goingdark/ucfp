//! Error handling tests for UCFP pipeline

use ucfp::{
    canonicalize, process_pipeline, CanonicalizeConfig, IngestConfig, IngestError, IngestMetadata,
    IngestPayload, IngestSource, PerceptualConfig, PerceptualError, PipelineError,
    PipelineStageConfig, RawIngestRecord,
};

fn base_metadata() -> IngestMetadata {
    IngestMetadata {
        tenant_id: Some("tenant".into()),
        doc_id: Some("doc".into()),
        received_at: None,
        original_source: None,
        attributes: None,
    }
}

fn base_record(payload: IngestPayload) -> RawIngestRecord {
    RawIngestRecord {
        id: "test".into(),
        source: IngestSource::RawText,
        metadata: base_metadata(),
        payload: Some(payload),
    }
}

/// Ingest validation errors
#[test]
fn ingest_validation_errors() {
    let cfg = IngestConfig::default();

    // Empty/whitespace text should fail
    for text in ["", " ", "  ", "\t", "\n", " \t \n "] {
        let record = base_record(IngestPayload::Text(text.into()));
        let result = process_pipeline(
            record,
            PipelineStageConfig::Perceptual,
            &cfg,
            &CanonicalizeConfig::default(),
            Some(&PerceptualConfig::default()),
            None,
        );
        assert!(
            matches!(
                result,
                Err(PipelineError::Ingest(IngestError::EmptyNormalizedText))
            ),
            "Should error on whitespace: {text:?}"
        );
    }

    // Missing payload should fail
    let record = RawIngestRecord {
        id: "test".into(),
        source: IngestSource::RawText,
        metadata: base_metadata(),
        payload: None,
    };
    let result = process_pipeline(
        record,
        PipelineStageConfig::Perceptual,
        &cfg,
        &CanonicalizeConfig::default(),
        Some(&PerceptualConfig::default()),
        None,
    );
    assert!(matches!(
        result,
        Err(PipelineError::Ingest(IngestError::MissingPayload))
    ));
}

/// Binary payload rejected by canonical stage
#[test]
fn binary_payload_rejected() {
    let record = RawIngestRecord {
        id: "test".into(),
        source: IngestSource::File {
            filename: "image.bin".into(),
            content_type: Some("application/octet-stream".into()),
        },
        metadata: base_metadata(),
        payload: Some(IngestPayload::Binary(vec![0, 1, 2])),
    };

    let result = process_pipeline(
        record,
        PipelineStageConfig::Perceptual,
        &IngestConfig::default(),
        &CanonicalizeConfig::default(),
        Some(&PerceptualConfig::default()),
        None,
    );
    assert!(matches!(result, Err(PipelineError::NonTextPayload)));
}

/// Perceptual config validation: k=0
#[test]
fn perceptual_config_k_zero() {
    let text = "The quick brown fox jumps over the lazy dog with enough tokens for processing";
    let record = base_record(IngestPayload::Text(text.into()));
    let cfg = PerceptualConfig {
        k: 0,
        ..Default::default()
    };

    let result = process_pipeline(
        record,
        PipelineStageConfig::Perceptual,
        &IngestConfig::default(),
        &CanonicalizeConfig::default(),
        Some(&cfg),
        None,
    );
    assert!(matches!(
        result,
        Err(PipelineError::Perceptual(PerceptualError::InvalidConfigK {
            k: 0
        }))
    ));
}

/// Perceptual config validation: w=0
#[test]
fn perceptual_config_w_zero() {
    let text = "The quick brown fox jumps over the lazy dog with enough tokens for processing";
    let record = base_record(IngestPayload::Text(text.into()));
    let cfg = PerceptualConfig {
        k: 3,
        w: 0,
        ..Default::default()
    };

    let result = process_pipeline(
        record,
        PipelineStageConfig::Perceptual,
        &IngestConfig::default(),
        &CanonicalizeConfig::default(),
        Some(&cfg),
        None,
    );
    assert!(matches!(
        result,
        Err(PipelineError::Perceptual(PerceptualError::InvalidConfigW {
            w: 0
        }))
    ));
}

/// Perceptual config validation: version=0
#[test]
fn perceptual_config_version_zero() {
    let text = "The quick brown fox jumps over the lazy dog with enough tokens for processing";
    let record = base_record(IngestPayload::Text(text.into()));
    let cfg = PerceptualConfig {
        version: 0,
        ..Default::default()
    };

    let result = process_pipeline(
        record,
        PipelineStageConfig::Perceptual,
        &IngestConfig::default(),
        &CanonicalizeConfig::default(),
        Some(&cfg),
        None,
    );
    assert!(matches!(
        result,
        Err(PipelineError::Perceptual(
            PerceptualError::InvalidConfigVersion { .. }
        ))
    ));
}

/// Perceptual config validation: k too large
#[test]
fn perceptual_config_k_too_large() {
    let text = "Short text"; // Not enough tokens for k=100
    let record = base_record(IngestPayload::Text(text.into()));
    let cfg = PerceptualConfig {
        k: 100,
        ..Default::default()
    };

    let result = process_pipeline(
        record,
        PipelineStageConfig::Perceptual,
        &IngestConfig::default(),
        &CanonicalizeConfig::default(),
        Some(&cfg),
        None,
    );
    assert!(matches!(
        result,
        Err(PipelineError::Perceptual(
            PerceptualError::NotEnoughTokens { .. }
        ))
    ));
}

/// Canonical config validation errors
#[test]
fn canonical_config_validation() {
    // Version 0 is invalid
    let result = canonicalize(
        "test",
        "Some text",
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

/// Canonical empty/invalid input errors
#[test]
fn canonical_input_validation() {
    // Empty doc_id
    let result = canonicalize("", "text", &CanonicalizeConfig::default());
    assert!(matches!(result, Err(ucfp::CanonicalError::MissingDocId)));

    // Whitespace-only doc_id
    let result = canonicalize("   ", "text", &CanonicalizeConfig::default());
    assert!(matches!(result, Err(ucfp::CanonicalError::MissingDocId)));

    // Empty input text
    let result = canonicalize("doc", "", &CanonicalizeConfig::default());
    assert!(matches!(result, Err(ucfp::CanonicalError::EmptyInput)));
}

/// Error display messages are meaningful
#[test]
fn error_display_messages() {
    let ingest_err = PipelineError::Ingest(IngestError::EmptyNormalizedText);
    let perceptual_err = PipelineError::Perceptual(PerceptualError::InvalidConfigK { k: 0 });

    let ingest_msg = format!("{ingest_err}");
    let perceptual_msg = format!("{perceptual_err}");

    assert!(!ingest_msg.is_empty());
    assert!(!perceptual_msg.is_empty());
    assert!(ingest_msg.len() > 5);
    assert!(perceptual_msg.len() > 5);
}

/// All ingest error variants can be displayed
#[test]
fn ingest_error_variants_display() {
    let errors = vec![
        IngestError::EmptyNormalizedText,
        IngestError::InvalidMetadata("test".into()),
        IngestError::InvalidUtf8("invalid".into()),
        IngestError::MissingPayload,
        IngestError::EmptyBinaryPayload,
        IngestError::PayloadTooLarge("large".into()),
    ];

    for err in errors {
        let msg = format!("{err}");
        assert!(!msg.is_empty(), "Error variant should be displayable");
    }
}
