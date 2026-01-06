//! # UCFP Ingest Layer
//!
//! This crate serves as the entry point for the Universal Content Fingerprinting
//! (UCFP) pipeline. It is responsible for receiving raw content and metadata,
//! validating it against a set of configurable policies, and normalizing it into
//! a canonical format that can be consumed by downstream processing stages.
//!
//! ## Core Responsibilities
//!
//! - **Metadata Validation and Normalization**: Enforces the presence of required
//!   metadata fields (e.g., `tenant_id`, `doc_id`), applies default values when
//!   they are missing, and sanitizes fields by stripping control characters.
//! - **Deterministic ID Generation**: Derives a deterministic document ID (UUIDv5)
//!   when one is not provided, ensuring content can be reliably traced.
//! - **Payload Handling**: Accepts text and binary payloads, decodes UTF-8 from
//!   byte streams, and normalizes whitespace in text payloads.
//! - **Policy Enforcement**: Allows for the definition of strict metadata policies,
//!   such as rejecting timestamps from the future or limiting the size of
//!   arbitrary attribute blobs.
//! - **Observability**: Emits structured logs with detailed context for both
//!   successful and failed ingest operations, facilitating monitoring and debugging.
//!
//! ## Key Concepts
//!
//! The [`ingest`] function is the primary entry point. It takes a [`RawIngestRecord`]
//! and an [`IngestConfig`] and produces a [`CanonicalIngestRecord`]. The process
//! is designed to be robust and fault-tolerant, with clear error types to
//! indicate the reason for failure.
//!
//! The design emphasizes a clean separation between the core ingestion logic and
//! the surrounding observability infrastructure, using `tracing` to provide rich,
//! contextual logs without cluttering the main business logic.
//!
//! ## Example Usage
//!
//! ```
//! use ufp_ingest::{ingest, IngestConfig, RawIngestRecord, IngestSource, IngestMetadata, IngestPayload};
//! use chrono::Utc;
//!
//! let config = IngestConfig::default();
//! let record = RawIngestRecord {
//!     id: "my-doc-1".into(),
//!     source: IngestSource::RawText,
//!     metadata: IngestMetadata {
//!         tenant_id: Some("my-tenant".into()),
//!         doc_id: None,
//!         received_at: Some(Utc::now()),
//!         original_source: None,
//!         attributes: None,
//!     },
//!     payload: Some(IngestPayload::Text("  Some text with   extra whitespace.  ".into())),
//! };
//!
//! let canonical_record = ingest(record, &config).unwrap();
//!
//! assert_eq!(canonical_record.tenant_id, "my-tenant");
//! // The payload is normalized.
//! // assert_eq!(canonical_record.normalized_payload, Some(ufp_ingest::CanonicalPayload::Text("Some text with extra whitespace.".into())));
//! ```
//
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

/// Observer hook for ingest operations.
///
/// Implementors can use this trait to attach custom metrics or logging
/// without having to wrap or fork the ingest logic. All callback methods
/// have default no-op implementations so you can override only what you need.
pub trait IngestObserver {
    /// Called after a successful ingest.
    ///
    /// `elapsed_micros` measures the time spent in [`ingest_with_observer`],
    /// including validation and normalization.
    fn on_success(&self, _record: &CanonicalIngestRecord, _elapsed_micros: u128) {}

    /// Called after a failed ingest.
    ///
    /// `elapsed_micros` measures the time spent in [`ingest_with_observer`]
    /// up to the point of failure.
    fn on_failure(&self, _error: &IngestError, _elapsed_micros: u128) {}
}

struct NoopObserver;

impl IngestObserver for NoopObserver {}

/// Public ingest function. It validates metadata, normalizes payload (trims and collapses
/// whitespace), and returns a canonical record for the canonical stage.
///
/// This convenience wrapper delegates to [`ingest_with_observer`] with an internal
/// no-op observer.
pub fn ingest(
    raw: RawIngestRecord,
    cfg: &IngestConfig,
) -> Result<CanonicalIngestRecord, IngestError> {
    ingest_with_observer(raw, cfg, &NoopObserver)
}

/// Ingest function with an explicit observer hook.
///
/// This variant behaves like [`ingest`], but additionally notifies the supplied
/// [`IngestObserver`] of successes and failures, making it easy to integrate
/// with metrics or custom logging.
pub fn ingest_with_observer<O>(
    raw: RawIngestRecord,
    cfg: &IngestConfig,
    observer: &O,
) -> Result<CanonicalIngestRecord, IngestError>
where
    O: IngestObserver + ?Sized,
{
    // Start timer for observability.
    let start = Instant::now();
    let RawIngestRecord {
        id,
        source,
        metadata,
        payload,
    } = raw;

    // Keep hints for logging in case of failure.
    let tenant_hint = metadata.tenant_id.clone();
    let doc_hint = metadata.doc_id.clone();

    // The record ID is crucial for tracing, so it's sanitized and validated first.
    let record_id = match metadata::sanitize_required_field("id", id, cfg.strip_control_chars) {
        Ok(id) => id,
        Err(err) => {
            let elapsed_micros = start.elapsed().as_micros();
            warn!(error = %err, elapsed_micros, "ingest_failure");
            observer.on_failure(&err, elapsed_micros);
            return Err(err);
        }
    };

    // Create a tracing span to group all logs related to this ingest operation.
    let span = tracing::span!(
        Level::INFO,
        "ufp_ingest.ingest",
        record_id = %record_id,
        source = ?source
    );
    let _guard = span.enter();

    // The core logic is in a separate function to keep this one focused on observability.
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
            observer.on_success(&record, elapsed_micros);
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
            observer.on_failure(&err, elapsed_micros);
            Err(err)
        }
    }
}

/// Core ingest logic: validates payload, normalizes metadata and payload.
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
