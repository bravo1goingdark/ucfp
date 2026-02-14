//! Metadata normalization and policy enforcement for the ingest crate.
//!
//! This module contains functions for sanitizing metadata, applying defaults,
//! and enforcing configured [`MetadataPolicy`] rules before records flow further
//! down the pipeline.
//!
//! # Responsibilities
//!
//! - **Sanitization**: Strip control characters from metadata strings
//! - **Default Application**: Fill in missing tenant_id, doc_id, received_at
//! - **Policy Enforcement**: Validate required fields, size limits, timestamps
//! - **ID Generation**: Derive deterministic document IDs using UUIDv5
//!
//! # Metadata Flow
//!
//! ```text
//! IngestMetadata (raw)
//!        │
//!        ▼
//! ┌─────────────────────────────┐
//! │ 1. Sanitize strings         │
//! │    - Strip control chars    │
//! │    - Trim whitespace        │
//! ├─────────────────────────────┤
//! │ 2. Apply defaults           │
//! │    - tenant_id              │
//! │    - doc_id (UUIDv5)        │
//! │    - received_at            │
//! ├─────────────────────────────┤
//! │ 3. Validate policies        │
//! │    - Required fields        │
//! │    - Attributes size        │
//! │    - Future timestamps      │
//! └─────────────────────────────┘
//!        │
//!        ▼
//! NormalizedMetadata (canonical)
//! ```
//!
//! # Examples
//!
//! ```rust
//! use ingest::{IngestConfig, RawIngestRecord, IngestMetadata, IngestSource};
//!
//! let config = IngestConfig::default();
//! let record = RawIngestRecord {
//!     id: "test".to_string(),
//!     source: IngestSource::Api,
//!     metadata: IngestMetadata {
//!         tenant_id: None, // Will default
//!         doc_id: None,    // Will derive
//!         received_at: None, // Will default to now
//!         original_source: None,
//!         attributes: None,
//!     },
//!     payload: None,
//! };
//!
//! // After ingest, all fields will be populated
//! ```
use chrono::{DateTime, Utc};

use crate::config::{IngestConfig, MetadataPolicy, RequiredField};
use crate::error::IngestError;
use crate::types::IngestMetadata;

/// Holds the result of metadata normalization.
///
/// This internal struct represents metadata after all sanitization, defaulting,
/// and validation has been applied. All fields are guaranteed to be present
/// (non-optional), unlike [`IngestMetadata`] which has many optional fields.
///
/// # Fields
///
/// - `tenant_id`: Non-empty tenant identifier
/// - `doc_id`: Non-empty document identifier (derived or provided)
/// - `received_at`: Valid timestamp (provided or current time)
/// - `original_source`: Sanitized optional source reference
/// - `attributes`: Size-checked optional JSON attributes
///
/// # Example
///
/// ```rust,ignore
/// use ingest::metadata::NormalizedMetadata;
///
/// let normalized = NormalizedMetadata {
///     tenant_id: "acme-corp".to_string(),
///     doc_id: "doc-123".to_string(),
///     received_at: chrono::Utc::now(),
///     original_source: Some("https://example.com".to_string()),
///     attributes: None,
/// };
/// ```
pub(crate) struct NormalizedMetadata {
    /// Normalized tenant identifier (default applied if missing).
    ///
    /// This will never be empty. If the input was empty or missing,
    /// [`IngestConfig::default_tenant_id`] was used.
    pub(crate) tenant_id: String,

    /// Normalized document identifier (derived if missing).
    ///
    /// This will never be empty. If the input was empty or missing,
    /// a UUIDv5 was derived using [`derive_doc_id`].
    pub(crate) doc_id: String,

    /// Normalized timestamp (defaulted to now if missing).
    ///
    /// This will always be a valid timestamp. If the input was missing,
    /// the current UTC time was used.
    pub(crate) received_at: DateTime<Utc>,

    /// Sanitized original source information.
    ///
    /// Control characters have been stripped if
    /// [`IngestConfig::strip_control_chars`] is enabled.
    pub(crate) original_source: Option<String>,

    /// Preserved arbitrary attributes.
    ///
    /// The JSON has been size-checked against
    /// [`MetadataPolicy::max_attribute_bytes`] if configured.
    pub(crate) attributes: Option<serde_json::Value>,
}

/// Normalizes and validates metadata fields.
///
/// This is the main entry point for metadata processing. It performs:
///
/// 1. **Attribute size enforcement**: Check attributes against limit
/// 2. **Tenant ID sanitization**: Strip control chars, apply default
/// 3. **Doc ID sanitization**: Strip control chars, derive if missing
/// 4. **Timestamp validation**: Check for future timestamps, apply default
/// 5. **Original source sanitization**: Strip control chars
/// 6. **Required field validation**: Ensure mandatory fields are present
///
/// # Arguments
///
/// * `metadata` - Raw metadata from the ingest request
/// * `cfg` - Configuration controlling normalization behavior
/// * `record_id` - The sanitized record ID (used for doc ID derivation)
///
/// # Returns
///
/// - `Ok(NormalizedMetadata)` - Successfully normalized metadata
/// - `Err(IngestError)` - Validation or normalization failure
///
/// # Errors
///
/// - [`IngestError::InvalidMetadata`] - Required field missing, attributes too large,
///   or future timestamp detected
///
/// # Examples
///
/// ```rust,ignore
/// use ingest::metadata::normalize_metadata;
/// use ingest::{IngestConfig, IngestMetadata};
///
/// let metadata = IngestMetadata {
///     tenant_id: Some("tenant\x07".to_string()), // Has control char
///     doc_id: None, // Will be derived
///     received_at: None, // Will default to now
///     original_source: None,
///     attributes: None,
/// };
///
/// let config = IngestConfig::default();
/// let normalized = normalize_metadata(metadata, &config, "record-123").unwrap();
///
/// assert_eq!(normalized.tenant_id, "tenant"); // Control char stripped
/// assert!(!normalized.doc_id.is_empty()); // Derived
/// ```
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

/// Derives a deterministic document ID from the tenant and record IDs.
///
/// This function creates a UUIDv5 that is stable across ingest operations.
/// Given the same tenant, record ID, and namespace, it will always produce
/// the same document ID.
///
/// # Algorithm
///
/// ```text
/// material = tenant_id + "\0" + record_id
/// doc_id = UUIDv5(config.doc_id_namespace, material)
/// ```
///
/// The null byte separator prevents collisions between different tenant/record
/// combinations (e.g., ("foo", "bar") vs ("fo", "obar")).
///
/// # Arguments
///
/// * `cfg` - Configuration containing the UUID namespace
/// * `tenant_id` - The normalized tenant identifier
/// * `record_id` - The sanitized record ID
///
/// # Returns
///
/// A UUID string in the standard format (e.g., "550e8400-e29b-41d4-a716-446655440000")
///
/// # Examples
///
/// ```rust,ignore
/// use ingest::metadata::derive_doc_id;
/// use ingest::IngestConfig;
///
/// let config = IngestConfig::default();
/// let doc_id = derive_doc_id(&config, "acme-corp", "record-123");
///
/// // Same inputs = same output
/// let doc_id2 = derive_doc_id(&config, "acme-corp", "record-123");
/// assert_eq!(doc_id, doc_id2);
///
/// // Different inputs = different output
/// let doc_id3 = derive_doc_id(&config, "other-corp", "record-123");
/// assert_ne!(doc_id, doc_id3);
/// ```
pub(crate) fn derive_doc_id(cfg: &IngestConfig, tenant_id: &str, record_id: &str) -> String {
    // Use a UUIDv5 to create a deterministic ID based on the namespace and a name.
    // Optimize for common case: use stack buffer for typical sizes to avoid allocation.
    const STACK_BUF_SIZE: usize = 256;
    let total_len = tenant_id.len() + record_id.len() + 1;

    if total_len <= STACK_BUF_SIZE {
        // Fast path: stack-allocated buffer for common sizes
        let mut material = [0u8; STACK_BUF_SIZE];
        material[..tenant_id.len()].copy_from_slice(tenant_id.as_bytes());
        material[tenant_id.len()] = 0; // Separator to prevent collisions.
        material[tenant_id.len() + 1..total_len].copy_from_slice(record_id.as_bytes());
        uuid::Uuid::new_v5(&cfg.doc_id_namespace, &material[..total_len]).to_string()
    } else {
        // Slow path: heap allocation for large inputs
        let mut material = Vec::with_capacity(total_len);
        material.extend_from_slice(tenant_id.as_bytes());
        material.push(0); // Separator to prevent collisions.
        material.extend_from_slice(record_id.as_bytes());
        uuid::Uuid::new_v5(&cfg.doc_id_namespace, &material).to_string()
    }
}

/// Checks if the serialized attributes exceed the configured limit.
///
/// This function serializes the attributes JSON value and checks its byte size
/// against the configured limit. If the limit is exceeded, an error is returned.
///
/// # Arguments
///
/// * `attributes` - The optional attributes JSON value (modified in place to None if oversized)
/// * `policy` - The metadata policy containing the size limit
///
/// # Returns
///
/// - `Ok(())` - Attributes are within limit or no limit configured
/// - `Err(IngestError::InvalidMetadata)` - Attributes exceed size limit
///
/// # Errors
///
/// Returns error if:
/// - Attributes JSON serialization fails
/// - Serialized size exceeds `max_attribute_bytes`
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
///
/// This function checks if a field marked as required in the policy is actually
/// present (after sanitization and defaulting).
///
/// # Arguments
///
/// * `policy` - The metadata policy containing required fields
/// * `field` - The field to check
/// * `present` - Whether the field is present (after sanitization)
///
/// # Returns
///
/// - `Ok(())` - Field is present or not required
/// - `Err(IngestError::InvalidMetadata)` - Required field is missing
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
///
/// This function performs the following transformations:
/// - If `strip_control` is true, removes all ASCII control characters (0x00-0x1F, 0x7F)
/// - Trims leading and trailing whitespace
/// - Returns `None` if the result is empty
///
/// # Arguments
///
/// * `value` - The optional string to sanitize
/// * `strip_control` - Whether to strip control characters
///
/// # Returns
///
/// - `Some(String)` - Sanitized non-empty string
/// - `None` - Input was None, empty, or became empty after sanitization
///
/// # Examples
///
/// ```rust,ignore
/// use ingest::metadata::sanitize_optional_string;
///
/// // Strip control characters and trim
/// let result = sanitize_optional_string(
///     Some("  Hello\x07World  ".to_string()),
///     true
/// );
/// assert_eq!(result, Some("HelloWorld".to_string()));
///
/// // Empty after trimming
/// let result = sanitize_optional_string(Some("   ".to_string()), true);
/// assert_eq!(result, None);
///
/// // None input
/// let result = sanitize_optional_string(None, true);
/// assert_eq!(result, None);
/// ```
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
///
/// Similar to [`sanitize_optional_string`] but returns an error if the result
/// is empty. Used for fields that must have a value (like the record ID).
///
/// # Arguments
///
/// * `field` - The field name (for error messages)
/// * `value` - The string to sanitize (must be present)
/// * `strip_control` - Whether to strip control characters
///
/// # Returns
///
/// - `Ok(String)` - Sanitized non-empty string
/// - `Err(IngestError::InvalidMetadata)` - Result is empty after sanitization
///
/// # Errors
///
/// Returns error if the sanitized string is empty.
pub(crate) fn sanitize_required_field(
    field: &str,
    value: String,
    strip_control: bool,
) -> Result<String, IngestError> {
    sanitize_optional_string(Some(value), strip_control)
        .ok_or_else(|| IngestError::InvalidMetadata(format!("{field} empty")))
}
