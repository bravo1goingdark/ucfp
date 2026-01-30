//! Core data model types for the `ingest` crate.
//!
//! These types represent the shape of ingest requests and the normalized
//! records that flow to downstream pipeline stages.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Source kinds we accept at ingest time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
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
    /// Optional tenant id; defaults to config default when metadata omits it.
    pub tenant_id: Option<String>,
    /// Optional document id; deterministically generated when None/empty.
    pub doc_id: Option<String>,
    /// Optional timestamp supplied by client.
    pub received_at: Option<DateTime<Utc>>,
    /// Optional original source id (e.g., URL or external id)
    pub original_source: Option<String>,
    /// Arbitrary attributes for future use (signed map might live elsewhere)
    pub attributes: Option<serde_json::Value>,
}

/// The inbound record for ingest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RawIngestRecord {
    /// Unique identifier for this ingest operation.
    pub id: String,
    /// Source of the content (API, file upload, raw text, etc.).
    pub source: IngestSource,
    /// Metadata associated with the record (tenant, doc_id, timestamps, etc.).
    pub metadata: IngestMetadata,
    /// Raw payload when available. Text and binary variants are supported to enable multi-modal handling.
    pub payload: Option<IngestPayload>,
}

/// Raw payload content provided during ingest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub enum IngestPayload {
    /// UTF-8 text payload for canonicalization.
    Text(String),
    /// Raw UTF-8 bytes that will be decoded during ingest.
    TextBytes(Vec<u8>),
    /// Arbitrary binary payload (e.g., images, audio, PDFs) that will bypass text canonicalization.
    Binary(Vec<u8>),
}

/// Normalized record produced by ingest. This is what the canonicalizer will accept.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CanonicalIngestRecord {
    /// Unique identifier for this ingest operation (mirrors RawIngestRecord.id).
    pub id: String,
    /// Tenant identifier for multi-tenant isolation.
    pub tenant_id: String,
    /// Document identifier (may be derived from id if not provided).
    pub doc_id: String,
    /// Timestamp when the record was received.
    pub received_at: DateTime<Utc>,
    /// Original source information if provided.
    pub original_source: Option<String>,
    /// Source of the content (API, file upload, raw text, etc.).
    pub source: IngestSource,
    /// Normalized payload. Text inputs have whitespace collapsed; binary inputs pass through unchanged.
    pub normalized_payload: Option<CanonicalPayload>,
    /// Raw attributes JSON preserved for downstream use.
    pub attributes: Option<serde_json::Value>,
}

/// Normalized payload ready for downstream stages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub enum CanonicalPayload {
    /// Normalized UTF-8 text payload.
    Text(String),
    /// Binary payload preserved for downstream perceptual/semantic stages.
    Binary(Vec<u8>),
}
