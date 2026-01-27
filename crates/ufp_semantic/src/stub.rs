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
