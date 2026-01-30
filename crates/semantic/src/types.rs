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
