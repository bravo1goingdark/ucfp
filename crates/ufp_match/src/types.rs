use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use thiserror::Error;
use ucfp::PipelineError;
use ufp_index::IndexError;

/// Match strategy to use when querying the index.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum MatchMode {
    /// Semantic-only matching using quantized embeddings and cosine similarity.
    #[default]
    Semantic,
    /// Perceptual-only matching using MinHash and Jaccard similarity.
    Perceptual,
    /// Combine semantic and perceptual scores using a weighted average.
    Hybrid {
        /// Weight to assign to the semantic score in [0.0, 1.0].
        semantic_weight: f32,
    },
}

/// Configuration for a single match request.
///
/// `MatchConfig` is designed to be cheap to clone and serde-friendly so it can
/// be passed across process boundaries or embedded in higher-level configs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MatchConfig {
    /// Matching mode (semantic, perceptual, or hybrid).
    #[serde(default)]
    pub mode: MatchMode,
    /// Maximum number of results to return to the caller.
    #[serde(default = "MatchConfig::default_max_results")]
    pub max_results: usize,
    /// Minimum final score required for a hit to be included.
    #[serde(default)]
    pub min_score: f32,
    /// Whether to enforce strict tenant isolation at the match layer.
    #[serde(default = "MatchConfig::default_tenant_enforce")]
    pub tenant_enforce: bool,
    /// Oversampling factor when querying the index. Internal `top_k` will be
    /// `ceil(max_results as f32 * oversample_factor)`.
    #[serde(default = "MatchConfig::default_oversample_factor")]
    pub oversample_factor: f32,
    /// Whether to populate per-mode scores in the match hits.
    #[serde(default)]
    pub explain: bool,
}

impl MatchConfig {
    pub(crate) fn default_max_results() -> usize {
        10
    }

    pub(crate) fn default_tenant_enforce() -> bool {
        true
    }

    pub(crate) fn default_oversample_factor() -> f32 {
        2.0
    }

    /// Validate the configuration for a single request.
    pub fn validate(&self) -> Result<(), MatchError> {
        if self.max_results == 0 {
            return Err(MatchError::InvalidConfig(
                "max_results must be greater than zero".into(),
            ));
        }
        if self.oversample_factor < 1.0 {
            return Err(MatchError::InvalidConfig(
                "oversample_factor must be >= 1.0".into(),
            ));
        }
        if self.min_score < 0.0 {
            return Err(MatchError::InvalidConfig("min_score must be >= 0.0".into()));
        }
        if let MatchMode::Hybrid { semantic_weight } = self.mode
            && !(0.0..=1.0).contains(&semantic_weight)
        {
            return Err(MatchError::InvalidConfig(
                "semantic_weight must be between 0.0 and 1.0".into(),
            ));
        }
        Ok(())
    }
}

impl Default for MatchConfig {
    fn default() -> Self {
        Self {
            mode: MatchMode::default(),
            max_results: Self::default_max_results(),
            min_score: 0.0,
            tenant_enforce: Self::default_tenant_enforce(),
            oversample_factor: Self::default_oversample_factor(),
            explain: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid_and_semantic() {
        let cfg = MatchConfig::default();
        assert!(cfg.validate().is_ok());
        assert!(matches!(cfg.mode, MatchMode::Semantic));
        assert_eq!(cfg.max_results, MatchConfig::default_max_results());
        assert!(cfg.oversample_factor >= 1.0);
    }

    #[test]
    fn invalid_max_results_rejected() {
        let cfg = MatchConfig {
            max_results: 0,
            ..MatchConfig::default()
        };
        let err = cfg.validate().expect_err("config should be invalid");
        match err {
            MatchError::InvalidConfig(msg) => assert!(msg.contains("max_results")),
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn invalid_oversample_factor_rejected() {
        let cfg = MatchConfig {
            oversample_factor: 0.5,
            ..MatchConfig::default()
        };
        let err = cfg.validate().expect_err("config should be invalid");
        match err {
            MatchError::InvalidConfig(msg) => assert!(msg.contains("oversample_factor")),
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn invalid_semantic_weight_rejected() {
        let cfg = MatchConfig {
            mode: MatchMode::Hybrid {
                semantic_weight: 1.5,
            },
            ..MatchConfig::default()
        };
        let err = cfg.validate().expect_err("config should be invalid");
        match err {
            MatchError::InvalidConfig(msg) => assert!(msg.contains("semantic_weight")),
            other => panic!("unexpected error: {other}"),
        }
    }
}

/// A single match request against the index.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MatchRequest {
    /// Tenant identifier; required for SaaS-style multi-tenant isolation.
    pub tenant_id: String,
    /// Free-text query to be canonicalized and embedded/fingerprinted.
    pub query_text: String,
    /// Per-request configuration; if omitted in higher layers, use `MatchConfig::default()`.
    pub config: MatchConfig,
    /// Optional opaque attributes; can be used for logging or per-tenant overrides.
    #[serde(default)]
    pub attributes: Option<JsonValue>,
}

/// A single hit returned by the matcher.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MatchHit {
    /// Canonical hash of the matched document (primary identifier in the index).
    pub canonical_hash: String,
    /// Final score after applying the configured match mode.
    pub score: f32,
    /// Underlying semantic score when available.
    pub semantic_score: Option<f32>,
    /// Underlying perceptual score when available.
    pub perceptual_score: Option<f32>,
    /// Stored metadata blob from the index record.
    pub metadata: JsonValue,
}

/// Errors produced by the matching layer.
#[derive(Debug, Error)]
pub enum MatchError {
    /// Invalid configuration (per-request or global).
    #[error("invalid match config: {0}")]
    InvalidConfig(String),
    /// Ingest/canonical/perceptual/semantic pipeline failed.
    #[error("pipeline error: {0}")]
    Pipeline(#[from] PipelineError),
    /// Index read or search failed.
    #[error("index error: {0}")]
    Index(#[from] IndexError),
}
