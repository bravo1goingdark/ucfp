use serde::{Deserialize, Serialize};

/// Embedding output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SemanticEmbedding {
    /// Identifier of the processed document/passage.
    pub doc_id: String,
    /// Final embedding values (either model output or deterministic stub).
    pub vector: Vec<f32>,
    /// Name of the model used to produce the vector.
    pub model_name: String,
    /// Tier requested during inference (surfaced for observability).
    pub tier: String,
    /// Dimension of `vector`.
    pub embedding_dim: usize,
    /// Whether [`vector`](Self::vector) was L2-normalized.
    pub normalized: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_embedding_creation() {
        let embedding = SemanticEmbedding {
            doc_id: "doc-1".into(),
            vector: vec![0.1, 0.2, 0.3, 0.4, 0.5],
            model_name: "test-model".into(),
            tier: "balanced".into(),
            embedding_dim: 5,
            normalized: true,
        };

        assert_eq!(embedding.doc_id, "doc-1");
        assert_eq!(embedding.vector, vec![0.1, 0.2, 0.3, 0.4, 0.5]);
        assert_eq!(embedding.model_name, "test-model");
        assert_eq!(embedding.tier, "balanced");
        assert_eq!(embedding.embedding_dim, 5);
        assert!(embedding.normalized);
    }

    #[test]
    fn semantic_embedding_clone() {
        let embedding = SemanticEmbedding {
            doc_id: "doc-1".into(),
            vector: vec![0.1, 0.2, 0.3],
            model_name: "test".into(),
            tier: "fast".into(),
            embedding_dim: 3,
            normalized: false,
        };

        let cloned = embedding.clone();
        assert_eq!(embedding, cloned);
    }

    #[test]
    fn semantic_embedding_serde_roundtrip() {
        let embedding = SemanticEmbedding {
            doc_id: "doc-123".into(),
            vector: vec![0.1, 0.2, 0.3, 0.4],
            model_name: "bge-small".into(),
            tier: "balanced".into(),
            embedding_dim: 4,
            normalized: true,
        };

        let serialized = serde_json::to_string(&embedding).unwrap();
        let deserialized: SemanticEmbedding = serde_json::from_str(&serialized).unwrap();

        assert_eq!(embedding, deserialized);
    }

    #[test]
    fn semantic_embedding_empty_vector() {
        let embedding = SemanticEmbedding {
            doc_id: "empty".into(),
            vector: vec![],
            model_name: "test".into(),
            tier: "fast".into(),
            embedding_dim: 0,
            normalized: false,
        };

        assert!(embedding.vector.is_empty());
        assert_eq!(embedding.embedding_dim, 0);
    }

    #[test]
    fn semantic_embedding_large_vector() {
        let vector: Vec<f32> = (0..1024).map(|i| i as f32 / 1024.0).collect();
        let embedding = SemanticEmbedding {
            doc_id: "large".into(),
            vector: vector.clone(),
            model_name: "large-model".into(),
            tier: "accurate".into(),
            embedding_dim: 1024,
            normalized: true,
        };

        assert_eq!(embedding.vector.len(), 1024);
        assert_eq!(embedding.embedding_dim, 1024);
    }

    #[test]
    fn semantic_embedding_partial_eq() {
        let e1 = SemanticEmbedding {
            doc_id: "doc-1".into(),
            vector: vec![0.1, 0.2, 0.3],
            model_name: "test".into(),
            tier: "fast".into(),
            embedding_dim: 3,
            normalized: false,
        };

        let e2 = SemanticEmbedding {
            doc_id: "doc-1".into(),
            vector: vec![0.1, 0.2, 0.3],
            model_name: "test".into(),
            tier: "fast".into(),
            embedding_dim: 3,
            normalized: false,
        };

        let e3 = SemanticEmbedding {
            doc_id: "doc-2".into(),
            vector: vec![0.4, 0.5, 0.6],
            model_name: "test".into(),
            tier: "fast".into(),
            embedding_dim: 3,
            normalized: false,
        };

        assert_eq!(e1, e2);
        assert_ne!(e1, e3);
    }
}
