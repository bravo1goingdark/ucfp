//! Error types produced by the `ingest` crate.
//!
//! The primary error surface is [`IngestError`], which is used for all
//! request-time failures during ingest normalization and validation.
use thiserror::Error;

/// Errors that can occur during ingest normalization and validation.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum IngestError {
    #[error("missing payload for source that requires payload")]
    MissingPayload,
    #[error("binary payload is empty")]
    EmptyBinaryPayload,
    #[error("invalid metadata: {0}")]
    InvalidMetadata(String),
    #[error("invalid utf-8 payload: {0}")]
    InvalidUtf8(String),
    #[error("text payload empty after normalization")]
    EmptyNormalizedText,
    #[error("payload exceeds size limit: {0}")]
    PayloadTooLarge(String),
}
