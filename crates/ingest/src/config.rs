//! Configuration types for the ingest pipeline.
//!
//! This module defines [`IngestConfig`] and [`MetadataPolicy`], which control how
//! raw ingest requests are interpreted, defaulted, and constrained at runtime.
//! These types are intended to be cheap to clone and easy to serialize from
//! external configuration formats such as JSON, TOML, or YAML.
//!
//! # Quick Start
//!
//! ```rust
//! use ingest::IngestConfig;
//!
//! // Use defaults for development
//! let config = IngestConfig::default();
//!
//! // Validate before use
//! config.validate().expect("Invalid configuration");
//! ```
//!
//! # Production Configuration
//!
//! ```rust
//! use ingest::{IngestConfig, MetadataPolicy, RequiredField};
//! use uuid::Uuid;
//!
//! let config = IngestConfig {
//!     version: 1,
//!     default_tenant_id: "production".to_string(),
//!     doc_id_namespace: Uuid::new_v5(
//!         &Uuid::NAMESPACE_DNS,
//!         b"myapp.example.com"
//!     ),
//!     strip_control_chars: true,
//!     metadata_policy: MetadataPolicy {
//!         required_fields: vec![
//!             RequiredField::TenantId,
//!             RequiredField::DocId,
//!         ],
//!         max_attribute_bytes: Some(1024 * 1024), // 1 MB
//!         reject_future_timestamps: true,
//!     },
//!     max_payload_bytes: Some(100 * 1024 * 1024),     // 100 MB
//!     max_normalized_bytes: Some(50 * 1024 * 1024),   // 50 MB
//! };
//!
//! // Always validate at startup
//! if let Err(e) = config.validate() {
//!     eprintln!("Configuration error: {}", e);
//!     std::process::exit(1);
//! }
//! ```
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Runtime configuration for ingest behavior.
///
/// `IngestConfig` controls all aspects of the ingest pipeline including validation,
/// normalization, size limits, and ID generation. It is designed to be cheap to clone
/// and serializable for configuration management.
///
/// # Fields
///
/// - `version`: Semantic version for tracking configuration changes
/// - `default_tenant_id`: Fallback tenant when metadata doesn't specify one
/// - `doc_id_namespace`: UUID namespace for deterministic document ID generation
/// - `strip_control_chars`: Whether to remove control characters from metadata
/// - `metadata_policy`: Fine-grained metadata validation rules
/// - `max_payload_bytes`: Maximum raw payload size (optional)
/// - `max_normalized_bytes`: Maximum normalized text size (optional)
///
/// # Serialization
///
/// This struct supports JSON, TOML, and YAML serialization:
///
/// ```json
/// {
///   "version": 1,
///   "default_tenant_id": "default",
///   "strip_control_chars": true,
///   "max_payload_bytes": 52428800,
///   "max_normalized_bytes": 10485760,
///   "metadata_policy": {
///     "required_fields": ["TenantId", "DocId"],
///     "max_attribute_bytes": 1048576,
///     "reject_future_timestamps": true
///   }
/// }
/// ```
///
/// # Examples
///
/// ## Default Configuration
///
/// ```rust
/// use ingest::IngestConfig;
/// use uuid::Uuid;
///
/// let config = IngestConfig::default();
///
/// assert_eq!(config.version, 1);
/// assert_eq!(config.default_tenant_id, "default");
/// assert_eq!(config.strip_control_chars, true);
/// assert!(config.max_payload_bytes.is_none());
/// assert!(config.max_normalized_bytes.is_none());
/// ```
///
/// ## Custom Configuration
///
/// ```rust
/// use ingest::{IngestConfig, MetadataPolicy, RequiredField};
/// use uuid::Uuid;
///
/// let config = IngestConfig {
///     version: 2,
///     default_tenant_id: "my-app".to_string(),
///     doc_id_namespace: Uuid::new_v5(
///         &Uuid::NAMESPACE_DNS,
///         b"my-app.example.com"
///     ),
///     strip_control_chars: true,
///     metadata_policy: MetadataPolicy {
///         required_fields: vec![RequiredField::TenantId],
///         max_attribute_bytes: Some(65536),
///         reject_future_timestamps: true,
///     },
///     max_payload_bytes: Some(10 * 1024 * 1024),
///     max_normalized_bytes: Some(5 * 1024 * 1024),
/// };
///
/// assert!(config.validate().is_ok());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestConfig {
    /// Semantic version of the ingest configuration.
    ///
    /// This version number helps track configuration changes and can be used
    /// for schema migration or feature flagging. Increment this when making
    /// breaking changes to ingest behavior.
    ///
    /// Default: `1`
    pub version: u32,

    /// Default tenant ID to use when metadata doesn't specify one.
    ///
    /// This ensures every canonical record has a tenant identifier, enabling
    /// multi-tenant isolation even when callers omit the tenant field.
    ///
    /// Default: `"default"`
    pub default_tenant_id: String,

    /// Namespace UUID for deterministic document ID generation.
    ///
    /// When `doc_id` is not provided in metadata, a UUIDv5 is derived using:
    /// `UUIDv5(doc_id_namespace, tenant_id + "\0" + record_id)`
    ///
    /// Using a consistent namespace ensures that:
    /// - The same content always gets the same ID (deterministic)
    /// - Different applications don't collide (namespace isolation)
    /// - Re-ingesting content is idempotent
    ///
    /// Default: [`Uuid::NAMESPACE_OID`]
    pub doc_id_namespace: Uuid,

    /// Whether to strip ASCII control characters from metadata strings.
    ///
    /// When `true`, control characters (0x00-0x1F and 0x7F) are removed from:
    /// - `tenant_id`
    /// - `doc_id`
    /// - `original_source`
    /// - `id` (record ID)
    ///
    /// This prevents log injection attacks and ensures metadata is safe for
    /// downstream systems. It is strongly recommended to keep this enabled.
    ///
    /// Default: `true`
    pub strip_control_chars: bool,

    /// Additional metadata validation policies.
    ///
    /// Controls which fields are required, attribute size limits, and timestamp
    /// validation rules.
    ///
    /// Default: [`MetadataPolicy::default()`]
    #[serde(default)]
    pub metadata_policy: MetadataPolicy,

    /// Maximum raw payload byte length allowed.
    ///
    /// If set, payloads exceeding this limit are rejected with
    /// [`IngestError::PayloadTooLarge`] before any processing.
    ///
    /// This check is performed on the raw payload size before normalization
    /// (whitespace collapsing, UTF-8 decoding, etc.).
    ///
    /// # Size Recommendations
    ///
    /// - Small text: 1-10 MB
    /// - Documents: 50-100 MB
    /// - Large files: 500 MB - 1 GB (if memory allows)
    ///
    /// Default: `None` (unlimited)
    #[serde(default)]
    pub max_payload_bytes: Option<usize>,

    /// Maximum normalized payload byte length allowed.
    ///
    /// If set, text payloads exceeding this limit after whitespace normalization
    /// are rejected with [`IngestError::PayloadTooLarge`].
    ///
    /// This is useful for enforcing limits on processed content size, which
    /// may differ from raw size due to whitespace collapsing.
    ///
    /// # Constraint
    ///
    /// Must be less than or equal to `max_payload_bytes` (validated by
    /// [`IngestConfig::validate()`]).
    ///
    /// Default: `None` (unlimited)
    #[serde(default)]
    pub max_normalized_bytes: Option<usize>,
}

/// Controls which metadata fields must be present and how optional blobs are constrained.
///
/// `MetadataPolicy` provides fine-grained control over metadata validation,
/// allowing you to enforce business rules such as required fields, size limits,
/// and timestamp constraints.
///
/// # Examples
///
/// ## Strict Policy
///
/// ```rust
/// use ingest::{MetadataPolicy, RequiredField};
///
/// let strict_policy = MetadataPolicy {
///     required_fields: vec![
///         RequiredField::TenantId,
///         RequiredField::DocId,
///         RequiredField::ReceivedAt,
///         RequiredField::OriginalSource,
///     ],
///     max_attribute_bytes: Some(1024),
///     reject_future_timestamps: true,
/// };
/// ```
///
/// ## Lenient Policy
///
/// ```rust
/// use ingest::MetadataPolicy;
///
/// let lenient_policy = MetadataPolicy::default();
/// // All fields optional, no size limits, future timestamps allowed
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MetadataPolicy {
    /// Metadata fields that must be provided by the caller (after sanitization).
    ///
    /// If a required field is missing or empty after control character stripping,
    /// ingest fails with [`IngestError::InvalidMetadata`].
    ///
    /// # Example
    ///
    /// ```rust
    /// use ingest::{MetadataPolicy, RequiredField};
    ///
    /// let policy = MetadataPolicy {
    ///     required_fields: vec![RequiredField::TenantId, RequiredField::DocId],
    ///     ..Default::default()
    /// };
    /// ```
    ///
    /// Default: empty vector (no required fields)
    pub required_fields: Vec<RequiredField>,

    /// Maximum serialized byte length allowed for `metadata.attributes`.
    ///
    /// If set, the JSON-serialized size of the attributes field must not exceed
    /// this limit. This protects downstream systems from very large metadata blobs.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ingest::MetadataPolicy;
    ///
    /// let policy = MetadataPolicy {
    ///     max_attribute_bytes: Some(1024 * 1024), // 1 MB
    ///     ..Default::default()
    /// };
    /// ```
    ///
    /// Default: `None` (unlimited)
    pub max_attribute_bytes: Option<usize>,

    /// Reject ingests with timestamps that lie in the future.
    ///
    /// When `true`, if `received_at` is strictly greater than the current time,
    /// ingest fails with [`IngestError::InvalidMetadata`] containing "future".
    ///
    /// This is useful for detecting clock skew or preventing future-dated content
    /// from entering the system.
    ///
    /// Default: `false`
    pub reject_future_timestamps: bool,
}

/// Metadata identifiers that can be enforced via [`MetadataPolicy`].
///
/// This enum defines the metadata fields that can be marked as required.
/// It is marked `#[non_exhaustive]` to allow future additions without
/// breaking existing code.
///
/// # Required Fields
///
/// - `TenantId`: Tenant identifier for multi-tenant isolation
/// - `DocId`: Document identifier (caller must provide, no derivation)
/// - `ReceivedAt`: Timestamp when content was received
/// - `OriginalSource`: Human-readable source reference
///
/// # Examples
///
/// ```rust
/// use ingest::{MetadataPolicy, RequiredField};
///
/// let policy = MetadataPolicy {
///     required_fields: vec![
///         RequiredField::TenantId,
///         RequiredField::DocId,
///     ],
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub enum RequiredField {
    /// Require the `tenant_id` field to be present in metadata.
    ///
    /// When required, callers must explicitly provide a non-empty tenant ID.
    /// The `default_tenant_id` fallback is not used.
    TenantId,

    /// Require the `doc_id` field to be present in metadata.
    ///
    /// When required, callers must explicitly provide a document ID.
    /// No UUIDv5 derivation is performed.
    DocId,

    /// Require the `received_at` timestamp to be present in metadata.
    ///
    /// When required, callers must provide a timestamp. The default
    /// (current time) is not applied.
    ReceivedAt,

    /// Require the `original_source` field to be present in metadata.
    ///
    /// When required, callers must provide a source reference.
    OriginalSource,
}

/// Errors that can occur when validating an [`IngestConfig`].
///
/// These are configuration-time issues and are intended to be surfaced during
/// service start-up rather than at request time. They indicate misconfiguration
/// that should be fixed before handling live traffic.
///
/// # Examples
///
/// ```rust
/// use ingest::{IngestConfig, ConfigError};
///
/// let bad_config = IngestConfig {
///     max_payload_bytes: Some(100),
///     max_normalized_bytes: Some(200), // Invalid: exceeds raw limit
///     ..Default::default()
/// };
///
/// match bad_config.validate() {
///     Err(ConfigError::NormalizedExceedsPayload { normalized, payload }) => {
///         println!("Config error: normalized ({}) > payload ({})",
///                  normalized, payload);
///     }
///     Ok(()) => println!("Config is valid"),
/// }
/// ```
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConfigError {
    /// The configured `max_normalized_bytes` is larger than `max_payload_bytes`.
    ///
    /// This violates the expectation that normalized text should always be
    /// bounded by the raw payload size limit and usually indicates a
    /// misconfiguration.
    ///
    /// # Example
    ///
    /// This error occurs when:
    /// ```rust,ignore
    /// max_payload_bytes: Some(100),
    /// max_normalized_bytes: Some(200), // ERROR: exceeds raw limit
    /// ```
    #[error(
        "max_normalized_bytes ({normalized}) exceeds max_payload_bytes ({payload}); \
         normalized payload must not exceed the raw payload limit"
    )]
    NormalizedExceedsPayload {
        /// Configured upper bound for normalized text payloads, in bytes.
        normalized: usize,
        /// Configured upper bound for raw payloads, in bytes.
        payload: usize,
    },
}

impl Default for IngestConfig {
    /// Creates a default `IngestConfig` suitable for development.
    ///
    /// # Defaults
    ///
    /// - `version`: 1
    /// - `default_tenant_id`: "default"
    /// - `doc_id_namespace`: `Uuid::NAMESPACE_OID`
    /// - `strip_control_chars`: true
    /// - `metadata_policy`: default (no required fields, no limits)
    /// - `max_payload_bytes`: None (unlimited)
    /// - `max_normalized_bytes`: None (unlimited)
    ///
    /// # Example
    ///
    /// ```rust
    /// use ingest::IngestConfig;
    ///
    /// let config = IngestConfig::default();
    /// assert_eq!(config.version, 1);
    /// assert_eq!(config.default_tenant_id, "default");
    /// assert!(config.strip_control_chars);
    /// ```
    fn default() -> Self {
        Self {
            version: 1,
            default_tenant_id: "default".into(),
            doc_id_namespace: Uuid::NAMESPACE_OID,
            strip_control_chars: true,
            metadata_policy: MetadataPolicy::default(),
            max_payload_bytes: None,
            max_normalized_bytes: None,
        }
    }
}

impl IngestConfig {
    /// Validates internal consistency of this configuration.
    ///
    /// This method checks for logical errors in the configuration that would
    /// cause runtime issues. It is inexpensive and should be called at process
    /// start-up to catch misconfigurations before handling live ingest traffic.
    ///
    /// # Validation Rules
    ///
    /// 1. `max_normalized_bytes` must be ≤ `max_payload_bytes` (if both are set)
    ///
    /// # Returns
    ///
    /// - `Ok(())` if configuration is valid
    /// - `Err(ConfigError)` describing the validation failure
    ///
    /// # Performance
    ///
    /// This method performs only in-memory checks with O(1) complexity.
    /// No I/O is performed.
    ///
    /// # Examples
    ///
    /// ## Valid Configuration
    ///
    /// ```rust
    /// use ingest::IngestConfig;
    ///
    /// let config = IngestConfig::default();
    /// assert!(config.validate().is_ok());
    /// ```
    ///
    /// ## Invalid Configuration
    ///
    /// ```rust
    /// use ingest::IngestConfig;
    ///
    /// let invalid_config = IngestConfig {
    ///     max_payload_bytes: Some(100),
    ///     max_normalized_bytes: Some(200), // Invalid!
    ///     ..Default::default()
    /// };
    ///
    /// assert!(invalid_config.validate().is_err());
    /// ```
    ///
    /// ## Production Usage
    ///
    /// ```rust
    /// use ingest::IngestConfig;
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = load_config()?;
    ///     config.validate()?;
    ///     // Continue with valid config...
    ///     Ok(())
    /// }
    ///
    /// fn load_config() -> anyhow::Result<IngestConfig> {
    ///     // Load from file, env vars, etc.
    ///     Ok(IngestConfig::default())
    /// }
    /// ```
    pub fn validate(&self) -> Result<(), ConfigError> {
        if let (Some(normalized), Some(payload)) =
            (self.max_normalized_bytes, self.max_payload_bytes)
        {
            if normalized > payload {
                return Err(ConfigError::NormalizedExceedsPayload {
                    normalized,
                    payload,
                });
            }
        }

        Ok(())
    }
}
