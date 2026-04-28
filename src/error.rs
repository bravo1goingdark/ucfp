//! Crate-wide error type and [`Result`] alias.

use thiserror::Error;

/// All errors UCFP can produce. `#[non_exhaustive]` so adding variants
/// is a non-breaking change for downstream `match` users.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// Per-modality SDK rejected the input (decode failure, oversize, etc.).
    #[error("modality error: {0}")]
    Modality(String),

    /// Index backend reported a storage-level failure.
    #[error("index backend error: {0}")]
    Index(String),

    /// Ingest source reported a transport-level failure.
    #[error("ingest source error: {0}")]
    Ingest(String),

    /// Reranker reported a model-level failure.
    #[error("reranker error: {0}")]
    Rerank(String),

    /// Cross-version or cross-config compare was attempted.
    #[error("incompatible record: {0}")]
    Incompatible(String),

    /// I/O failure (file read, network, etc.).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// No record exists for `(tenant_id, record_id)`. Maps to HTTP 404
    /// at the server boundary; surfaced by `IndexBackend::get_record_metadata`
    /// and friends when the lookup misses.
    #[error("record not found: tenant {tenant_id}, record {record_id}")]
    RecordNotFound {
        /// Tenant the missing record was scoped to.
        tenant_id: u32,
        /// Identifier of the missing record within the tenant.
        record_id: u64,
    },

    /// Operation is not supported by this build / backend / algorithm.
    /// Maps to HTTP 501 at the server boundary; used by the default
    /// `IndexBackend::get_record_metadata` impl and by handler dispatch
    /// arms whose feature flag is disabled.
    #[error("operation not supported: {0}")]
    Unsupported(String),

    /// Caller is authenticated but not allowed to access the requested
    /// tenant namespace. Maps to HTTP 403 at the server boundary.
    #[error("forbidden: key tenant {key_tenant} cannot access tenant {path_tenant}")]
    Forbidden {
        /// Tenant ID bound to the API key used for this request.
        key_tenant: u32,
        /// Tenant ID extracted from the request path or body.
        path_tenant: u32,
    },
}

/// `Result<T, Error>` alias for ergonomic crate-internal use.
pub type Result<T> = std::result::Result<T, Error>;
