use std::io;
use thiserror::Error;

/// Errors surfaced by the `semanticize` function.
#[derive(Debug, Error)]
pub enum SemanticError {
    /// The ONNX model could not be located locally and no fallback URL was provided.
    #[error("model file not found: {0}")]
    ModelNotFound(String),
    /// The tokenizer JSON is missing and there was no remote URL to fetch it from.
    #[error("tokenizer missing: {0}")]
    TokenizerMissing(String),
    /// Configuration is inconsistent (e.g., both tokenizer path and URL are missing).
    #[error("invalid semantic config: {0}")]
    InvalidConfig(String),
    /// Unable to download remote assets.
    #[error("download failed: {0}")]
    Download(String),
    /// Low-level IO failures while touching the filesystem.
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    /// ONNX Runtime, tokenizer, or normalization errors.
    #[error("inference failure: {0}")]
    Inference(String),
}

impl Clone for SemanticError {
    fn clone(&self) -> Self {
        match self {
            SemanticError::ModelNotFound(s) => SemanticError::ModelNotFound(s.clone()),
            SemanticError::TokenizerMissing(s) => SemanticError::TokenizerMissing(s.clone()),
            SemanticError::InvalidConfig(s) => SemanticError::InvalidConfig(s.clone()),
            SemanticError::Download(s) => SemanticError::Download(s.clone()),
            SemanticError::Io(_) => SemanticError::Inference("IO error occurred".to_string()),
            SemanticError::Inference(s) => SemanticError::Inference(s.clone()),
        }
    }
}
