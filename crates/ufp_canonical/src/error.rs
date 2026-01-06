use thiserror::Error;

/// Errors that can occur during canonicalization.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CanonicalError {
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("canonical document requires a non-empty doc_id")]
    MissingDocId,
    #[error("input text empty after normalization")]
    EmptyInput,
}
