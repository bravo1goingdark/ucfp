use ucfp::{
    CanonicalizeConfig, IngestConfig, IngestError, IngestMetadata, IngestPayload, IngestSource,
    PerceptualConfig, PerceptualError, PipelineError, RawIngestRecord, process_document,
    process_record_with_perceptual_configs,
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
