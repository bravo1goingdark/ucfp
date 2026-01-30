use fxhash::hash64;

use crate::normalize::l2_normalize_in_place;
use crate::{SemanticConfig, SemanticEmbedding};

/// Deterministic stub used when tier is `"fast"` or the model assets are unavailable.
/// Generates sinusoid values derived from a hash of the input text to guarantee reproducible
/// vectors with minimal CPU cost.
pub(crate) fn make_stub_embedding(
    doc_id: &str,
    text: &str,
    cfg: &SemanticConfig,
) -> SemanticEmbedding {
    let dim = match cfg.tier.as_str() {
        "fast" => 384,
        "accurate" => 1024,
        _ => 768,
    };
    let mut v = vec![0f32; dim];
    let h = hash64(text.as_bytes());
    for (idx, value) in v.iter_mut().enumerate() {
        *value = ((h >> (idx % 32)) as f32 * 0.0001).sin();
    }
    if cfg.normalize {
        l2_normalize_in_place(&mut v);
    }
    SemanticEmbedding {
        doc_id: doc_id.to_string(),
        vector: v,
        model_name: cfg.model_name.clone(),
        tier: cfg.tier.clone(),
        embedding_dim: dim,
        normalized: cfg.normalize,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_stub_embedding_fast_tier() {
        let cfg = SemanticConfig {
            tier: "fast".into(),
            normalize: false,
            ..Default::default()
        };

        let embedding = make_stub_embedding("doc-1", "hello world", &cfg);

        assert_eq!(embedding.doc_id, "doc-1");
        assert_eq!(embedding.embedding_dim, 384);
        assert_eq!(embedding.vector.len(), 384);
        assert_eq!(embedding.tier, "fast");
        assert!(!embedding.normalized);
    }

    #[test]
    fn make_stub_embedding_balanced_tier() {
        let cfg = SemanticConfig {
            tier: "balanced".into(),
            normalize: false,
            ..Default::default()
        };

        let embedding = make_stub_embedding("doc-2", "test text", &cfg);

        assert_eq!(embedding.embedding_dim, 768);
        assert_eq!(embedding.vector.len(), 768);
        assert_eq!(embedding.tier, "balanced");
    }

    #[test]
    fn make_stub_embedding_accurate_tier() {
        let cfg = SemanticConfig {
            tier: "accurate".into(),
            normalize: false,
            ..Default::default()
        };

        let embedding = make_stub_embedding("doc-3", "another test", &cfg);

        assert_eq!(embedding.embedding_dim, 1024);
        assert_eq!(embedding.vector.len(), 1024);
        assert_eq!(embedding.tier, "accurate");
    }

    #[test]
    fn make_stub_embedding_deterministic() {
        let cfg = SemanticConfig {
            tier: "fast".into(),
            normalize: false,
            ..Default::default()
        };

        let e1 = make_stub_embedding("doc-1", "same text", &cfg);
        let e2 = make_stub_embedding("doc-2", "same text", &cfg);

        // Same text should produce same vector (doc_id doesn't matter for stub)
        assert_eq!(e1.vector, e2.vector);
    }

    #[test]
    fn make_stub_embedding_different_text() {
        let cfg = SemanticConfig {
            tier: "fast".into(),
            normalize: false,
            ..Default::default()
        };

        let e1 = make_stub_embedding("doc-1", "hello", &cfg);
        let e2 = make_stub_embedding("doc-2", "world", &cfg);

        // Different text should produce different vectors
        assert_ne!(e1.vector, e2.vector);
    }

    #[test]
    fn make_stub_embedding_normalized() {
        let cfg = SemanticConfig {
            tier: "fast".into(),
            normalize: true,
            ..Default::default()
        };

        let embedding = make_stub_embedding("doc-1", "test", &cfg);

        assert!(embedding.normalized);
        // Check that it's actually normalized
        let norm: f32 = embedding.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 1e-4,
            "Vector should be normalized, got norm={norm}"
        );
    }

    #[test]
    fn make_stub_embedding_empty_text() {
        let cfg = SemanticConfig {
            tier: "fast".into(),
            normalize: false,
            ..Default::default()
        };

        let embedding = make_stub_embedding("doc-1", "", &cfg);
        assert_eq!(embedding.vector.len(), 384);
        // Empty string should still produce a valid vector with expected dimensions
        // Note: The hash of an empty string produces non-zero values
        assert!(!embedding.vector.is_empty());
    }

    #[test]
    fn make_stub_embedding_values_in_range() {
        let cfg = SemanticConfig {
            tier: "balanced".into(),
            normalize: false,
            ..Default::default()
        };

        let embedding = make_stub_embedding("doc-1", "test", &cfg);

        // All values should be in range [-1, 1] (since they're sin values)
        for (i, &val) in embedding.vector.iter().enumerate() {
            assert!(
                (-1.0..=1.0).contains(&val),
                "Value at index {i} is {val} which is outside [-1, 1]"
            );
        }
    }

    #[test]
    fn make_stub_embedding_unicode() {
        let cfg = SemanticConfig {
            tier: "fast".into(),
            normalize: false,
            ..Default::default()
        };

        let embedding = make_stub_embedding("doc-1", "Hello 世界 🌍", &cfg);

        assert_eq!(embedding.vector.len(), 384);
        // Unicode text should work fine
        assert!(!embedding.vector.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn make_stub_embedding_long_text() {
        let cfg = SemanticConfig {
            tier: "fast".into(),
            normalize: false,
            ..Default::default()
        };

        let long_text = "a".repeat(10000);
        let embedding = make_stub_embedding("doc-1", &long_text, &cfg);

        assert_eq!(embedding.vector.len(), 384);
        // Long text should still produce a valid vector
        assert!(!embedding.vector.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn make_stub_embedding_special_characters() {
        let cfg = SemanticConfig {
            tier: "fast".into(),
            normalize: false,
            ..Default::default()
        };

        let embedding = make_stub_embedding("doc-1", "!@#$%^&*()_+-=[]{}|;':\",./<>?", &cfg);

        assert_eq!(embedding.vector.len(), 384);
        assert!(!embedding.vector.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn make_stub_embedding_preserves_model_name() {
        let cfg = SemanticConfig {
            tier: "fast".into(),
            model_name: "custom-model".into(),
            ..Default::default()
        };

        let embedding = make_stub_embedding("doc-1", "test", &cfg);

        assert_eq!(embedding.model_name, "custom-model");
    }

    #[test]
    fn make_stub_embedding_unknown_tier_defaults_to_balanced() {
        let cfg = SemanticConfig {
            tier: "unknown".into(),
            normalize: false,
            ..Default::default()
        };

        let embedding = make_stub_embedding("doc-1", "test", &cfg);

        // Unknown tier should default to 768 (balanced)
        assert_eq!(embedding.embedding_dim, 768);
    }
}
