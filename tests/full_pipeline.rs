use ucfp::{
    process_pipeline, CanonicalizeConfig, IngestConfig, IngestMetadata, IngestPayload,
    IngestSource, PerceptualConfig, PipelineError, PipelineStageConfig, RawIngestRecord,
};

#[test]
fn full_pipeline_executes_with_defaults() -> Result<(), PipelineError> {
    let raw = RawIngestRecord {
        id: "full-pipeline".into(),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: Some("tenant-full".into()),
            doc_id: Some("doc-full".into()),
            received_at: None,
            original_source: Some("integration/full_pipeline".into()),
            attributes: None,
        },
        payload: Some(IngestPayload::Text(
            "The quick brown fox jumps over the lazy dog".into(),
        )),
    };

    let ingest_cfg = IngestConfig::default();
    let canonical_cfg = CanonicalizeConfig::default();
    let perceptual_cfg = PerceptualConfig::default();

    let (doc, fingerprint, _) = process_pipeline(
        raw,
        PipelineStageConfig::Perceptual,
        &ingest_cfg,
        &canonical_cfg,
        Some(&perceptual_cfg),
        None,
    )?;
    let fingerprint = fingerprint.expect("fingerprint should be present");

    assert_eq!(doc.doc_id, "doc-full");
    assert!(!doc.canonical_text.is_empty());
    assert!(!fingerprint.minhash.is_empty());
    assert_eq!(fingerprint.meta.k, perceptual_cfg.k);
    assert_eq!(fingerprint.meta.config_version, perceptual_cfg.version);

    Ok(())
}
