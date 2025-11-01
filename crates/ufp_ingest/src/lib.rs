//! Ingest layer for text-based UCFP.
//! Provides public API for receiving inputs, normalizing metadata, basic validation,
//! and producing a canonical ingest record ready for canonicalizer.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Source kinds we accept at ingest time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IngestSource {
    RawText,
    Url(String),
    File {
        filename: String,
        content_type: Option<String>,
    },
    Api,
}

/// Metadata associated with an ingest request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IngestMetadata {
    pub tenant_id: String,
    pub doc_id: String,
    pub received_at: DateTime<Utc>,
    /// Optional original source id (e.g., URL or external id)
    pub original_source: Option<String>,
    /// Arbitrary attributes for future use (signed map might live elsewhere)
    pub attributes: Option<serde_json::Value>,
}

/// The inbound record for ingest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RawIngestRecord {
    pub id: String,
    pub source: IngestSource,
    pub metadata: IngestMetadata,
    /// Raw payload when available. Text and binary variants are supported to enable multi-modal handling.
    pub payload: Option<IngestPayload>,
}

/// Raw payload content provided during ingest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IngestPayload {
    /// UTF-8 text payload for canonicalization.
    Text(String),
    /// Arbitrary binary payload (e.g., images, audio, PDFs) that will bypass text canonicalization.
    Binary(Vec<u8>),
}

/// Normalized record produced by ingest. This is what the canonicalizer will accept.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CanonicalIngestRecord {
    pub id: String,
    pub tenant_id: String,
    pub doc_id: String,
    pub received_at: DateTime<Utc>,
    pub original_source: Option<String>,
    pub source: IngestSource,
    /// Normalized payload. Text inputs have whitespace collapsed; binary inputs pass through unchanged.
    pub normalized_payload: Option<CanonicalPayload>,
    /// Raw attributes JSON preserved
    pub attributes: Option<serde_json::Value>,
}

/// Normalized payload ready for downstream stages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CanonicalPayload {
    /// Normalized UTF-8 text payload.
    Text(String),
    /// Binary payload preserved for downstream perceptual/semantic stages.
    Binary(Vec<u8>),
}

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum IngestError {
    #[error("missing payload for source that requires payload")]
    MissingPayload,
    #[error("invalid metadata: {0}")]
    InvalidMetadata(String),
}

/// Public ingest function. It validates metadata, normalizes payload (trims and collapses whitespace),
/// and returns a canonical record for the canonicalizer stage.
pub fn ingest(raw: RawIngestRecord) -> Result<CanonicalIngestRecord, IngestError> {
    validate_record(&raw)?;

    let RawIngestRecord {
        id,
        source,
        metadata,
        payload,
    } = raw;

    let normalized_payload = payload.map(normalize_payload_value);

    Ok(CanonicalIngestRecord {
        id,
        tenant_id: metadata.tenant_id,
        doc_id: metadata.doc_id,
        received_at: metadata.received_at,
        original_source: metadata.original_source,
        source,
        normalized_payload,
        attributes: metadata.attributes,
    })
}

fn validate_record(record: &RawIngestRecord) -> Result<(), IngestError> {
    if record.id.trim().is_empty() {
        return Err(IngestError::InvalidMetadata("id empty".into()));
    }

    if record.metadata.tenant_id.trim().is_empty() {
        return Err(IngestError::InvalidMetadata("tenant_id empty".into()));
    }

    if record.metadata.doc_id.trim().is_empty() {
        return Err(IngestError::InvalidMetadata("doc_id empty".into()));
    }

    ensure_payload(&record.source, &record.payload)
}

fn ensure_payload(
    source: &IngestSource,
    payload: &Option<IngestPayload>,
) -> Result<(), IngestError> {
    let has_payload = match payload {
        Some(IngestPayload::Text(text)) => !text.trim().is_empty(),
        Some(IngestPayload::Binary(bytes)) => !bytes.is_empty(),
        None => false,
    };

    match source {
        IngestSource::RawText | IngestSource::File { .. } => {
            if !has_payload {
                return Err(IngestError::MissingPayload);
            }
        }
        IngestSource::Url(_) | IngestSource::Api => {}
    }

    Ok(())
}

fn normalize_payload_value(payload: IngestPayload) -> CanonicalPayload {
    match payload {
        IngestPayload::Text(text) => CanonicalPayload::Text(normalize_payload(&text)),
        IngestPayload::Binary(bytes) => CanonicalPayload::Binary(bytes),
    }
}

/// Collapses repeated whitespace, trims edges, and normalizes newlines to single ' '.
/// Keeps content deterministic across runs.
pub fn normalize_payload(s: &str) -> String {
    let mut normalized = String::with_capacity(s.len());
    for segment in s.split_whitespace() {
        if !normalized.is_empty() {
            normalized.push(' ');
        }
        normalized.push_str(segment);
    }
    normalized
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

    fn base_metadata() -> IngestMetadata {
        IngestMetadata {
            tenant_id: "tenant1".into(),
            doc_id: "doc-123".into(),
            received_at: fixed_timestamp(),
            original_source: None,
            attributes: None,
        }
    }

    #[test]
    fn test_normalize_payload() {
        let cases = [
            (
                "  Hello\n\n   world\t this  is\n a test  ",
                "Hello world this is a test",
            ),
            ("\n", ""),
            ("emoji \u{1f600} test ", "emoji \u{1f600} test"),
        ];

        for (input, expected) in cases {
            assert_eq!(normalize_payload(input), expected);
        }
    }

    #[test]
    fn test_ingest_rawtext_success() {
        let record = RawIngestRecord {
            id: "ingest-1".into(),
            source: IngestSource::RawText,
            metadata: base_metadata(),
            payload: Some(IngestPayload::Text(" Hello   world \n ".into())),
        };

        let rec = ingest(record).expect("ingest should succeed");
        assert_eq!(rec.tenant_id, "tenant1");
        assert_eq!(rec.doc_id, "doc-123");
        match rec.normalized_payload {
            Some(CanonicalPayload::Text(text)) => assert_eq!(text, "Hello world"),
            _ => panic!("expected text payload"),
        }
    }

    #[test]
    fn test_ingest_missing_payload_for_rawtext() {
        let record = RawIngestRecord {
            id: "ingest-2".into(),
            source: IngestSource::RawText,
            metadata: base_metadata(),
            payload: Some(IngestPayload::Text("   ".into())),
        };

        let res = ingest(record);
        assert!(matches!(res, Err(IngestError::MissingPayload)));
    }

    #[test]
    fn test_ingest_file_binary_payload() {
        let record = RawIngestRecord {
            id: "ingest-3".into(),
            source: IngestSource::File {
                filename: "image.png".into(),
                content_type: Some("image/png".into()),
            },
            metadata: base_metadata(),
            payload: Some(IngestPayload::Binary(vec![1, 2, 3, 4])),
        };

        let rec = ingest(record).expect("ingest should succeed");
        match rec.normalized_payload {
            Some(CanonicalPayload::Binary(bytes)) => assert_eq!(bytes, vec![1, 2, 3, 4]),
            _ => panic!("expected binary payload"),
        }
    }

    #[test]
    fn test_metadata_preserved() {
        let record = RawIngestRecord {
            id: "ingest-4".into(),
            source: IngestSource::Api,
            metadata: IngestMetadata {
                tenant_id: "tenant-x".into(),
                doc_id: "doc-y".into(),
                received_at: fixed_timestamp(),
                original_source: Some("source-42".into()),
                attributes: Some(serde_json::json!({"kind": "demo"})),
            },
            payload: None,
        };

        let rec = ingest(record).expect("ingest should succeed");
        assert_eq!(rec.tenant_id, "tenant-x");
        assert_eq!(rec.doc_id, "doc-y");
        assert_eq!(rec.original_source.as_deref(), Some("source-42"));
        assert_eq!(rec.attributes, Some(serde_json::json!({"kind": "demo"})));
        assert!(rec.normalized_payload.is_none());
    }
}
