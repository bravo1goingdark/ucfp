use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::circuit_breaker::CircuitBreakerConfig;
use crate::rate_limit::RateLimitConfig;
use crate::retry::RetryConfig;

/// Runtime configuration describing which model/tokenizer to use and how to post-process vectors.
///
/// # Example
/// ```no_run
/// use semantic::{semanticize, SemanticConfig};
///
/// let cfg = SemanticConfig {
///     mode: "api".into(),
///     api_url: Some("https://api-inference.huggingface.co/models/BAAI/bge-small-en-v1.5".into()),
///     api_auth_header: Some("Bearer hf_xxx".into()),
///     api_provider: Some("hf".into()),
///     normalize: true,
///     ..Default::default()
/// };
///
/// let _ = semanticize("doc123", "This is a test.", &cfg);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SemanticConfig {
    /// Model tier: `"fast"` forces the deterministic stub, `"balanced"` and `"accurate"`
    /// attempt to run ONNX inference.
    pub tier: String,
    /// Inference mode selector: `"onnx"` (local), `"api"` (remote HTTP), or `"fast"` (stub).
    pub mode: String,
    /// Friendly label surfaced on every `SemanticEmbedding`.
    pub model_name: String,
    /// Local path where the ONNX file should live (also used as the download target when
    /// [`model_url`](Self::model_url) is provided).
    pub model_path: PathBuf,
    /// Optional HTTPS/S3 URL that will be downloaded when [`model_path`](Self::model_path) is missing.
    pub model_url: Option<String>,
    /// API inference endpoint when [`mode`](Self::mode) is `"api"`.
    pub api_url: Option<String>,
    /// Authorization header (e.g., `"Bearer hf_xxx"`).
    pub api_auth_header: Option<String>,
    /// Remote provider hint: `"hf"`, `"openai"`, or `"custom"` (default).
    pub api_provider: Option<String>,
    /// Overall API timeout in seconds.
    pub api_timeout_secs: Option<u64>,
    /// Path to `tokenizer.json`. When absent and [`tokenizer_url`](Self::tokenizer_url) is provided we
    /// infer the filename from the URL and place it next to the model file.
    pub tokenizer_path: Option<PathBuf>,
    /// Optional HTTPS/S3 URL for fetching the tokenizer on-demand.
    pub tokenizer_url: Option<String>,
    /// Normalize the resulting vector to unit-length (recommended for cosine similarity).
    pub normalize: bool,
    /// Compute device (currently only `"cpu"` is implemented, but the field keeps the config forward-compatible).
    pub device: String,
    /// Retry configuration for API calls.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_config: Option<RetryConfig>,
    /// Circuit breaker configuration for API resilience.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub circuit_breaker_config: Option<CircuitBreakerConfig>,
    /// Rate limiting configuration for API providers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_config: Option<RateLimitConfig>,
    /// Whether to enable resilience features (retry, circuit breaker, rate limiting).
    /// Defaults to true for production safety.
    pub enable_resilience: bool,
}

impl Default for SemanticConfig {
    fn default() -> Self {
        Self {
            tier: "balanced".into(),
            mode: "onnx".into(),
            model_name: "bge-small-en-v1.5".into(),
            model_path: PathBuf::from("./models/bge-small-en-v1.5/onnx/model.onnx"),
            model_url: None,
            api_url: None,
            api_auth_header: None,
            api_provider: None,
            api_timeout_secs: Some(30),
            tokenizer_path: Some(PathBuf::from("./models/bge-small-en-v1.5/tokenizer.json")),
            tokenizer_url: None,
            normalize: true,
            device: "cpu".into(),
            retry_config: None,           // Uses defaults when None
            circuit_breaker_config: None, // Uses defaults when None
            rate_limit_config: None,      // Uses defaults when None
            enable_resilience: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_default_values() {
        let cfg = SemanticConfig::default();
        assert_eq!(cfg.tier, "balanced");
        assert_eq!(cfg.mode, "onnx");
        assert_eq!(cfg.model_name, "bge-small-en-v1.5");
        assert_eq!(
            cfg.model_path,
            PathBuf::from("./models/bge-small-en-v1.5/onnx/model.onnx")
        );
        assert!(cfg.model_url.is_none());
        assert!(cfg.api_url.is_none());
        assert!(cfg.api_auth_header.is_none());
        assert!(cfg.api_provider.is_none());
        assert_eq!(cfg.api_timeout_secs, Some(30));
        assert_eq!(
            cfg.tokenizer_path,
            Some(PathBuf::from("./models/bge-small-en-v1.5/tokenizer.json"))
        );
        assert!(cfg.tokenizer_url.is_none());
        assert!(cfg.normalize);
        assert_eq!(cfg.device, "cpu");
    }

    #[test]
    fn config_custom_values() {
        let cfg = SemanticConfig {
            tier: "fast".into(),
            mode: "api".into(),
            model_name: "custom-model".into(),
            model_path: PathBuf::from("/custom/path/model.onnx"),
            normalize: false,
            ..Default::default()
        };

        assert_eq!(cfg.tier, "fast");
        assert_eq!(cfg.mode, "api");
        assert_eq!(cfg.model_name, "custom-model");
        assert_eq!(cfg.model_path, PathBuf::from("/custom/path/model.onnx"));
        assert!(!cfg.normalize);
    }

    #[test]
    fn config_serde_roundtrip() {
        let cfg = SemanticConfig {
            tier: "accurate".into(),
            mode: "api".into(),
            model_name: "test-model".into(),
            model_path: PathBuf::from("/test/model.onnx"),
            model_url: Some("https://example.com/model.onnx".into()),
            api_url: Some("https://api.example.com/embed".into()),
            api_auth_header: Some("Bearer token123".into()),
            api_provider: Some("openai".into()),
            api_timeout_secs: Some(60),
            tokenizer_path: Some(PathBuf::from("/test/tokenizer.json")),
            tokenizer_url: Some("https://example.com/tokenizer.json".into()),
            normalize: false,
            device: "cuda".into(),
            retry_config: None,
            circuit_breaker_config: None,
            rate_limit_config: None,
            enable_resilience: true,
        };

        let serialized = serde_json::to_string(&cfg).unwrap();
        let deserialized: SemanticConfig = serde_json::from_str(&serialized).unwrap();

        assert_eq!(cfg, deserialized);
    }

    #[test]
    fn config_clone() {
        let cfg = SemanticConfig::default();
        let cloned = cfg.clone();
        assert_eq!(cfg, cloned);
    }

    #[test]
    fn config_partial_eq() {
        let cfg1 = SemanticConfig::default();
        let cfg2 = SemanticConfig::default();
        assert_eq!(cfg1, cfg2);

        let cfg3 = SemanticConfig {
            tier: "fast".into(),
            ..Default::default()
        };
        assert_ne!(cfg1, cfg3);
    }

    #[test]
    fn config_all_modes() {
        let modes = vec!["onnx", "api", "fast"];
        for mode in modes {
            let cfg = SemanticConfig {
                mode: mode.into(),
                ..Default::default()
            };
            assert_eq!(cfg.mode, mode);
        }
    }

    #[test]
    fn config_all_tiers() {
        let tiers = vec!["fast", "balanced", "accurate"];
        for tier in tiers {
            let cfg = SemanticConfig {
                tier: tier.into(),
                ..Default::default()
            };
            assert_eq!(cfg.tier, tier);
        }
    }
}
