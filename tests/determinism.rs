use ucfp::{
    CanonicalizeConfig, IngestConfig, IngestMetadata, IngestPayload, IngestSource,
    PerceptualConfig, RawIngestRecord,
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
    ucfp::process_document_with_configs(raw, ingest_cfg, canonical_cfg, perceptual_cfg)
}
