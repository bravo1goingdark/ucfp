//! # UCFP Semantic Fingerprinting
//!
//! This crate provides meaning-aware fingerprinting by converting canonicalized
//! text into dense vector embeddings. It is designed for flexibility and
//! resilience, supporting multiple inference modes and offering deterministic
//! fallbacks.
//!
//! ## Core Features
//!
//! - **Multiple Inference Modes**:
//!   - **ONNX**: Local inference using ONNX Runtime for full offline capability.
//!   - **API**: Remote inference via HTTP, with support for Hugging Face, OpenAI,
//!     and custom API endpoints.
//!   - **Fast**: A deterministic stub generator for testing and development,
//!     which produces reproducible vectors without requiring model assets.
//! - **Resilience**: Automatically falls back to the "fast" mode if model assets
//!   are missing or unreachable, ensuring the pipeline continues to operate.
//! - **Performance**: Caches tokenizers and ONNX sessions on a per-thread basis
//!   to minimize I/O and compilation overhead in hot paths. Batch processing
//!   is supported to efficiently handle multiple documents.
//! - **Configurability**: All behavior is controlled at runtime via the
//!   [`SemanticConfig`] struct, allowing for different models, tiers (fast,
//!   balanced, accurate), and post-processing options (e.g., L2 normalization).
//!
//! ## Key Concepts
//!
//! The main entry point is the [`semanticize`] function, which orchestrates the
//! entire process: asset resolution (including on-demand downloading),
//! tokenization, inference, and normalization. The resulting [`SemanticEmbedding`]
//! contains the vector and rich metadata for downstream use.
//!
//! The implementation uses a thread-local cache for model handles, ensuring that
//! expensive setup costs are paid only once per thread. This makes it suitable
//! for high-throughput services.
//!
//! ## Example Usage
//!
//! ### Local ONNX Inference
//! ```no_run
//! use ufp_semantic::{semanticize, SemanticConfig};
//! use std::path::PathBuf;
//!
//! let cfg = SemanticConfig {
//!     model_path: PathBuf::from("path/to/your/model.onnx"),
//!     tokenizer_path: Some(PathBuf::from("path/to/your/tokenizer.json")),
//!     ..Default::default()
//! };
//!
//! let embedding = semanticize("doc-1", "This is a test.", &cfg).unwrap();
//! ```
//!
//! ### Remote API Inference
//! ```no_run
//! use ufp_semantic::{semanticize, SemanticConfig};
//!
//! let cfg = SemanticConfig {
//!     mode: "api".into(),
//!     api_url: Some("https://api-inference.huggingface.co/models/BAAI/bge-small-en-v1.5".into()),
//!     api_auth_header: Some("Bearer YOUR_HF_TOKEN".into()),
//!     api_provider: Some("hf".into()),
//!     ..Default::default()
//! };
//!
//! let embedding = semanticize("doc-2", "Another test.", &cfg).unwrap();
//! ```

pub mod config;
pub mod error;
pub mod types;

mod api;
mod assets;
mod cache;
mod normalize;
mod onnx;
mod stub;

pub use crate::config::SemanticConfig;
pub use crate::error::SemanticError;
pub use crate::types::SemanticEmbedding;

use crate::api::{semanticize_batch_via_api, semanticize_via_api};
use crate::assets::{resolve_model_assets, should_fallback_to_stub};
use crate::cache::get_or_load_model_handle;
use crate::normalize::l2_normalize_in_place;
use crate::onnx::run_onnx_embeddings;
use crate::stub::make_stub_embedding;

/// Converts the provided `text` into a [`SemanticEmbedding`] using the supplied [`SemanticConfig`].
///
/// When `cfg.tier == "fast"` the deterministic stub is returned immediately. For other tiers the
/// function resolves ONNX/tokenizer assets (downloading remote URLs if necessary), runs inference,
/// normalizes the vector if requested, and returns the enriched metadata bundle.
pub fn semanticize(
    doc_id: &str,
    text: &str,
    cfg: &SemanticConfig,
) -> Result<SemanticEmbedding, SemanticError> {
    // --- Mode selection ---
    match cfg.mode.as_str() {
        "fast" => return Ok(make_stub_embedding(doc_id, text, cfg)),
        "api" => return semanticize_via_api(doc_id, text, cfg),
        "onnx" => {} // Continue to ONNX logic
        _ => {}      // Default to ONNX for unknown modes
    }

    // The "fast" tier is a shortcut to the stub embedding, regardless of mode.
    if cfg.tier == "fast" {
        return Ok(make_stub_embedding(doc_id, text, cfg));
    }

    // --- Asset resolution ---
    // Attempt to resolve model assets, but fall back to a stub if they are not found
    // and no download URLs are provided. This makes the system resilient to missing assets.
    let assets = match resolve_model_assets(cfg) {
        Ok(assets) => assets,
        Err(err) if should_fallback_to_stub(&err) => {
            return Ok(make_stub_embedding(doc_id, text, cfg));
        }
        Err(err) => return Err(err),
    };

    // --- Inference ---
    // Get a handle to the cached model, loading it if necessary.
    let handle = get_or_load_model_handle(&assets)?;
    let texts = [text];
    // Run the ONNX model to get the embeddings.
    let mut vectors = run_onnx_embeddings(handle.as_ref(), &texts)?;
    let mut embedding = vectors
        .pop()
        .ok_or_else(|| SemanticError::Inference("model returned no outputs".into()))?;

    // --- Post-processing ---
    // Normalize the embedding to unit length if requested. This is important for cosine similarity.
    if cfg.normalize {
        l2_normalize_in_place(&mut embedding);
    }

    let embedding_dim = embedding.len();

    Ok(SemanticEmbedding {
        doc_id: doc_id.to_string(),
        vector: embedding,
        model_name: cfg.model_name.clone(),
        tier: cfg.tier.clone(),
        embedding_dim,
        normalized: cfg.normalize,
    })
}

/// Batch variant of [`semanticize`] that reuses the configured mode.
///
/// For `"api"` mode, the function prefers provider-native batch semantics; ONNX mode now shares the
/// cached session and executes a single batched inference (padding shorter sequences) so callers
/// pay the setup cost only once per batch.
pub fn semanticize_batch<'a, D, T>(
    docs: &'a [(D, T)],
    cfg: &SemanticConfig,
) -> Result<Vec<SemanticEmbedding>, SemanticError>
where
    D: AsRef<str> + 'a,
    T: AsRef<str> + 'a,
{
    // --- Mode selection ---
    match cfg.mode.as_str() {
        "fast" => {
            return docs
                .iter()
                .map(|(doc_id, text)| Ok(make_stub_embedding(doc_id.as_ref(), text.as_ref(), cfg)))
                .collect()
        }
        "api" => return semanticize_batch_via_api(docs, cfg),
        _ => {} // Default to ONNX
    }

    if docs.is_empty() {
        return Ok(Vec::new());
    }

    if cfg.tier == "fast" {
        return docs
            .iter()
            .map(|(doc_id, text)| Ok(make_stub_embedding(doc_id.as_ref(), text.as_ref(), cfg)))
            .collect();
    }

    // --- Asset resolution ---
    let assets = match resolve_model_assets(cfg) {
        Ok(assets) => assets,
        Err(err) if should_fallback_to_stub(&err) => {
            return docs
                .iter()
                .map(|(doc_id, text)| Ok(make_stub_embedding(doc_id.as_ref(), text.as_ref(), cfg)))
                .collect();
        }
        Err(err) => return Err(err),
    };

    // --- Inference ---
    let handle = get_or_load_model_handle(&assets)?;
    let text_refs: Vec<&str> = docs.iter().map(|(_, text)| text.as_ref()).collect();
    let embeddings = run_onnx_embeddings(handle.as_ref(), &text_refs)?;
    if embeddings.len() != docs.len() {
        return Err(SemanticError::Inference(format!(
            "model returned {} embeddings for {} inputs",
            embeddings.len(),
            docs.len()
        )));
    }

    // --- Post-processing ---
    let mut results = Vec::with_capacity(docs.len());
    for ((doc_id, _), mut vector) in docs.iter().zip(embeddings.into_iter()) {
        if cfg.normalize {
            l2_normalize_in_place(&mut vector);
        }
        let embedding_dim = vector.len();
        results.push(SemanticEmbedding {
            doc_id: doc_id.as_ref().to_owned(),
            vector,
            model_name: cfg.model_name.clone(),
            tier: cfg.tier.clone(),
            embedding_dim,
            normalized: cfg.normalize,
        });
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_stub_determinism() {
        let cfg = SemanticConfig {
            tier: "fast".into(),
            ..SemanticConfig::default()
        };
        let e1 = semanticize("d1", "big cat", &cfg).unwrap();
        let e2 = semanticize("d1", "big cat", &cfg).unwrap();
        assert_eq!(e1.vector, e2.vector);
    }

    #[test]
    fn semanticize_falls_back_when_model_missing() {
        let cfg = SemanticConfig {
            model_path: PathBuf::from("./missing/model.onnx"),
            tokenizer_path: Some(PathBuf::from("./missing/tokenizer.json")),
            tier: "balanced".into(),
            ..SemanticConfig::default()
        };

        let embedding = semanticize("doc-stub", "fallback text", &cfg)
            .expect("missing assets should produce stub");
        let stub = make_stub_embedding("doc-stub", "fallback text", &cfg);
        assert_eq!(embedding.vector, stub.vector);
        assert_eq!(embedding.embedding_dim, stub.embedding_dim);
    }

    #[test]
    fn semanticize_batch_falls_back_when_model_missing() {
        let cfg = SemanticConfig {
            model_path: PathBuf::from("./missing/model.onnx"),
            tokenizer_path: Some(PathBuf::from("./missing/tokenizer.json")),
            tier: "balanced".into(),
            ..SemanticConfig::default()
        };

        let docs = vec![("doc-a", "hello"), ("doc-b", "world")];
        let embeddings =
            semanticize_batch(&docs, &cfg).expect("batch fallback should produce stub embeddings");
        assert_eq!(embeddings.len(), docs.len());

        for (actual, (doc_id, text)) in embeddings.iter().zip(docs.iter()) {
            let stub = make_stub_embedding(doc_id, text, &cfg);
            assert_eq!(actual.vector, stub.vector);
            assert_eq!(actual.embedding_dim, stub.embedding_dim);
        }
    }

    #[test]
    #[ignore = "requires local ONNX + tokenizer assets under models/"]
    fn test_real_model_inference() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root");

        let model_path = workspace_root
            .join("models")
            .join("bge-small-en-v1.5")
            .join("onnx")
            .join("model.onnx");
        let tokenizer_path = workspace_root
            .join("models")
            .join("bge-small-en-v1.5")
            .join("tokenizer.json");

        assert!(
            model_path.exists(),
            "expected ONNX model at {}",
            model_path.display()
        );
        assert!(
            tokenizer_path.exists(),
            "expected tokenizer json at {}",
            tokenizer_path.display()
        );

        let cfg = SemanticConfig {
            model_path,
            tokenizer_path: Some(tokenizer_path),
            tier: "balanced".into(),
            ..SemanticConfig::default()
        };

        let embedding = semanticize("doc1", "hello world", &cfg)
            .expect("inference should succeed with real model");

        assert!(
            embedding.embedding_dim > 0 && !embedding.vector.is_empty(),
            "embedding should have non-zero dimensions"
        );
        assert_eq!(embedding.doc_id, "doc1");
        assert!(embedding.normalized);
    }
}
