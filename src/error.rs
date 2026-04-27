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
}

/// `Result<T, Error>` alias for ergonomic crate-internal use.
pub type Result<T> = std::result::Result<T, Error>;
