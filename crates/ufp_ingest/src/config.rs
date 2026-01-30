//! Configuration types for the `ufp_ingest` pipeline.
//!
//! This module defines [`IngestConfig`] and [`MetadataPolicy`], which control how
//! raw ingest requests are interpreted, defaulted, and constrained at runtime.
//! These types are intended to be cheap to clone and easy to serialize from
//! external configuration formats such as JSON, TOML, or YAML.
use serde::{Deserialize, Serialize};
use thiserror::Error;
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
    /// Maximum raw payload byte length allowed.
    #[serde(default)]
    pub max_payload_bytes: Option<usize>,
    /// Maximum normalized payload byte length allowed.
    #[serde(default)]
    pub max_normalized_bytes: Option<usize>,
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
#[non_exhaustive]
pub enum RequiredField {
    /// Require the tenant_id field to be present in metadata.
    TenantId,
    /// Require the doc_id field to be present in metadata.
    DocId,
    /// Require the received_at timestamp to be present in metadata.
    ReceivedAt,
    /// Require the original_source field to be present in metadata.
    OriginalSource,
}

/// Errors that can occur when validating an [`IngestConfig`].
///
/// These are configuration-time issues and are intended to be surfaced during
/// service start-up rather than at request time.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConfigError {
    /// The configured `max_normalized_bytes` is larger than `max_payload_bytes`.
    ///
    /// This violates the expectation that normalized text should always be
    /// bounded by the raw payload size limit and usually indicates a
    /// misconfiguration.
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
    /// This method is inexpensive and can be called at process start-up to
    /// catch obvious misconfigurations before handling live ingest traffic.
    /// It does **not** perform any I/O and only inspects in-memory values.
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
