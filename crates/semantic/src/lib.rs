//! UCFP Semantic Fingerprinting
//!
//! This crate handles turning text into meaning-aware vectors. Given canonicalized
//! text, it spits out dense embeddings you can use for similarity search,
//! clustering, or whatever semantic stuff you're into.
//!
//! We support a few modes:
//!
//! - **ONNX mode** - Run models locally. Requires model files.
//! - **API mode** - Call out to Hugging Face (router endpoint is your friend)
//! - **Stub mode** - For testing. Generates fake but consistent vectors.
//!
//! The nice thing is the fallback behavior. If a model file is missing or an API
//! call fails, we fall back to stub mode instead of panicking. Saved our bacon
//! more than once in production.
//!
//! ## Threading notes
//!
//! Tokenizers and ONNX sessions get cached per-thread. First call on any thread
//! does the expensive setup. After that, it's fast. Batches work too.
//!
//! ## Quick example
//!
//! ```no_run
//! use semantic::{semanticize, SemanticConfig};
//! use std::path::PathBuf;
//!
//! #[tokio::main]
//! async fn main() {
//!     let cfg = SemanticConfig {
//!         model_path: PathBuf::from("path/to/your/model.onnx"),
//!         tokenizer_path: Some(PathBuf::from("path/to/your/tokenizer.json")),
//!         ..Default::default()
//!     };
//!
//!     let embedding = semanticize("doc-1", "This is a test.", &cfg).await.unwrap();
//! }
//! ```
//!
//! ## API mode example
//!
//! ```no_run
//! use semantic::{semanticize, SemanticConfig};
//!
//! #[tokio::main]
//! async fn main() {
//!     let cfg = SemanticConfig {
//!         mode: "api".into(),
//!         api_url: Some("https://router.huggingface.co/hf-inference/models/BAAI/bge-small-en-v1.5/pipeline/feature-extraction".into()),
//!         api_auth_header: Some("Bearer YOUR_HF_TOKEN".into()),
//!         api_provider: Some("auto".into()),
//!         ..Default::default()
//!     };
//!
//!     let embedding = semanticize("doc-2", "Another test.", &cfg).await.unwrap();
//! }
//! ```
//!
//! ## Env vars to know
//!
//! - `UFP_SEMANTIC_API_URL` - Override the API endpoint
//! - `UFP_SEMANTIC_API_TOKEN` - Your HF token
//!
//! Full example at `examples/api_embed.rs`.

pub mod config;
pub mod error;
pub mod types;

// Resilience bits
pub mod circuit_breaker;
pub mod rate_limit;
pub mod retry;
mod serde_millis;

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
pub async fn semanticize(
    doc_id: &str,
    text: &str,
    cfg: &SemanticConfig,
) -> Result<SemanticEmbedding, SemanticError> {
    // --- Mode selection ---
    match cfg.mode.as_str() {
        "fast" => return Ok(make_stub_embedding(doc_id, text, cfg)),
        "api" => return semanticize_via_api(doc_id, text, cfg).await,
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
    let assets = match resolve_model_assets(cfg).await {
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
    let mut vectors = run_onnx_embeddings(
        handle.as_ref(),
        &texts,
        cfg.max_sequence_length,
        cfg.enable_chunking,
        cfg.chunk_overlap_ratio,
        &cfg.pooling_strategy,
    )?;
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
pub async fn semanticize_batch<'a, D, T>(
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
        "api" => return semanticize_batch_via_api(docs, cfg).await,
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
    let assets = match resolve_model_assets(cfg).await {
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
    let embeddings = run_onnx_embeddings(
        handle.as_ref(),
        &text_refs,
        cfg.max_sequence_length,
        cfg.enable_chunking,
        cfg.chunk_overlap_ratio,
        &cfg.pooling_strategy,
    )?;
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

    #[tokio::test]
    async fn test_stub_determinism() {
        let cfg = SemanticConfig {
            tier: "fast".into(),
            ..SemanticConfig::default()
        };
        let e1 = semanticize("d1", "big cat", &cfg).await.unwrap();
        let e2 = semanticize("d1", "big cat", &cfg).await.unwrap();
        assert_eq!(e1.vector, e2.vector);
    }

    #[tokio::test]
    async fn semanticize_falls_back_when_model_missing() {
        let cfg = SemanticConfig {
            model_path: PathBuf::from("./missing/model.onnx"),
            tokenizer_path: Some(PathBuf::from("./missing/tokenizer.json")),
            tier: "balanced".into(),
            ..SemanticConfig::default()
        };

        let embedding = semanticize("doc-stub", "fallback text", &cfg)
            .await
            .expect("missing assets should produce stub");
        let stub = make_stub_embedding("doc-stub", "fallback text", &cfg);
        assert_eq!(embedding.vector, stub.vector);
        assert_eq!(embedding.embedding_dim, stub.embedding_dim);
    }

    #[tokio::test]
    async fn semanticize_batch_falls_back_when_model_missing() {
        let cfg = SemanticConfig {
            model_path: PathBuf::from("./missing/model.onnx"),
            tokenizer_path: Some(PathBuf::from("./missing/tokenizer.json")),
            tier: "balanced".into(),
            ..SemanticConfig::default()
        };

        let docs = vec![("doc-a", "hello"), ("doc-b", "world")];
        let embeddings = semanticize_batch(&docs, &cfg)
            .await
            .expect("batch fallback should produce stub embeddings");
        assert_eq!(embeddings.len(), docs.len());

        for (actual, (doc_id, text)) in embeddings.iter().zip(docs.iter()) {
            let stub = make_stub_embedding(doc_id, text, &cfg);
            assert_eq!(actual.vector, stub.vector);
            assert_eq!(actual.embedding_dim, stub.embedding_dim);
        }
    }

    #[tokio::test]
    #[ignore = "requires local ONNX + tokenizer assets under models/"]
    async fn test_real_model_inference() {
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
            .await
            .expect("inference should succeed with real model");

        assert!(
            embedding.embedding_dim > 0 && !embedding.vector.is_empty(),
            "embedding should have non-zero dimensions"
        );
        assert_eq!(embedding.doc_id, "doc1");
        assert!(embedding.normalized);
    }

    // Additional integration tests

    #[tokio::test]
    async fn semanticize_fast_mode() {
        let cfg = SemanticConfig {
            mode: "fast".into(),
            tier: "balanced".into(),
            normalize: false,
            ..Default::default()
        };

        let embedding = semanticize("doc1", "hello world", &cfg).await.unwrap();
        assert_eq!(embedding.embedding_dim, 768);
        assert!(!embedding.normalized);
    }

    #[tokio::test]
    async fn semanticize_batch_empty() {
        let cfg = SemanticConfig::default();
        let docs: Vec<(&str, &str)> = vec![];

        let embeddings = semanticize_batch(&docs, &cfg).await.unwrap();
        assert!(embeddings.is_empty());
    }

    #[tokio::test]
    async fn semanticize_batch_single() {
        let cfg = SemanticConfig {
            tier: "fast".into(),
            ..Default::default()
        };

        let docs = vec![("doc-1", "single document")];
        let embeddings = semanticize_batch(&docs, &cfg).await.unwrap();

        assert_eq!(embeddings.len(), 1);
        assert_eq!(embeddings[0].doc_id, "doc-1");
    }

    #[tokio::test]
    async fn semanticize_batch_multiple() {
        let cfg = SemanticConfig {
            tier: "fast".into(),
            ..Default::default()
        };

        let docs = vec![("doc-1", "first"), ("doc-2", "second"), ("doc-3", "third")];
        let embeddings = semanticize_batch(&docs, &cfg).await.unwrap();

        assert_eq!(embeddings.len(), 3);
        assert_eq!(embeddings[0].doc_id, "doc-1");
        assert_eq!(embeddings[1].doc_id, "doc-2");
        assert_eq!(embeddings[2].doc_id, "doc-3");
    }

    #[tokio::test]
    async fn semanticize_different_texts_produce_different_embeddings() {
        let cfg = SemanticConfig {
            tier: "fast".into(),
            ..Default::default()
        };

        let e1 = semanticize("doc1", "hello world", &cfg).await.unwrap();
        let e2 = semanticize("doc2", "goodbye world", &cfg).await.unwrap();

        assert_ne!(e1.vector, e2.vector);
    }

    #[tokio::test]
    async fn semanticize_all_tiers() {
        for tier in ["fast", "balanced", "accurate"] {
            let cfg = SemanticConfig {
                tier: tier.into(),
                mode: "fast".into(),
                ..Default::default()
            };

            let embedding = semanticize("doc", "test", &cfg).await.unwrap();

            match tier {
                "fast" => assert_eq!(embedding.embedding_dim, 384),
                "balanced" => assert_eq!(embedding.embedding_dim, 768),
                "accurate" => assert_eq!(embedding.embedding_dim, 1024),
                _ => panic!("unknown tier"),
            }
        }
    }

    #[tokio::test]
    async fn semanticize_with_normalization() {
        let cfg = SemanticConfig {
            tier: "fast".into(),
            mode: "fast".into(),
            normalize: true,
            ..Default::default()
        };

        let embedding = semanticize("doc", "test", &cfg).await.unwrap();

        assert!(embedding.normalized);
        let norm: f32 = embedding.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-4);
    }

    #[tokio::test]
    async fn semanticize_without_normalization() {
        let cfg = SemanticConfig {
            tier: "fast".into(),
            mode: "fast".into(),
            normalize: false,
            ..Default::default()
        };

        let embedding = semanticize("doc", "test", &cfg).await.unwrap();

        assert!(!embedding.normalized);
    }

    #[tokio::test]
    async fn semanticize_preserves_doc_id() {
        let cfg = SemanticConfig {
            tier: "fast".into(),
            ..Default::default()
        };

        let doc_id = "my-special-doc-id-123";
        let embedding = semanticize(doc_id, "test", &cfg).await.unwrap();

        assert_eq!(embedding.doc_id, doc_id);
    }

    #[tokio::test]
    async fn semanticize_preserves_model_name() {
        let cfg = SemanticConfig {
            tier: "fast".into(),
            mode: "fast".into(),
            model_name: "my-custom-model".into(),
            ..Default::default()
        };

        let embedding = semanticize("doc", "test", &cfg).await.unwrap();

        assert_eq!(embedding.model_name, "my-custom-model");
    }

    #[tokio::test]
    async fn semanticize_unicode_text() {
        let cfg = SemanticConfig {
            tier: "fast".into(),
            ..Default::default()
        };

        let embedding = semanticize("doc", "Hello 世界", &cfg).await.unwrap();
        assert!(embedding.embedding_dim > 0);
        assert!(!embedding.vector.is_empty());
    }

    #[tokio::test]
    async fn semanticize_empty_text() {
        let cfg = SemanticConfig {
            tier: "fast".into(),
            ..Default::default()
        };

        let embedding = semanticize("doc", "", &cfg).await.unwrap();
        assert!(embedding.embedding_dim > 0);
        assert!(!embedding.vector.is_empty());
    }

    #[tokio::test]
    async fn semanticize_long_text() {
        let cfg = SemanticConfig {
            tier: "fast".into(),
            ..Default::default()
        };

        let long_text = "word ".repeat(1000);
        let embedding = semanticize("doc", &long_text, &cfg).await.unwrap();
        assert!(embedding.embedding_dim > 0);
    }

    #[tokio::test]
    async fn semanticize_special_characters() {
        let cfg = SemanticConfig {
            tier: "fast".into(),
            ..Default::default()
        };

        let embedding = semanticize("doc", "!@#$%^&*()_+", &cfg).await.unwrap();
        assert!(embedding.embedding_dim > 0);
    }

    #[tokio::test]
    async fn semanticize_multiline_text() {
        let cfg = SemanticConfig {
            tier: "fast".into(),
            ..Default::default()
        };

        let text = "Line 1\nLine 2\nLine 3";
        let embedding = semanticize("doc", text, &cfg).await.unwrap();
        assert!(embedding.embedding_dim > 0);
    }

    #[tokio::test]
    async fn semanticize_determinism_repeated_calls() {
        let cfg = SemanticConfig {
            tier: "fast".into(),
            ..Default::default()
        };

        let text = "test text for determinism";

        for _ in 0..10 {
            let e1 = semanticize("doc", text, &cfg).await.unwrap();
            let e2 = semanticize("doc", text, &cfg).await.unwrap();
            assert_eq!(e1.vector, e2.vector);
        }
    }

    #[tokio::test]
    async fn batch_preserves_order() {
        let cfg = SemanticConfig {
            tier: "fast".into(),
            ..Default::default()
        };

        let docs = vec![
            ("first", "doc one content"),
            ("second", "doc two content"),
            ("third", "doc three content"),
        ];

        let embeddings = semanticize_batch(&docs, &cfg).await.unwrap();

        assert_eq!(embeddings[0].doc_id, "first");
        assert_eq!(embeddings[1].doc_id, "second");
        assert_eq!(embeddings[2].doc_id, "third");
    }

    #[tokio::test]
    async fn batch_different_texts_different_embeddings() {
        let cfg = SemanticConfig {
            tier: "fast".into(),
            ..Default::default()
        };

        let docs = vec![("a", "hello"), ("b", "world")];
        let embeddings = semanticize_batch(&docs, &cfg).await.unwrap();

        assert_ne!(embeddings[0].vector, embeddings[1].vector);
    }

    #[tokio::test]
    async fn fast_mode_ignores_model_settings() {
        let cfg = SemanticConfig {
            mode: "fast".into(),
            tier: "accurate".into(),
            model_path: PathBuf::from("/nonexistent/model.onnx"),
            ..Default::default()
        };

        let embedding = semanticize("doc", "test", &cfg).await.unwrap();
        assert_eq!(embedding.tier, "accurate");
        assert_eq!(embedding.embedding_dim, 1024);
    }

    #[tokio::test]
    async fn tier_fast_ignores_onnx() {
        let cfg = SemanticConfig {
            tier: "fast".into(),
            mode: "onnx".into(),
            model_path: PathBuf::from("/nonexistent/model.onnx"),
            ..Default::default()
        };

        let embedding = semanticize("doc", "test", &cfg).await.unwrap();
        assert_eq!(embedding.tier, "fast");
        assert_eq!(embedding.embedding_dim, 384);
    }
}
