use index::IndexError;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use thiserror::Error;

use std::collections::HashMap;

/// Coarse match mode for metrics and backwards compatibility.
///
/// The richer, declarative strategy is expressed via [`MatchExpr`]; `MatchMode`
/// is retained primarily for high-level observability and simple configs.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum MatchMode {
    /// Semantic-only matching using cosine similarity over embeddings.
    #[default]
    Semantic,
    /// Perceptual-only matching using Jaccard similarity over MinHash.
    Perceptual,
    /// Combine semantic and perceptual scores using a weighted policy.
    Hybrid,
}

/// Identifier for the similarity metric used by a particular signal.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MetricId {
    Cosine,
    Jaccard,
    Hamming,
}

/// Declarative matching strategy.
///
/// This allows callers to describe how exact, perceptual, and semantic signals
/// are combined without hard-coding the logic into the engine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MatchExpr {
    /// Exact identity match on canonical hash.
    Exact,
    /// Pure semantic similarity with an explicit metric + threshold.
    Semantic { metric: MetricId, min_score: f32 },
    /// Pure perceptual similarity with an explicit metric + threshold.
    Perceptual { metric: MetricId, min_score: f32 },
    /// Weighted combination of semantic and perceptual scores.
    Weighted {
        /// Weight to assign to the semantic score in [0.0, 1.0].
        semantic_weight: f32,
        /// Minimum final score required for the candidate to be accepted.
        min_overall: f32,
    },
    /// Logical conjunction of two sub-strategies.
    And {
        left: Box<MatchExpr>,
        right: Box<MatchExpr>,
    },
    /// Logical disjunction of two sub-strategies.
    Or {
        left: Box<MatchExpr>,
        right: Box<MatchExpr>,
    },
}

impl MatchExpr {
    /// Default strategy: semantic cosine similarity with no minimum.
    pub fn default_semantic() -> Self {
        MatchExpr::Semantic {
            metric: MetricId::Cosine,
            min_score: 0.0,
        }
    }
}

/// Configuration for a single match request.
///
/// `MatchConfig` is designed to be cheap to clone and serde-friendly so it can
/// be passed across process boundaries or embedded in higher-level configs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MatchConfig {
    /// Configuration schema version for this match config.
    pub version: String,
    /// Logical policy identifier used for audits and replay.
    pub policy_id: String,
    /// Policy version identifier.
    pub policy_version: String,
    /// High-level mode used for observability; strategy is defined by `strategy`.
    #[serde(default)]
    pub mode: MatchMode,
    /// Declarative strategy for combining exact, semantic, and perceptual
    /// signals into a final match decision.
    #[serde(default = "MatchExpr::default_semantic")]
    pub strategy: MatchExpr,
    /// Maximum number of results to return to the caller.
    #[serde(default = "MatchConfig::default_max_results")]
    pub max_results: usize,
    /// Whether to enforce strict tenant isolation at the match layer.
    #[serde(default = "MatchConfig::default_tenant_enforce")]
    pub tenant_enforce: bool,
    /// Oversampling factor when querying the index. Internal `top_k` will be
    /// `ceil(max_results as f32 * oversample_factor)`.
    #[serde(default = "MatchConfig::default_oversample_factor")]
    pub oversample_factor: f32,
    /// Whether to populate per-signal scores and explanation data in the hits.
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

    /// Convenience constructor for a basic semantic-only policy.
    pub fn semantic_default(policy_id: &str, policy_version: &str) -> Self {
        Self {
            version: "v1".to_string(),
            policy_id: policy_id.to_string(),
            policy_version: policy_version.to_string(),
            mode: MatchMode::Semantic,
            strategy: MatchExpr::default_semantic(),
            max_results: Self::default_max_results(),
            tenant_enforce: Self::default_tenant_enforce(),
            oversample_factor: Self::default_oversample_factor(),
            explain: false,
        }
    }

    /// Validate the configuration for a single request.
    pub fn validate(&self) -> Result<(), MatchError> {
        if self.version.trim().is_empty() {
            return Err(MatchError::InvalidConfig(
                "config.version must not be empty".into(),
            ));
        }
        if self.policy_id.trim().is_empty() {
            return Err(MatchError::InvalidConfig(
                "policy_id must not be empty".into(),
            ));
        }
        if self.policy_version.trim().is_empty() {
            return Err(MatchError::InvalidConfig(
                "policy_version must not be empty".into(),
            ));
        }
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

        // Validate strategy-specific invariants.
        fn validate_expr(expr: &MatchExpr) -> Result<(), MatchError> {
            match expr {
                MatchExpr::Exact => Ok(()),
                MatchExpr::Semantic { min_score, .. } | MatchExpr::Perceptual { min_score, .. } => {
                    if *min_score < 0.0 {
                        return Err(MatchError::InvalidConfig("min_score must be >= 0.0".into()));
                    }
                    Ok(())
                }
                MatchExpr::Weighted {
                    semantic_weight,
                    min_overall,
                } => {
                    if !(*semantic_weight >= 0.0 && *semantic_weight <= 1.0) {
                        return Err(MatchError::InvalidConfig(
                            "semantic_weight must be between 0.0 and 1.0".into(),
                        ));
                    }
                    if *min_overall < 0.0 {
                        return Err(MatchError::InvalidConfig(
                            "min_overall must be >= 0.0".into(),
                        ));
                    }
                    Ok(())
                }
                MatchExpr::And { left, right } | MatchExpr::Or { left, right } => {
                    validate_expr(left)?;
                    validate_expr(right)
                }
            }
        }

        validate_expr(&self.strategy)
    }
}

impl Default for MatchConfig {
    fn default() -> Self {
        MatchConfig::semantic_default("default-policy", "v1")
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
            MatchError::InvalidConfig(msg) => {
                assert!(msg.contains("oversample_factor"))
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn invalid_weighted_strategy_rejected() {
        let cfg = MatchConfig {
            strategy: MatchExpr::Weighted {
                semantic_weight: 1.5,
                min_overall: 0.0,
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
    /// Free-text query text (for logging or legacy paths). Prefer precomputed
    /// signals and canonical hashes for production use.
    pub query_text: String,
    /// Per-request configuration; if omitted in higher layers, use
    /// `MatchConfig::default()`.
    pub config: MatchConfig,
    /// Optional opaque attributes; can be used for logging or per-tenant overrides.
    #[serde(default)]
    pub attributes: Option<JsonValue>,
    /// Optional pipeline version that produced the query/document signals.
    #[serde(default)]
    pub pipeline_version: Option<String>,
    /// Optional fingerprint / embedding version map.
    #[serde(default)]
    pub fingerprint_versions: Option<HashMap<String, String>>,
    /// Optional canonical hash of the query for exact-match strategies.
    #[serde(default)]
    pub query_canonical_hash: Option<String>,
}

/// Additional explanation data for a single hit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MatchExplanation {
    /// Optional semantic distance (e.g., 1.0 - cosine_similarity).
    pub semantic_distance: Option<f32>,
    /// Optional perceptual overlap (e.g., Jaccard index).
    pub perceptual_overlap: Option<f32>,
    /// Optional token overlap or similar textual signal.
    pub token_overlap: Option<f32>,
}

/// A single hit returned by the matcher.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MatchHit {
    /// Canonical hash of the matched document (primary identifier in the index).
    pub canonical_hash: String,
    /// Final score after applying the configured strategy.
    pub score: f32,
    /// Underlying semantic score when available.
    pub semantic_score: Option<f32>,
    /// Underlying perceptual score when available.
    pub perceptual_score: Option<f32>,
    /// Exact-match score when available (typically 1.0 or 0.0).
    pub exact_score: Option<f32>,
    /// Stored metadata blob from the index record.
    pub metadata: JsonValue,
    /// Match engine version that produced this decision.
    pub match_version: String,
    /// Policy identifier and version used for this decision.
    pub policy_id: String,
    pub policy_version: String,
    /// Optional human/machine-readable explanation artifacts.
    pub explanation: Option<MatchExplanation>,
}

/// Errors produced by the matching layer.
#[derive(Debug, Error)]
pub enum MatchError {
    /// Invalid configuration (per-request or global).
    #[error("invalid match config: {0}")]
    InvalidConfig(String),
    /// Ingest stage failed.
    #[error("ingest error: {0}")]
    Ingest(String),
    /// Canonical stage failed.
    #[error("canonical error: {0}")]
    Canonical(String),
    /// Perceptual stage failed.
    #[error("perceptual error: {0}")]
    Perceptual(String),
    /// Semantic stage failed.
    #[error("semantic error: {0}")]
    Semantic(String),
    /// Pipeline stage failed.
    #[error("pipeline error: {0}")]
    Pipeline(String),
    /// Index read or search failed.
    #[error("index error: {0}")]
    Index(#[from] IndexError),
}
