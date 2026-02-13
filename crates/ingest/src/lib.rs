//! UCFP Ingest Layer - Content Ingestion and Validation
//!
//! This crate provides the entry point to the Universal Content Fingerprinting (UCFP) pipeline,
//! transforming raw content and metadata into clean, deterministic records suitable for
//! downstream processing.
//!
//! # Overview
//!
//! The ingest crate is responsible for:
//! - **Validation**: Enforcing metadata policies, size limits, and business rules
//! - **Normalization**: Collapsing whitespace, stripping control characters, sanitizing inputs
//! - **ID Generation**: Deriving stable document IDs using UUIDv5 when not explicitly provided
//! - **Multi-modal Support**: Handling text, binary, and structured payloads uniformly
//! - **Observability**: Structured logging via `tracing` for production debugging
//!
//! # Pipeline Position
//!
//! ```text
//! Raw Content ──▶ Ingest ──▶ Canonical ──▶ Perceptual/Semantic ──▶ Index ──▶ Match
//!                    ↑
//!                 (this crate)
//! ```
//!
//! # Quick Start
//!
//! ```rust
//! use ingest::{
//!     ingest, IngestConfig, RawIngestRecord,
//!     IngestSource, IngestMetadata, IngestPayload
//! };
//! use chrono::Utc;
//!
//! // Configure (use defaults for quick start)
//! let config = IngestConfig::default();
//!
//! // Create a raw record
//! let record = RawIngestRecord {
//!     id: "doc-001".to_string(),
//!     source: IngestSource::RawText,
//!     metadata: IngestMetadata {
//!         tenant_id: Some("acme-corp".to_string()),
//!         doc_id: Some("report-q4-2024".to_string()),
//!         received_at: Some(Utc::now()),
//!         original_source: None,
//!         attributes: None,
//!     },
//!     payload: Some(IngestPayload::Text(
//!         "  Quarterly report: revenue up 15% YoY.   ".to_string()
//!     )),
//! };
//!
//! // Ingest and get canonical record
//! let canonical = ingest(record, &config).unwrap();
//!
//! assert_eq!(canonical.tenant_id, "acme-corp");
//! // Whitespace normalized: "Quarterly report: revenue up 15% YoY."
//! ```
//!
//! # Core Design Principles
//!
//! 1. **Fail Fast**: Validation happens before any transformation
//! 2. **Deterministic**: Same input always produces same output (critical for fingerprinting)
//! 3. **Observable**: Every operation is logged with structured tracing
//! 4. **Safe**: Control characters stripped, sizes bounded, UTF-8 validated
//!
//! # Architecture
//!
//! The ingest pipeline follows a strict data flow:
//!
//! 1. **Payload Requirements Check**: Verify source mandates are met
//! 2. **Raw Size Validation**: Enforce `max_payload_bytes` limit
//! 3. **Metadata Normalization**: Apply defaults, validate policies, sanitize
//! 4. **Payload Normalization**: Decode UTF-8, collapse whitespace, preserve binary
//! 5. **Normalized Size Validation**: Enforce `max_normalized_bytes` limit
//! 6. **Canonical Record Construction**: Build deterministic output
//!
//! # Module Structure
//!
//! - [`config`](config): Configuration types (`IngestConfig`, `MetadataPolicy`)
//! - [`error`](error): Error types (`IngestError`)
//! - [`types`](types): Data model (`RawIngestRecord`, `CanonicalIngestRecord`, etc.)
//! - [`metadata`](metadata): Metadata normalization and validation logic
//! - [`payload`](payload): Payload validation and transformation utilities
//!
//! # Error Handling
//!
//! All errors are typed via [`IngestError`] for precise handling:
//!
//! ```rust
//! use ingest::{ingest, IngestError};
//!
//! match ingest(record, &config) {
//!     Ok(canonical) => process(canonical),
//!     Err(IngestError::PayloadTooLarge(msg)) => {
//!         eprintln!("Content too large: {}", msg);
//!     }
//!     Err(IngestError::InvalidUtf8(msg)) => {
//!         eprintln!("Invalid encoding: {}", msg);
//!     }
//!     Err(e) => {
//!         eprintln!("Ingest failed: {}", e);
//!     }
//! }
//! ```
//!
//! # Configuration
//!
//! For production use, configure size limits and policies:
//!
//! ```rust
//! use ingest::{IngestConfig, MetadataPolicy, RequiredField};
//! use uuid::Uuid;
//!
//! let config = IngestConfig {
//!     version: 1,
//!     default_tenant_id: "default".to_string(),
//!     doc_id_namespace: Uuid::new_v5(&Uuid::NAMESPACE_DNS, b"myapp.example.com"),
//!     strip_control_chars: true,
//!     metadata_policy: MetadataPolicy {
//!         required_fields: vec![
//!             RequiredField::TenantId,
//!             RequiredField::DocId,
//!         ],
//!         max_attribute_bytes: Some(1024 * 1024), // 1 MB
//!         reject_future_timestamps: true,
//!     },
//!     max_payload_bytes: Some(50 * 1024 * 1024),      // 50 MB raw
//!     max_normalized_bytes: Some(10 * 1024 * 1024),   // 10 MB normalized
//! };
//!
//! // Validate at startup
//! config.validate().expect("Invalid configuration");
//! ```
//!
//! # Performance
//!
//! - **Base overhead**: ~5-15μs for small payloads
//! - **Text normalization**: O(n) where n = text length
//! - **Memory**: Allocates new String during normalization
//! - **Thread safety**: `ingest()` is pure and safe for parallel processing
//!
//! # Examples
//!
//! See the `examples/` directory for complete working examples:
//! - `ingest_demo.rs`: Basic text ingestion
//! - `batch_ingest.rs`: Processing multiple records
//! - `size_limit_demo.rs`: Size limit enforcement demonstration
//!
//! # See Also
//!
//! - [Crate documentation](doc/ingest.md) for comprehensive guides
//! - [`config`](config) module for configuration details
//! - [`types`](types) module for data structure definitions

use std::time::Instant;

use tracing::{info, warn, Level};

mod config;
mod error;
mod metadata;
mod payload;
mod types;

use crate::metadata::normalize_metadata;

pub use crate::config::{ConfigError, IngestConfig, MetadataPolicy, RequiredField};
pub use crate::error::IngestError;
pub use crate::payload::{
    normalize_payload_option, payload_kind, payload_length, validate_payload_requirements,
};
pub use crate::types::{
    CanonicalIngestRecord, CanonicalPayload, IngestMetadata, IngestPayload, IngestSource,
    RawIngestRecord,
};

/// Ingests a raw record and produces a canonical, normalized record.
///
/// This is the primary entry point for the ingest pipeline. It validates the raw record,
/// normalizes metadata and payload, and returns a deterministic `CanonicalIngestRecord`
/// suitable for downstream processing.
///
/// # Arguments
///
/// * `raw` - The raw ingest record containing metadata and optional payload
/// * `cfg` - Runtime configuration controlling validation and normalization behavior
///
/// # Returns
///
/// * `Ok(CanonicalIngestRecord)` - Successfully ingested and normalized record
/// * `Err(IngestError)` - Validation or normalization failure with specific error type
///
/// # Errors
///
/// This function can return various [`IngestError`] variants:
///
/// * `MissingPayload` - Source requires a payload but none was provided
/// * `EmptyBinaryPayload` - Binary payload has zero bytes
/// * `InvalidMetadata(String)` - Metadata policy violation (required field missing, future timestamp, etc.)
/// * `InvalidUtf8(String)` - `TextBytes` payload contains invalid UTF-8 sequences
/// * `EmptyNormalizedText` - Text payload is empty after whitespace normalization
/// * `PayloadTooLarge(String)` - Payload exceeds configured size limits
///
/// # Side Effects
///
/// * Emits structured tracing spans for observability
/// * Records timing metrics for performance monitoring
///
/// # Examples
///
/// ## Basic Usage
///
/// ```rust
/// use ingest::{
///     ingest, IngestConfig, RawIngestRecord,
///     IngestSource, IngestMetadata, IngestPayload
/// };
/// use chrono::Utc;
///
/// let config = IngestConfig::default();
/// let record = RawIngestRecord {
///     id: "my-doc-1".into(),
///     source: IngestSource::RawText,
///     metadata: IngestMetadata {
///         tenant_id: Some("my-tenant".into()),
///         doc_id: None, // Will be derived
///         received_at: Some(Utc::now()),
///         original_source: None,
///         attributes: None,
///     },
///     payload: Some(IngestPayload::Text(
///         "  Some text with   extra whitespace.  ".into()
///     )),
/// };
///
/// let canonical = ingest(record, &config).unwrap();
/// assert_eq!(canonical.tenant_id, "my-tenant");
/// // Note: doc_id is derived if not provided
/// ```
///
/// ## Error Handling
///
/// ```rust
/// use ingest::{ingest, IngestConfig, IngestError, IngestPayload, IngestSource};
/// use ingest::{RawIngestRecord, IngestMetadata};
///
/// let config = IngestConfig::default();
///
/// // Invalid UTF-8 bytes
/// let record = RawIngestRecord {
///     id: "test".into(),
///     source: IngestSource::RawText,
///     metadata: IngestMetadata {
///         tenant_id: Some("tenant".into()),
///         doc_id: Some("doc".into()),
///         received_at: None,
///         original_source: None,
///         attributes: None,
///     },
///     payload: Some(IngestPayload::TextBytes(vec![0xff, 0xfe])),
/// };
///
/// match ingest(record, &config) {
///     Err(IngestError::InvalidUtf8(_)) => println!("Invalid UTF-8 detected"),
///     _ => println!("Other result"),
/// }
/// ```
///
/// # Performance
///
/// - Small text payloads: ~10-20μs
/// - Large text payloads: scales linearly with size
/// - Binary payloads: minimal overhead (size check only)
pub fn ingest(
    raw: RawIngestRecord,
    cfg: &IngestConfig,
) -> Result<CanonicalIngestRecord, IngestError> {
    let start = Instant::now();
    let RawIngestRecord {
        id,
        source,
        metadata,
        payload,
    } = raw;

    let tenant_hint = metadata.tenant_id.clone();
    let doc_hint = metadata.doc_id.clone();

    let record_id = match metadata::sanitize_required_field("id", id, cfg.strip_control_chars) {
        Ok(id) => id,
        Err(err) => {
            let elapsed_micros = start.elapsed().as_micros();
            warn!(error = %err, elapsed_micros, "ingest_failure");
            return Err(err);
        }
    };

    let span = tracing::span!(
        Level::INFO,
        "ingest.ingest",
        record_id = %record_id,
        source = ?source
    );
    let _guard = span.enter();

    match ingest_inner(record_id.clone(), source, metadata, payload, cfg) {
        Ok(record) => {
            let elapsed_micros = start.elapsed().as_micros();
            info!(
                tenant_id = %record.tenant_id,
                doc_id = %record.doc_id,
                payload_kind = %payload_kind(record.normalized_payload.as_ref()),
                normalized_len = payload_length(record.normalized_payload.as_ref()),
                elapsed_micros,
                "ingest_success"
            );
            Ok(record)
        }
        Err(err) => {
            let elapsed_micros = start.elapsed().as_micros();
            warn!(
                tenant_id = ?tenant_hint,
                doc_id = ?doc_hint,
                error = %err,
                elapsed_micros,
                "ingest_failure"
            );
            Err(err)
        }
    }
}

/// Core ingest logic: validates payload requirements, normalizes metadata and payload.
///
/// This internal function performs the actual ingest work. It is separated from the
/// public `ingest()` function to facilitate testing and to keep the observability
/// wrapper clean.
///
/// # Arguments
///
/// * `record_id` - Sanitized unique identifier for this ingest operation
/// * `source` - Source type (RawText, File, etc.)
/// * `metadata` - Raw metadata to be normalized
/// * `payload` - Optional raw payload
/// * `cfg` - Configuration for validation and normalization
///
/// # Returns
///
/// Normalized `CanonicalIngestRecord` on success, `IngestError` on failure
fn ingest_inner(
    record_id: String,
    source: IngestSource,
    metadata: IngestMetadata,
    payload: Option<IngestPayload>,
    cfg: &IngestConfig,
) -> Result<CanonicalIngestRecord, IngestError> {
    // Some sources require a payload, so we check for that first.
    validate_payload_requirements(&source, &payload)?;

    // Reject oversized raw payloads before normalization.
    if let Some(limit) = cfg.max_payload_bytes {
        if let Some(ref p) = payload {
            let len = match p {
                IngestPayload::Text(s) => s.len(),
                IngestPayload::TextBytes(b) => b.len(),
                IngestPayload::Binary(b) => b.len(),
            };
            if len > limit {
                return Err(IngestError::PayloadTooLarge(format!(
                    "raw payload size {len} exceeds limit of {limit}"
                )));
            }
        }
    }

    // Metadata is normalized and validated against the configured policies.
    let normalized_metadata = normalize_metadata(metadata, cfg, &record_id)?;
    // The payload is normalized based on its type (text or binary).
    let normalized_payload = normalize_payload_option(&source, payload, cfg)?;

    Ok(CanonicalIngestRecord {
        id: record_id,
        tenant_id: normalized_metadata.tenant_id,
        doc_id: normalized_metadata.doc_id,
        received_at: normalized_metadata.received_at,
        original_source: normalized_metadata.original_source,
        source,
        normalized_payload,
        attributes: normalized_metadata.attributes,
    })
}

/// Normalizes text by collapsing repeated whitespace and trimming edges.
///
/// This function performs the following transformations:
/// - Trims leading and trailing whitespace
/// - Collapses multiple consecutive whitespace characters (spaces, tabs, newlines) into single spaces
/// - Preserves Unicode characters (including emojis)
/// - Handles all Unicode whitespace as defined by `char::is_whitespace()`
///
/// # Arguments
///
/// * `s` - The input string to normalize
///
/// # Returns
///
/// A new `String` with whitespace normalized. Returns empty string if input is whitespace-only.
///
/// # Examples
///
/// ```rust
/// use ingest::normalize_payload;
///
/// // Collapse multiple spaces
/// let result = normalize_payload("  Hello   world  ");
/// assert_eq!(result, "Hello world");
///
/// // Handle newlines and tabs
/// let result = normalize_payload("Line1\n\n\t\tLine2");
/// assert_eq!(result, "Line1 Line2");
///
/// // Preserve Unicode
/// let result = normalize_payload("  Hello 👋  world  ");
/// assert_eq!(result, "Hello 👋 world");
///
/// // Empty result for whitespace-only input
/// let result = normalize_payload("   \n\t   ");
/// assert_eq!(result, "");
/// ```
///
/// # Performance
///
/// - Time complexity: O(n) where n is the length of the input string
/// - Space complexity: O(n) for the output string
/// - Pre-allocates capacity equal to input length to minimize reallocations
///
/// # Use Cases
///
/// - Preparing text for fingerprinting (ensures whitespace differences don't affect matching)
/// - Normalizing user input for storage
/// - Cleaning scraped web content
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
    use chrono::{DateTime, Duration, NaiveDate, Utc};

    use super::*;

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
            tenant_id: Some("tenant1".into()),
            doc_id: Some("doc-123".into()),
            received_at: Some(fixed_timestamp()),
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

        let rec = ingest(record, &IngestConfig::default()).expect("ingest should succeed");
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

        let res = ingest(record, &IngestConfig::default());
        assert!(matches!(res, Err(IngestError::EmptyNormalizedText)));
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

        let rec = ingest(record, &IngestConfig::default()).expect("ingest should succeed");
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
                tenant_id: Some("tenant-x".into()),
                doc_id: Some("doc-y".into()),
                received_at: Some(fixed_timestamp()),
                original_source: Some("source-42".into()),
                attributes: Some(serde_json::json!({"kind": "demo"})),
            },
            payload: None,
        };

        let rec = ingest(record, &IngestConfig::default()).expect("ingest should succeed");
        assert_eq!(rec.tenant_id, "tenant-x");
        assert_eq!(rec.doc_id, "doc-y");
        assert_eq!(rec.original_source.as_deref(), Some("source-42"));
        assert_eq!(rec.attributes, Some(serde_json::json!({"kind": "demo"})));
        assert!(rec.normalized_payload.is_none());
    }

    #[test]
    fn test_defaults_applied_when_metadata_missing() {
        let record = RawIngestRecord {
            id: "ingest-5".into(),
            source: IngestSource::RawText,
            metadata: IngestMetadata {
                tenant_id: None,
                doc_id: None,
                received_at: None,
                original_source: Some("\u{0007}source\n".into()),
                attributes: None,
            },
            payload: Some(IngestPayload::Text("payload".into())),
        };

        let cfg = IngestConfig {
            default_tenant_id: "fallback".into(),
            ..Default::default()
        };

        let rec = ingest(record, &cfg).expect("ingest should succeed");
        assert_eq!(rec.tenant_id, "fallback");
        assert!(!rec.doc_id.is_empty());
        assert!(rec.original_source.unwrap().contains("source"));
    }

    #[test]
    fn test_doc_id_derivation_deterministic() {
        let metadata = IngestMetadata {
            tenant_id: None,
            doc_id: None,
            received_at: None,
            original_source: None,
            attributes: None,
        };

        let cfg = IngestConfig::default();
        let record_a = RawIngestRecord {
            id: "deterministic".into(),
            source: IngestSource::RawText,
            metadata: metadata.clone(),
            payload: Some(IngestPayload::Text("payload".into())),
        };
        let record_b = RawIngestRecord {
            id: "deterministic".into(),
            source: IngestSource::RawText,
            metadata,
            payload: Some(IngestPayload::Text("payload".into())),
        };

        let rec_a = ingest(record_a, &cfg).expect("first ingest succeeds");
        let rec_b = ingest(record_b, &cfg).expect("second ingest succeeds");

        assert_eq!(rec_a.doc_id, rec_b.doc_id);
    }

    #[test]
    fn test_invalid_utf8_payload_rejected() {
        let record = RawIngestRecord {
            id: "ingest-utf8".into(),
            source: IngestSource::RawText,
            metadata: base_metadata(),
            payload: Some(IngestPayload::TextBytes(vec![0xff, 0xfe])),
        };

        let res = ingest(record, &IngestConfig::default());
        assert!(matches!(res, Err(IngestError::InvalidUtf8(_))));
    }

    #[test]
    fn test_control_chars_removed_from_metadata() {
        let record = RawIngestRecord {
            id: "ingest-ctrl".into(),
            source: IngestSource::Api,
            metadata: IngestMetadata {
                tenant_id: Some("tenant\u{0003}".into()),
                doc_id: Some("doc\n\r".into()),
                received_at: None,
                original_source: Some(" source\u{0008} ".into()),
                attributes: None,
            },
            payload: None,
        };

        let rec = ingest(record, &IngestConfig::default()).expect("ingest should succeed");
        assert_eq!(rec.tenant_id, "tenant");
        assert_eq!(rec.doc_id, "doc");
        assert_eq!(rec.original_source.as_deref(), Some("source"));
    }

    #[test]
    fn required_tenant_id_enforced() {
        let record = RawIngestRecord {
            id: "ingest-required-tenant".into(),
            source: IngestSource::RawText,
            metadata: IngestMetadata {
                tenant_id: None,
                doc_id: Some("doc".into()),
                received_at: Some(fixed_timestamp()),
                original_source: None,
                attributes: None,
            },
            payload: Some(IngestPayload::Text("payload".into())),
        };

        let cfg = IngestConfig {
            metadata_policy: MetadataPolicy {
                required_fields: vec![RequiredField::TenantId],
                ..Default::default()
            },
            ..Default::default()
        };

        let res = ingest(record, &cfg);
        assert!(matches!(res, Err(IngestError::InvalidMetadata(_))));
    }

    #[test]
    fn future_timestamp_rejected() {
        let future = Utc::now() + Duration::days(1);
        let record = RawIngestRecord {
            id: "ingest-future-ts".into(),
            source: IngestSource::Api,
            metadata: IngestMetadata {
                tenant_id: Some("tenant".into()),
                doc_id: Some("doc".into()),
                received_at: Some(future),
                original_source: None,
                attributes: None,
            },
            payload: None,
        };

        let cfg = IngestConfig {
            metadata_policy: MetadataPolicy {
                reject_future_timestamps: true,
                ..Default::default()
            },
            ..Default::default()
        };

        let res = ingest(record, &cfg);
        assert!(matches!(res, Err(IngestError::InvalidMetadata(msg)) if msg.contains("future")));
    }

    #[test]
    fn max_attribute_bytes_enforced() {
        let record = RawIngestRecord {
            id: "ingest-attrs".into(),
            source: IngestSource::Api,
            metadata: IngestMetadata {
                tenant_id: Some("tenant".into()),
                doc_id: Some("doc".into()),
                received_at: Some(fixed_timestamp()),
                original_source: None,
                attributes: Some(serde_json::json!({
                    "blob": "x".repeat(32)
                })),
            },
            payload: None,
        };

        let cfg = IngestConfig {
            metadata_policy: MetadataPolicy {
                max_attribute_bytes: Some(16),
                ..Default::default()
            },
            ..Default::default()
        };

        let res = ingest(record, &cfg);
        assert!(
            matches!(res, Err(IngestError::InvalidMetadata(msg)) if msg.contains("attributes exceed"))
        );
    }

    #[test]
    fn test_ingest_empty_binary_payload() {
        let record = RawIngestRecord {
            id: "ingest-empty-binary".into(),
            source: IngestSource::File {
                filename: "empty.bin".into(),
                content_type: Some("application/octet-stream".into()),
            },
            metadata: base_metadata(),
            payload: Some(IngestPayload::Binary(vec![])),
        };

        let res = ingest(record, &IngestConfig::default());
        assert!(matches!(res, Err(IngestError::EmptyBinaryPayload)));
    }

    #[test]
    fn max_payload_bytes_enforced_text() {
        let record = RawIngestRecord {
            id: "ingest-payload-limit".into(),
            source: IngestSource::RawText,
            metadata: base_metadata(),
            payload: Some(IngestPayload::Text("x".repeat(17))),
        };

        let cfg = IngestConfig {
            max_payload_bytes: Some(16),
            ..Default::default()
        };

        let res = ingest(record, &cfg);
        assert!(
            matches!(res, Err(IngestError::PayloadTooLarge(msg)) if msg.contains("raw payload"))
        );
    }

    #[test]
    fn max_payload_bytes_enforced_bytes() {
        let record = RawIngestRecord {
            id: "ingest-payload-limit-bytes".into(),
            source: IngestSource::RawText,
            metadata: base_metadata(),
            payload: Some(IngestPayload::TextBytes(vec![b'x'; 17])),
        };

        let cfg = IngestConfig {
            max_payload_bytes: Some(16),
            ..Default::default()
        };

        let res = ingest(record, &cfg);
        assert!(
            matches!(res, Err(IngestError::PayloadTooLarge(msg)) if msg.contains("raw payload"))
        );
    }

    #[test]
    fn max_payload_bytes_enforced_binary() {
        let record = RawIngestRecord {
            id: "ingest-payload-limit-binary".into(),
            source: IngestSource::File {
                filename: "large.bin".into(),
                content_type: None,
            },
            metadata: base_metadata(),
            payload: Some(IngestPayload::Binary(vec![0; 17])),
        };

        let cfg = IngestConfig {
            max_payload_bytes: Some(16),
            ..Default::default()
        };

        let res = ingest(record, &cfg);
        assert!(
            matches!(res, Err(IngestError::PayloadTooLarge(msg)) if msg.contains("raw payload"))
        );
    }

    #[test]
    fn max_normalized_bytes_enforced() {
        let record = RawIngestRecord {
            id: "ingest-norm-limit".into(),
            source: IngestSource::RawText,
            metadata: base_metadata(),
            payload: Some(IngestPayload::Text("a ".repeat(9))), // Raw: 18 bytes
        };

        let cfg = IngestConfig {
            max_payload_bytes: Some(20),
            max_normalized_bytes: Some(16),
            ..Default::default()
        };

        let res = ingest(record, &cfg);
        // Normalizes to "a a a a a a a a a", which is 17 bytes long
        assert!(
            matches!(res, Err(IngestError::PayloadTooLarge(msg)) if msg.contains("normalized payload"))
        );
    }

    #[test]
    fn payload_size_limits_respected() {
        let record = RawIngestRecord {
            id: "ingest-limits-ok".into(),
            source: IngestSource::RawText,
            metadata: base_metadata(),
            payload: Some(IngestPayload::Text(" data data ".into())), // Raw: 11, Normalized: 9
        };

        let cfg = IngestConfig {
            max_payload_bytes: Some(12),
            max_normalized_bytes: Some(10),
            ..Default::default()
        };

        let res = ingest(record, &cfg);
        assert!(res.is_ok());
        let rec = res.unwrap();
        assert_eq!(
            rec.normalized_payload,
            Some(CanonicalPayload::Text("data data".into()))
        );
    }
}
