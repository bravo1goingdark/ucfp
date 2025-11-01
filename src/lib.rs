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

use std::error::Error;
use std::fmt;

/// Errors that can occur while processing an ingest record through the pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipelineError {
    Ingest(IngestError),
    NonTextPayload,
    MissingCanonicalPayload,
}

impl fmt::Display for PipelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PipelineError::Ingest(err) => write!(f, "ingest failure: {err}"),
            PipelineError::NonTextPayload => write!(f, "payload is not text; cannot canonicalize"),
            PipelineError::MissingCanonicalPayload => {
                write!(f, "ingest succeeded without canonical payload")
            }
        }
    }
}

impl Error for PipelineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            PipelineError::Ingest(err) => Some(err),
            PipelineError::NonTextPayload | PipelineError::MissingCanonicalPayload => None,
        }
    }
}

impl From<IngestError> for PipelineError {
    fn from(value: IngestError) -> Self {
        PipelineError::Ingest(value)
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, NaiveDate, Utc};

    fn fixed_timestamp() -> DateTime<Utc> {
        let Some(date) = NaiveDate::from_ymd_opt(2024, 1, 1) else {
            panic!("invalid date components");
        };
        let Some(date_time) = date.and_hms_opt(0, 0, 0) else {
            panic!("invalid time components");
        };
        DateTime::<Utc>::from_naive_utc_and_offset(date_time, Utc)
    }

    fn base_record(payload: IngestPayload) -> RawIngestRecord {
        RawIngestRecord {
            id: "ingest-1".into(),
            source: IngestSource::RawText,
            metadata: IngestMetadata {
                tenant_id: "tenant".into(),
                doc_id: "doc".into(),
                received_at: fixed_timestamp(),
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
                received_at: fixed_timestamp(),
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
                received_at: fixed_timestamp(),
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
}
