/// Ingest layer for text-based UCFP.
/// Provides public API for receiving inputs, normalizing metadata, basic validation,
/// and producing a canonical ingest record ready for canonicalizer.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Source kinds we accept at ingest time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IngestSource {
    RawText,
    Url(String),
    File { filename: String, content_type: Option<String> },
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

/// The inbound request for ingest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestRequest {
    pub source: IngestSource,
    pub metadata: Option<IngestMetadata>,
    /// The raw payload when available (for RawText/File). For Url, content may be fetched
    /// by a higher-layer fetcher or pipeline.
    pub payload: Option<String>,
}

/// Normalized record produced by ingest. This is what the canonicalizer will accept.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalIngestRecord {
    pub id: String, // stable UUID for this ingest operation
    pub tenant_id: String,
    pub doc_id: String,
    pub received_at: DateTime<Utc>,
    pub original_source: Option<String>,
    pub source: IngestSource,
    /// Normalized payload - trimmed and with normalized whitespace. Not yet canonicalized.
    pub normalized_payload: Option<String>,
    /// raw attributes JSON preserved
    pub attributes: Option<serde_json::Value>,
}

#[derive(Error, Debug)]
pub enum IngestError {
    #[error("missing payload for source that requires payload")]
    MissingPayload,
    #[error("invalid metadata: {0}")]
    InvalidMetadata(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<chrono::ParseError> for IngestError {
    fn from(e: chrono::ParseError) -> Self {
        IngestError::Internal(format!("chrono error: {}", e))
    }
}

/// Public ingest function. It validates metadata (or supplies defaults), normalizes
/// payload (trims and collapses whitespace), and returns a canonical record for the
/// canonicalizer stage.
pub fn ingest(req: IngestRequest) -> Result<CanonicalIngestRecord, IngestError> {
    // Validate or create metadata
    let meta = match req.metadata {
        Some(m) => validate_or_default_metadata(m)?,
        None => default_metadata(),
    };

    // Check payload requirements
    match &req.source {
        IngestSource::RawText | IngestSource::File { .. } => {
            if req.payload.as_ref().map(|s| s.trim().is_empty()).unwrap_or(true) {
                return Err(IngestError::MissingPayload);
            }
        }
        IngestSource::Url(_url) => {
            // For Url, payload may be empty: fetching may happen asynchronously in pipeline.
        }
        IngestSource::Api => {
            // API calls should usually provide payload; but allow missing for metadata-only.
        }
    }

    // Normalize payload if present.
    let normalized_payload = req.payload.map(|p| normalize_whitespace(&p));

    // Create UUID for this ingest run
    let id = Uuid::new_v4().to_string();

    Ok(CanonicalIngestRecord {
        id,
        tenant_id: meta.tenant_id,
        doc_id: meta.doc_id,
        received_at: meta.received_at,
        original_source: meta.original_source,
        source: req.source,
        normalized_payload,
        attributes: meta.attributes,
    })
}

fn validate_or_default_metadata(mut m: IngestMetadata) -> Result<IngestMetadata, IngestError> {
    // Tenant id required
    if m.tenant_id.trim().is_empty() {
        return Err(IngestError::InvalidMetadata("tenant_id empty".into()));
    }

    // doc_id default to UUID if empty
    if m.doc_id.trim().is_empty() {
        m.doc_id = Uuid::new_v4().to_string();
    }

    // received_at default to now if zero? We'll accept what's present.
    Ok(m)
}

fn default_metadata() -> IngestMetadata {
    IngestMetadata {
        tenant_id: "public".to_string(),
        doc_id: Uuid::new_v4().to_string(),
        received_at: Utc::now(),
        original_source: None,
        attributes: None,
    }
}

/// Collapses repeated whitespace, trims edges, and normalizes newlines to single ' '.
/// Keeps content deterministic across runs.
fn normalize_whitespace(s: &str) -> String {
    // Simple, deterministic algorithm: split on Unicode whitespace and join with single space.
    s.split_whitespace().collect::<Vec<&str>>().join(" ")
}

// -----------------------------
// Example usage and tests
// -----------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_whitespace() {
        let s = "  Hello\n\n   world\t this  is\n a test  ";
        let out = normalize_whitespace(s);
        assert_eq!(out, "Hello world this is a test");
    }

    #[test]
    fn test_ingest_rawtext_success() {
        let req = IngestRequest {
            source: IngestSource::RawText,
            metadata: Some(IngestMetadata {
                tenant_id: "tenant1".into(),
                doc_id: "doc-123".into(),
                received_at: Utc::now(),
                original_source: None,
                attributes: None,
            }),
            payload: Some(" Hello   world \n ".into()),
        };

        let rec = ingest(req).expect("ingest should succeed");
        assert_eq!(rec.tenant_id, "tenant1");
        assert_eq!(rec.doc_id, "doc-123");
        assert_eq!(rec.normalized_payload.unwrap(), "Hello world");
    }

    #[test]
    fn test_ingest_missing_payload_for_rawtext() {
        let req = IngestRequest {
            source: IngestSource::RawText,
            metadata: Some(IngestMetadata {
                tenant_id: "t".into(),
                doc_id: "d".into(),
                received_at: Utc::now(),
                original_source: None,
                attributes: None,
            }),
            payload: Some("   ".into()),
        };

        let res = ingest(req);
        assert!(matches!(res, Err(IngestError::MissingPayload)));
    }
}

