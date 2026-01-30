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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_model_not_found() {
        let err = SemanticError::ModelNotFound("/path/to/model.onnx".into());
        assert!(err.to_string().contains("model file not found"));
        assert!(err.to_string().contains("/path/to/model.onnx"));
    }

    #[test]
    fn error_tokenizer_missing() {
        let err = SemanticError::TokenizerMissing("/path/to/tokenizer.json".into());
        assert!(err.to_string().contains("tokenizer missing"));
        assert!(err.to_string().contains("/path/to/tokenizer.json"));
    }

    #[test]
    fn error_invalid_config() {
        let err = SemanticError::InvalidConfig("missing model path".into());
        assert!(err.to_string().contains("invalid semantic config"));
        assert!(err.to_string().contains("missing model path"));
    }

    #[test]
    fn error_download() {
        let err = SemanticError::Download("network timeout".into());
        assert!(err.to_string().contains("download failed"));
        assert!(err.to_string().contains("network timeout"));
    }

    #[test]
    fn error_inference() {
        let err = SemanticError::Inference("ONNX session failed".into());
        assert!(err.to_string().contains("inference failure"));
        assert!(err.to_string().contains("ONNX session failed"));
    }

    #[test]
    fn error_from_io() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let err: SemanticError = io_err.into();
        assert!(err.to_string().contains("io error"));
    }

    #[test]
    fn error_clone() {
        let err1 = SemanticError::ModelNotFound("path".into());
        let cloned = err1.clone();
        assert_eq!(format!("{err1}"), format!("{}", cloned));

        let err2 = SemanticError::Inference("error".into());
        let cloned2 = err2.clone();
        assert_eq!(format!("{err2}"), format!("{}", cloned2));
    }

    #[test]
    fn error_clone_io_converts_to_inference() {
        let io_err = io::Error::other("test");
        let err: SemanticError = io_err.into();
        let cloned = err.clone();
        // IO errors get converted to Inference on clone
        assert!(cloned.to_string().contains("IO error occurred"));
    }

    #[test]
    fn error_debug_formatting() {
        let err = SemanticError::ModelNotFound("test.onnx".into());
        let debug_str = format!("{err:?}");
        assert!(debug_str.contains("ModelNotFound"));
        assert!(debug_str.contains("test.onnx"));
    }

    #[test]
    fn error_all_variants_cloneable() {
        let variants = vec![
            SemanticError::ModelNotFound("a".into()),
            SemanticError::TokenizerMissing("b".into()),
            SemanticError::InvalidConfig("c".into()),
            SemanticError::Download("d".into()),
            SemanticError::Inference("e".into()),
        ];

        for err in variants {
            let _cloned = err.clone();
        }
    }
}
