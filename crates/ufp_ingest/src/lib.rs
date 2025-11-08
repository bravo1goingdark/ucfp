//! Ingest layer for text-based UCFP.
//! Provides public API for receiving inputs, normalizing metadata, basic validation,
//! and producing a canonical ingest record ready for canonicalizer.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use thiserror::Error;
use tracing::{Level, info, warn};
use uuid::Uuid;

/// Runtime configuration for ingest behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestConfig {
    /// Semantic version of the ingest configuration.
    pub version: u32,
    /// Default tenant id to fall back on when metadata omits it.
    pub default_tenant_id: String,
    /// Namespace UUID used to deterministically derive doc ids when absent.
    pub doc_id_namespace: Uuid,
    /// Whether to strip ASCII control characters from metadata.
    pub strip_control_chars: bool,
    /// Additional metadata validation policies.
    #[serde(default)]
    pub metadata_policy: MetadataPolicy,
}

/// Controls which metadata fields must be present and how optional blobs are constrained.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MetadataPolicy {
    /// Metadata fields that must be provided by the caller (after sanitization).
    pub required_fields: Vec<RequiredField>,
    /// Maximum serialized byte length allowed for `metadata.attributes`.
    pub max_attribute_bytes: Option<usize>,
    /// Reject ingests with timestamps that lie in the future.
    pub reject_future_timestamps: bool,
}

/// Metadata identifiers that can be enforced via `MetadataPolicy`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RequiredField {
    TenantId,
    DocId,
    ReceivedAt,
    OriginalSource,
}

impl Default for IngestConfig {
    fn default() -> Self {
        Self {
            version: 1,
            default_tenant_id: "default".into(),
            doc_id_namespace: Uuid::NAMESPACE_OID,
            strip_control_chars: true,
            metadata_policy: MetadataPolicy::default(),
        }
    }
}

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
    /// Optional tenant id; defaults to config default when None/empty.
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
    /// Raw UTF-8 bytes that will be decoded during ingest.
    TextBytes(Vec<u8>),
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
    #[error("invalid utf-8 payload: {0}")]
    InvalidUtf8(String),
    #[error("text payload empty after normalization")]
    EmptyNormalizedText,
}

/// Public ingest function. It validates metadata, normalizes payload (trims and collapses whitespace),
/// and returns a canonical record for the canonicalizer stage.
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

    let record_id = match sanitize_required_field("id", id, cfg.strip_control_chars) {
        Ok(id) => id,
        Err(err) => {
            warn!(error = %err, "ingest_failure");
            return Err(err);
        }
    };

    let span = tracing::span!(
        Level::INFO,
        "ufp_ingest.ingest",
        record_id = %record_id,
        source = debug(&source)
    );
    let _guard = span.enter();

    match ingest_inner(record_id.clone(), source, metadata, payload, cfg) {
        Ok(record) => {
            info!(
                tenant_id = %record.tenant_id,
                doc_id = %record.doc_id,
                payload_kind = %payload_kind(record.normalized_payload.as_ref()),
                normalized_len = payload_length(record.normalized_payload.as_ref()),
                elapsed_micros = start.elapsed().as_micros(),
                "ingest_success"
            );
            Ok(record)
        }
        Err(err) => {
            warn!(
                tenant_id = ?tenant_hint,
                doc_id = ?doc_hint,
                error = %err,
                "ingest_failure"
            );
            Err(err)
        }
    }
}

fn ingest_inner(
    record_id: String,
    source: IngestSource,
    metadata: IngestMetadata,
    payload: Option<IngestPayload>,
    cfg: &IngestConfig,
) -> Result<CanonicalIngestRecord, IngestError> {
    validate_payload_requirements(&source, &payload)?;

    let normalized_metadata = normalize_metadata(metadata, cfg, &record_id)?;
    let normalized_payload = normalize_payload_option(&source, payload)?;

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

struct NormalizedMetadata {
    tenant_id: String,
    doc_id: String,
    received_at: DateTime<Utc>,
    original_source: Option<String>,
    attributes: Option<serde_json::Value>,
}

fn normalize_metadata(
    metadata: IngestMetadata,
    cfg: &IngestConfig,
    record_id: &str,
) -> Result<NormalizedMetadata, IngestError> {
    let IngestMetadata {
        tenant_id,
        doc_id,
        received_at,
        original_source,
        attributes,
    } = metadata;

    let mut attributes = attributes;
    enforce_attribute_limit(&mut attributes, &cfg.metadata_policy)?;

    let tenant_id_clean = sanitize_optional_string(tenant_id, cfg.strip_control_chars);
    enforce_required_metadata(
        &cfg.metadata_policy,
        RequiredField::TenantId,
        tenant_id_clean.is_some(),
    )?;
    let tenant_id = tenant_id_clean.unwrap_or_else(|| cfg.default_tenant_id.clone());

    let doc_id_clean = sanitize_optional_string(doc_id, cfg.strip_control_chars);
    enforce_required_metadata(
        &cfg.metadata_policy,
        RequiredField::DocId,
        doc_id_clean.is_some(),
    )?;
    let doc_id = doc_id_clean.unwrap_or_else(|| derive_doc_id(cfg, &tenant_id, record_id));

    let received_at_opt = received_at;
    enforce_required_metadata(
        &cfg.metadata_policy,
        RequiredField::ReceivedAt,
        received_at_opt.is_some(),
    )?;
    let now = Utc::now();
    if cfg.metadata_policy.reject_future_timestamps
        && matches!(received_at_opt.as_ref(), Some(ts) if *ts > now)
    {
        return Err(IngestError::InvalidMetadata(
            "received_at lies in the future".into(),
        ));
    }
    let received_at = received_at_opt.unwrap_or(now);

    let original_source = sanitize_optional_string(original_source, cfg.strip_control_chars);
    enforce_required_metadata(
        &cfg.metadata_policy,
        RequiredField::OriginalSource,
        original_source.is_some(),
    )?;

    Ok(NormalizedMetadata {
        tenant_id,
        doc_id,
        received_at,
        original_source,
        attributes,
    })
}

fn derive_doc_id(cfg: &IngestConfig, tenant_id: &str, record_id: &str) -> String {
    let mut material = Vec::with_capacity(tenant_id.len() + record_id.len() + 1);
    material.extend_from_slice(tenant_id.as_bytes());
    material.push(0);
    material.extend_from_slice(record_id.as_bytes());
    Uuid::new_v5(&cfg.doc_id_namespace, &material).to_string()
}

fn enforce_attribute_limit(
    attributes: &mut Option<serde_json::Value>,
    policy: &MetadataPolicy,
) -> Result<(), IngestError> {
    if let (Some(limit), Some(ref value)) = (policy.max_attribute_bytes, attributes.as_ref()) {
        let serialized = serde_json::to_vec(value).map_err(|err| {
            IngestError::InvalidMetadata(format!("attributes serialization failed: {err}"))
        })?;
        if serialized.len() > limit {
            return Err(IngestError::InvalidMetadata(format!(
                "attributes exceed {limit} bytes (got {})",
                serialized.len()
            )));
        }
    }
    Ok(())
}

fn enforce_required_metadata(
    policy: &MetadataPolicy,
    field: RequiredField,
    present: bool,
) -> Result<(), IngestError> {
    if policy.required_fields.contains(&field) && !present {
        return Err(IngestError::InvalidMetadata(format!(
            "{field:?} is required by ingest policy"
        )));
    }
    Ok(())
}

fn validate_payload_requirements(
    source: &IngestSource,
    payload: &Option<IngestPayload>,
) -> Result<(), IngestError> {
    let has_payload = payload.is_some();
    if source_requires_payload(source) && !has_payload {
        return Err(IngestError::MissingPayload);
    }
    Ok(())
}

fn source_requires_payload(source: &IngestSource) -> bool {
    matches!(source, IngestSource::RawText | IngestSource::File { .. })
}

fn normalize_payload_option(
    source: &IngestSource,
    payload: Option<IngestPayload>,
) -> Result<Option<CanonicalPayload>, IngestError> {
    let payload = match payload {
        Some(value) => value,
        None => return Ok(None),
    };

    let canonical = normalize_payload_value(payload)?;
    if source_requires_text_payload(source) && !matches!(canonical, CanonicalPayload::Text(_)) {
        return Err(IngestError::InvalidMetadata(
            "text-based source requires text payload".into(),
        ));
    }
    Ok(Some(canonical))
}

fn source_requires_text_payload(source: &IngestSource) -> bool {
    matches!(source, IngestSource::RawText | IngestSource::Url(_))
}

fn normalize_payload_value(payload: IngestPayload) -> Result<CanonicalPayload, IngestError> {
    match payload {
        IngestPayload::Text(text) => normalize_text_payload(text),
        IngestPayload::TextBytes(bytes) => {
            let text = String::from_utf8(bytes)
                .map_err(|err| IngestError::InvalidUtf8(err.to_string()))?;
            normalize_text_payload(text)
        }
        IngestPayload::Binary(bytes) => {
            if bytes.is_empty() {
                Err(IngestError::MissingPayload)
            } else {
                Ok(CanonicalPayload::Binary(bytes))
            }
        }
    }
}

fn normalize_text_payload(text: String) -> Result<CanonicalPayload, IngestError> {
    let normalized = normalize_payload(&text);
    if normalized.is_empty() {
        Err(IngestError::EmptyNormalizedText)
    } else {
        Ok(CanonicalPayload::Text(normalized))
    }
}

fn sanitize_optional_string(value: Option<String>, strip_control: bool) -> Option<String> {
    value.and_then(|raw| {
        let filtered = if strip_control {
            raw.chars().filter(|c| !c.is_control()).collect::<String>()
        } else {
            raw
        };
        let trimmed = filtered.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn sanitize_required_field(
    field: &str,
    value: String,
    strip_control: bool,
) -> Result<String, IngestError> {
    sanitize_optional_string(Some(value), strip_control)
        .ok_or_else(|| IngestError::InvalidMetadata(format!("{field} empty")))
}

fn payload_kind(payload: Option<&CanonicalPayload>) -> &'static str {
    match payload {
        Some(CanonicalPayload::Text(_)) => "text",
        Some(CanonicalPayload::Binary(_)) => "binary",
        None => "none",
    }
}

fn payload_length(payload: Option<&CanonicalPayload>) -> usize {
    match payload {
        Some(CanonicalPayload::Text(text)) => text.len(),
        Some(CanonicalPayload::Binary(bytes)) => bytes.len(),
        None => 0,
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
    use chrono::{DateTime, Duration, NaiveDate, Utc};

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
}
