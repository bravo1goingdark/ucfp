//! Workspace umbrella crate for Universal Content Fingerprinting (UCFP).
//!
//! This crate stitches together ingest normalization and canonicalization so
//! callers can operate over text payloads with a single API entry point.

pub use ufp_canonical::{
    CanonicalizeConfig, CanonicalizedDocument, Token, canonicalize, collapse_whitespace, hash_text,
    tokenize,
};
pub use ufp_ingest::{
    CanonicalIngestRecord, CanonicalPayload, IngestError, IngestMetadata, IngestPayload,
    IngestSource, RawIngestRecord, ingest, normalize_payload,
};
pub use ufp_perceptual::{
    PerceptualConfig, PerceptualError, PerceptualFingerprint, perceptualize_tokens,
};

use chrono::{DateTime, NaiveDate, Utc};
use std::error::Error;
use std::fmt;

/// Errors that can occur while processing an ingest record through the pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipelineError {
    Ingest(IngestError),
    NonTextPayload,
    MissingCanonicalPayload,
    Perceptual(PerceptualError),
}

impl fmt::Display for PipelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PipelineError::Ingest(err) => write!(f, "ingest failure: {err}"),
            PipelineError::NonTextPayload => write!(f, "payload is not text; cannot canonicalize"),
            PipelineError::MissingCanonicalPayload => {
                write!(f, "ingest succeeded without canonical payload")
            }
            PipelineError::Perceptual(err) => write!(f, "perceptual fingerprinting failed: {err}"),
        }
    }
}

impl Error for PipelineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            PipelineError::Ingest(err) => Some(err),
            PipelineError::Perceptual(err) => Some(err),
            PipelineError::NonTextPayload | PipelineError::MissingCanonicalPayload => None,
        }
    }
}

impl From<IngestError> for PipelineError {
    fn from(value: IngestError) -> Self {
        PipelineError::Ingest(value)
    }
}

impl From<PerceptualError> for PipelineError {
    fn from(value: PerceptualError) -> Self {
        PipelineError::Perceptual(value)
    }
}

/// Process a raw ingest record end-to-end, returning a canonicalized document.
/// Binary payloads produce a `PipelineError::NonTextPayload`.
pub fn process_record(
    raw: RawIngestRecord,
    cfg: &CanonicalizeConfig,
) -> Result<CanonicalizedDocument, PipelineError> {
    let canonical_record = ingest(raw)?;

    let payload = canonical_record
        .normalized_payload
        .as_ref()
        .ok_or(PipelineError::MissingCanonicalPayload)?;

    match payload {
        CanonicalPayload::Text(text) => Ok(canonicalize(text, cfg)),
        CanonicalPayload::Binary(_) => Err(PipelineError::NonTextPayload),
    }
}

/// Run ingest, canonicalization, and perceptual fingerprinting in order.
/// Returns both the canonical document and the resulting perceptual fingerprint.
pub fn process_record_with_perceptual(
    raw: RawIngestRecord,
    canonical_cfg: &CanonicalizeConfig,
    perceptual_cfg: &PerceptualConfig,
) -> Result<(CanonicalizedDocument, PerceptualFingerprint), PipelineError> {
    let doc = process_record(raw, canonical_cfg)?;
    let token_refs: Vec<&str> = doc.tokens.iter().map(|t| t.text.as_str()).collect();
    let fingerprint = perceptualize_tokens(&token_refs, perceptual_cfg)?;
    Ok((doc, fingerprint))
}

fn demo_timestamp() -> DateTime<Utc> {
    let Some(date) = NaiveDate::from_ymd_opt(2025, 1, 1) else {
        panic!("invalid demo date components");
    };
    let Some(date_time) = date.and_hms_opt(0, 0, 0) else {
        panic!("invalid demo time components");
    };
    DateTime::<Utc>::from_naive_utc_and_offset(date_time, Utc)
}

/// Convenience helper that feeds the bundled `big_text.txt` sample through the full pipeline.
/// Useful for demos and integration smoke tests.
pub fn big_text_demo(
    perceptual_cfg: &PerceptualConfig,
) -> Result<(CanonicalizedDocument, PerceptualFingerprint), PipelineError> {
    const BIG_TEXT: &str = include_str!("../crates/ufp_canonical/examples/big_text.txt");

    let raw = RawIngestRecord {
        id: "demo-big-text".into(),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: "ucfp-demo".into(),
            doc_id: "big-text".into(),
            received_at: demo_timestamp(),
            original_source: Some("crates/ufp_canonical/examples/big_text.txt".into()),
            attributes: None,
        },
        payload: Some(IngestPayload::Text(BIG_TEXT.to_string())),
    };

    process_record_with_perceptual(raw, &CanonicalizeConfig::default(), perceptual_cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_record(payload: IngestPayload) -> RawIngestRecord {
        RawIngestRecord {
            id: "ingest-1".into(),
            source: IngestSource::RawText,
            metadata: IngestMetadata {
                tenant_id: "tenant".into(),
                doc_id: "doc".into(),
                received_at: demo_timestamp(),
                original_source: Some("origin".into()),
                attributes: None,
            },
            payload: Some(payload),
        }
    }

    #[test]
    fn process_record_canonicalizes_text() {
        let cfg = CanonicalizeConfig::default();
        let record = base_record(IngestPayload::Text(" Hello   Rust ".into()));

        let doc = process_record(record, &cfg).expect("canonicalization should succeed");
        assert_eq!(doc.canonical_text, "hello rust");
        assert_eq!(doc.tokens.len(), 2);
        assert_eq!(doc.tokens[0].text, "hello");
        assert_eq!(doc.tokens[1].text, "rust");
    }

    #[test]
    fn process_record_rejects_binary_payload() {
        let cfg = CanonicalizeConfig::default();
        let record = RawIngestRecord {
            id: "ingest-binary".into(),
            source: IngestSource::File {
                filename: "data.bin".into(),
                content_type: None,
            },
            metadata: IngestMetadata {
                tenant_id: "tenant".into(),
                doc_id: "doc".into(),
                received_at: demo_timestamp(),
                original_source: None,
                attributes: None,
            },
            payload: Some(IngestPayload::Binary(vec![0, 1, 2])),
        };

        let result = process_record(record, &cfg);
        assert!(matches!(result, Err(PipelineError::NonTextPayload)));
    }

    #[test]
    fn process_record_requires_payload() {
        let cfg = CanonicalizeConfig::default();
        let record = RawIngestRecord {
            id: "ingest-empty".into(),
            source: IngestSource::RawText,
            metadata: IngestMetadata {
                tenant_id: "tenant".into(),
                doc_id: "doc".into(),
                received_at: demo_timestamp(),
                original_source: None,
                attributes: None,
            },
            payload: Some(IngestPayload::Text("   ".into())),
        };

        let result = process_record(record, &cfg);
        assert!(matches!(
            result,
            Err(PipelineError::Ingest(IngestError::MissingPayload))
        ));
    }

    #[test]
    fn process_record_deterministic_output() {
        let cfg = CanonicalizeConfig::default();
        let record_a = base_record(IngestPayload::Text(" Caf\u{00E9}\nRust ".into()));
        let record_b = base_record(IngestPayload::Text("Cafe\u{0301} RUST".into()));

        let doc_a = process_record(record_a, &cfg).expect("first canonicalization");
        let doc_b = process_record(record_b, &cfg).expect("second canonicalization");

        assert_eq!(doc_a.canonical_text, doc_b.canonical_text);
        assert_eq!(doc_a.sha256_hex, doc_b.sha256_hex);
    }

    #[test]
    fn process_record_with_perceptual_produces_fingerprint() {
        let canonical_cfg = CanonicalizeConfig::default();
        let mut perceptual_cfg = PerceptualConfig::default();
        perceptual_cfg.k = 3; // ensure tokens >= k for the short input
        let record = base_record(IngestPayload::Text(
            "The quick brown fox jumps over the lazy dog".into(),
        ));

        let (doc, fp) = process_record_with_perceptual(record, &canonical_cfg, &perceptual_cfg)
            .expect("pipeline should succeed");

        assert!(!doc.canonical_text.is_empty());
        assert!(!fp.shingles.is_empty());
        assert_eq!(fp.meta.k, 3);
    }

    #[test]
    fn big_text_demo_runs_full_pipeline() {
        let mut cfg = PerceptualConfig::default();
        cfg.use_parallel = false;
        let (_doc, fp) = big_text_demo(&cfg).expect("demo pipeline should succeed");
        assert!(!fp.minhash.is_empty());
    }
}
