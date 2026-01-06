//! Metadata normalization and policy enforcement for `ufp_ingest`.
//!
//! Functions in this module are responsible for sanitizing metadata, applying
//! defaults, and enforcing configured [`MetadataPolicy`](crate::MetadataPolicy)
//! rules before records flow further down the pipeline.
use chrono::{DateTime, Utc};

use crate::config::{IngestConfig, MetadataPolicy, RequiredField};
use crate::error::IngestError;
use crate::types::IngestMetadata;

/// Holds the result of metadata normalization.
pub(crate) struct NormalizedMetadata {
    pub(crate) tenant_id: String,
    pub(crate) doc_id: String,
    pub(crate) received_at: DateTime<Utc>,
    pub(crate) original_source: Option<String>,
    pub(crate) attributes: Option<serde_json::Value>,
}

/// Normalizes and validates metadata fields.
pub(crate) fn normalize_metadata(
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
    // Enforce size limits on the attributes JSON blob to prevent abuse.
    enforce_attribute_limit(&mut attributes, &cfg.metadata_policy)?;

    // Sanitize and apply defaults to tenant ID.
    let tenant_id_clean = sanitize_optional_string(tenant_id, cfg.strip_control_chars);
    enforce_required_metadata(
        &cfg.metadata_policy,
        RequiredField::TenantId,
        tenant_id_clean.is_some(),
    )?;
    let tenant_id = tenant_id_clean.unwrap_or_else(|| cfg.default_tenant_id.clone());

    // Sanitize and derive doc ID if not present.
    let doc_id_clean = sanitize_optional_string(doc_id, cfg.strip_control_chars);
    enforce_required_metadata(
        &cfg.metadata_policy,
        RequiredField::DocId,
        doc_id_clean.is_some(),
    )?;
    let doc_id = doc_id_clean.unwrap_or_else(|| derive_doc_id(cfg, &tenant_id, record_id));

    // Validate and default the received timestamp.
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

    // Sanitize original source.
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

/// Derives a deterministic doc ID from the tenant and record IDs.
pub(crate) fn derive_doc_id(cfg: &IngestConfig, tenant_id: &str, record_id: &str) -> String {
    // Use a UUIDv5 to create a deterministic ID based on the namespace and a name.
    let mut material = Vec::with_capacity(tenant_id.len() + record_id.len() + 1);
    material.extend_from_slice(tenant_id.as_bytes());
    material.push(0); // Separator to prevent collisions.
    material.extend_from_slice(record_id.as_bytes());
    uuid::Uuid::new_v5(&cfg.doc_id_namespace, &material).to_string()
}

/// Checks if the serialized attributes exceed the configured limit.
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

/// Enforces that a required metadata field is present.
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

/// Sanitizes an optional string by stripping control characters and trimming whitespace.
pub(crate) fn sanitize_optional_string(
    value: Option<String>,
    strip_control: bool,
) -> Option<String> {
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

/// Sanitizes a required string field.
pub(crate) fn sanitize_required_field(
    field: &str,
    value: String,
    strip_control: bool,
) -> Result<String, IngestError> {
    sanitize_optional_string(Some(value), strip_control)
        .ok_or_else(|| IngestError::InvalidMetadata(format!("{field} empty")))
}
