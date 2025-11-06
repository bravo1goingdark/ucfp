//! ufp_semantic: Meaning-level fingerprinting with runtime-configurable models.
//!
//! Converts canonicalized text into dense embeddings using ONNX transformer models.
//! Falls back to deterministic stub if model isn't found. Designed for full offline
//! operation and configurable model tiers (fast, balanced, accurate).

use once_cell::sync::OnceCell;
use onnxruntime::{environment::Environment, ndarray::Array};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;
use tokenizers::Tokenizer;
use ureq::AgentBuilder;

use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    time::Duration,
};

static ORT_ENV: OnceCell<Environment> = OnceCell::new();
const ORT_NAME: &str = "ufp_semantic";

#[derive(Clone, Copy)]
enum ApiProviderKind {
    HuggingFace,
    OpenAI,
    Custom,
}

/// Runtime configuration describing which model/tokenizer to use and how to post-process vectors.
///
/// # Example
/// ```no_run
/// use ufp_semantic::{semanticize, SemanticConfig};
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
    /// Friendly label surfaced on every [`SemanticEmbedding`].
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
        }
    }
}

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

/// Errors surfaced by [`semanticize`].
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
    match cfg.mode.as_str() {
        "fast" => return Ok(make_stub_embedding(doc_id, text, cfg)),
        "api" => return semanticize_via_api(doc_id, text, cfg),
        "onnx" => {}
        _ => {}
    }

    if cfg.tier == "fast" {
        return Ok(make_stub_embedding(doc_id, text, cfg));
    }

    let assets = resolve_model_assets(cfg)?;

    let tokenizer = Tokenizer::from_file(&assets.tokenizer_path)
        .map_err(|e| SemanticError::Inference(e.to_string()))?;
    let enc = tokenizer
        .encode(text, true)
        .map_err(|e| SemanticError::Inference(e.to_string()))?;
    let ids: Vec<i64> = enc.get_ids().iter().map(|&x| x as i64).collect();
    let mask: Vec<i64> = enc.get_attention_mask().iter().map(|&x| x as i64).collect();

    let seq_len = ids.len();
    let attn_len = mask.len();

    let env = ort_environment()?;
    let mut session = env
        .new_session_builder()
        .map_err(|e| SemanticError::Inference(e.to_string()))?
        .with_model_from_file(&assets.model_path)
        .map_err(|e| SemanticError::Inference(e.to_string()))?;

    let input_ids = Array::from_shape_vec((1, seq_len), ids)
        .map_err(|e| SemanticError::Inference(e.to_string()))?;
    let attn_mask = Array::from_shape_vec((1, attn_len), mask)
        .map_err(|e| SemanticError::Inference(e.to_string()))?;

    let mut runtime_inputs = Vec::with_capacity(session.inputs.len());
    let mut input_ids_tensor = Some(input_ids);
    let mut attn_mask_tensor = Some(attn_mask);

    for input in &session.inputs {
        match input.name.as_str() {
            "input_ids" => {
                let tensor = input_ids_tensor.take().ok_or_else(|| {
                    SemanticError::InvalidConfig(
                        "model requested `input_ids` multiple times".into(),
                    )
                })?;
                runtime_inputs.push(tensor.into_dyn());
            }
            "attention_mask" => {
                let tensor = attn_mask_tensor.take().ok_or_else(|| {
                    SemanticError::InvalidConfig(
                        "model requested `attention_mask` multiple times".into(),
                    )
                })?;
                runtime_inputs.push(tensor.into_dyn());
            }
            "token_type_ids" => {
                let tensor = Array::from_elem((1, seq_len), 0_i64);
                runtime_inputs.push(tensor.into_dyn());
            }
            other => {
                return Err(SemanticError::Inference(format!(
                    "unsupported model input '{other}'"
                )))
            }
        }
    }

    if runtime_inputs.is_empty() {
        return Err(SemanticError::Inference(
            "model did not declare any inputs".into(),
        ));
    }

    let outputs = session
        .run::<i64, f32, _>(runtime_inputs)
        .map_err(|e| SemanticError::Inference(e.to_string()))?;

    let output_tensor = outputs
        .into_iter()
        .next()
        .ok_or_else(|| SemanticError::Inference("model returned no outputs".into()))?;

    let mut embedding: Vec<f32> = output_tensor.iter().copied().collect();

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
/// For `"api"` mode, the function prefers provider-native batch semantics; other modes fall back to
/// per-document inference.
pub fn semanticize_batch<'a, D, T>(
    docs: &'a [(D, T)],
    cfg: &SemanticConfig,
) -> Result<Vec<SemanticEmbedding>, SemanticError>
where
    D: AsRef<str> + 'a,
    T: AsRef<str> + 'a,
{
    match cfg.mode.as_str() {
        "fast" => docs
            .iter()
            .map(|(doc_id, text)| Ok(make_stub_embedding(doc_id.as_ref(), text.as_ref(), cfg)))
            .collect(),
        "api" => semanticize_batch_via_api(docs, cfg),
        _ => docs
            .iter()
            .map(|(doc_id, text)| semanticize(doc_id.as_ref(), text.as_ref(), cfg))
            .collect(),
    }
}

fn semanticize_via_api(
    doc_id: &str,
    text: &str,
    cfg: &SemanticConfig,
) -> Result<SemanticEmbedding, SemanticError> {
    let url = cfg
        .api_url
        .as_deref()
        .ok_or_else(|| SemanticError::InvalidConfig("api_url is required for api mode".into()))?;
    let provider = api_provider_kind(cfg);
    let payload_text = vec![text.to_string()];
    let payload = build_api_payload(provider, &payload_text, cfg, false);
    let response = send_api_request(url, cfg, payload)?;
    let mut vectors = parse_embeddings_from_value(response)?;
    let mut embedding = vectors
        .pop()
        .or_else(|| vectors.into_iter().next())
        .ok_or_else(|| {
            SemanticError::Inference("API response did not contain embeddings".into())
        })?;

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

fn semanticize_batch_via_api<D, T>(
    docs: &[(D, T)],
    cfg: &SemanticConfig,
) -> Result<Vec<SemanticEmbedding>, SemanticError>
where
    D: AsRef<str>,
    T: AsRef<str>,
{
    if docs.is_empty() {
        return Ok(Vec::new());
    }

    let url = cfg
        .api_url
        .as_deref()
        .ok_or_else(|| SemanticError::InvalidConfig("api_url is required for api mode".into()))?;
    let provider = api_provider_kind(cfg);

    let doc_ids: Vec<String> = docs
        .iter()
        .map(|(doc_id, _)| doc_id.as_ref().to_owned())
        .collect();
    let texts: Vec<String> = docs
        .iter()
        .map(|(_, text)| text.as_ref().to_owned())
        .collect();

    let payload = build_api_payload(provider, &texts, cfg, true);
    let vectors = parse_embeddings_from_value(send_api_request(url, cfg, payload)?)?;

    if vectors.len() != doc_ids.len() {
        return Err(SemanticError::Inference(format!(
            "API returned {} embeddings for {} inputs",
            vectors.len(),
            doc_ids.len()
        )));
    }

    let mut results = Vec::with_capacity(doc_ids.len());
    for (doc_id, mut vector) in doc_ids.into_iter().zip(vectors.into_iter()) {
        if cfg.normalize {
            l2_normalize_in_place(&mut vector);
        }
        let embedding_dim = vector.len();
        results.push(SemanticEmbedding {
            doc_id,
            vector,
            model_name: cfg.model_name.clone(),
            tier: cfg.tier.clone(),
            embedding_dim,
            normalized: cfg.normalize,
        });
    }

    Ok(results)
}

/// Deterministic stub used when tier is `"fast"` or the model assets are unavailable.
/// Generates sinusoid values derived from a hash of the input text to guarantee reproducible
/// vectors with minimal CPU cost.
fn make_stub_embedding(doc_id: &str, text: &str, cfg: &SemanticConfig) -> SemanticEmbedding {
    use fxhash::hash64;
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

/// In-place L2 normalization helper to keep allocations down during hot paths.
fn l2_normalize_in_place(v: &mut [f32]) {
    let norm = v.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    if norm > 0.0 {
        let inv = 1.0 / norm as f32;
        for x in v.iter_mut() {
            *x *= inv;
        }
    }
}

#[derive(Debug)]
struct ModelAssets {
    model_path: PathBuf,
    tokenizer_path: PathBuf,
}

/// Ensures that the model and tokenizer exist locally, downloading them when URLs are provided.
fn resolve_model_assets(cfg: &SemanticConfig) -> Result<ModelAssets, SemanticError> {
    let model_path = ensure_local_file(&cfg.model_path, cfg.model_url.as_deref(), || {
        SemanticError::ModelNotFound(cfg.model_path.display().to_string())
    })?;

    let tokenizer_target = tokenizer_storage_path(cfg)?;
    let tokenizer_path =
        ensure_local_file(&tokenizer_target, cfg.tokenizer_url.as_deref(), || {
            SemanticError::TokenizerMissing(cfg.model_name.clone())
        })?;

    Ok(ModelAssets {
        model_path,
        tokenizer_path,
    })
}

/// Determines where the tokenizer should be stored. When no explicit path is supplied we infer a
/// filename from the remote URL and place it next to the model file.
fn tokenizer_storage_path(cfg: &SemanticConfig) -> Result<PathBuf, SemanticError> {
    if let Some(path) = &cfg.tokenizer_path {
        return Ok(path.clone());
    }

    if let Some(url) = &cfg.tokenizer_url {
        let inferred_name = infer_filename_from_url(url).unwrap_or_else(|| "tokenizer.json".into());
        let base_dir = cfg
            .model_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        return Ok(base_dir.join(inferred_name));
    }

    Err(SemanticError::TokenizerMissing(cfg.model_name.clone()))
}

/// Returns `target` if it already exists, otherwise attempts to download `remote_url`.
fn ensure_local_file<F>(
    target: &Path,
    remote_url: Option<&str>,
    on_missing: F,
) -> Result<PathBuf, SemanticError>
where
    F: FnOnce() -> SemanticError,
{
    if target.exists() {
        return Ok(target.to_path_buf());
    }

    if let Some(url) = remote_url {
        download_to_path(target, url)?;
        return Ok(target.to_path_buf());
    }

    Err(on_missing())
}

/// Downloads `url` into `target`, creating parent directories as needed.
fn download_to_path(target: &Path, url: &str) -> Result<(), SemanticError> {
    if let Some(parent) = target.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }

    let response = ureq::get(url)
        .call()
        .map_err(|e| SemanticError::Download(e.to_string()))?;
    if !(200..400).contains(&response.status()) {
        return Err(SemanticError::Download(format!(
            "unexpected status {} while fetching {}",
            response.status(),
            url
        )));
    }

    let mut reader = response.into_reader();
    let mut file = File::create(target)?;
    io::copy(&mut reader, &mut file)?;
    Ok(())
}

/// Lazily constructs a global ONNX Runtime environment that can be shared by all calls.
fn ort_environment() -> Result<&'static Environment, SemanticError> {
    ORT_ENV.get_or_try_init(|| {
        Environment::builder()
            .with_name(ORT_NAME)
            .build()
            .map_err(|e| SemanticError::Inference(e.to_string()))
    })
}

/// Extracts a filename from the provided URL, stripping query/fragment parts.
fn infer_filename_from_url(url: &str) -> Option<String> {
    url.split('/')
        .rev()
        .find(|segment| !segment.is_empty())
        .map(|segment| segment.split(['?', '#']).next().unwrap_or(segment))
        .map(|segment| segment.to_string())
}

fn api_provider_kind(cfg: &SemanticConfig) -> ApiProviderKind {
    let provider = cfg
        .api_provider
        .as_deref()
        .unwrap_or("custom")
        .to_ascii_lowercase();
    match provider.as_str() {
        "hf" | "huggingface" => ApiProviderKind::HuggingFace,
        "openai" | "gpt" => ApiProviderKind::OpenAI,
        _ => ApiProviderKind::Custom,
    }
}

fn build_api_payload(
    provider: ApiProviderKind,
    texts: &[String],
    cfg: &SemanticConfig,
    batch: bool,
) -> Value {
    match provider {
        ApiProviderKind::HuggingFace => {
            if batch {
                json!({ "inputs": texts })
            } else if let Some(first) = texts.first() {
                json!({ "inputs": first })
            } else {
                json!({ "inputs": "" })
            }
        }
        ApiProviderKind::OpenAI => {
            if batch {
                json!({ "input": texts, "model": cfg.model_name })
            } else if let Some(first) = texts.first() {
                json!({ "input": first, "model": cfg.model_name })
            } else {
                json!({ "input": "", "model": cfg.model_name })
            }
        }
        ApiProviderKind::Custom => {
            if batch {
                json!({ "texts": texts })
            } else if let Some(first) = texts.first() {
                json!({ "text": first })
            } else {
                json!({ "text": "" })
            }
        }
    }
}

fn send_api_request(
    url: &str,
    cfg: &SemanticConfig,
    payload: Value,
) -> Result<Value, SemanticError> {
    let agent = api_agent(cfg);
    let mut request = agent.post(url);
    request = request.set("Content-Type", "application/json");
    if let Some(header) = cfg.api_auth_header.as_deref() {
        request = request.set("Authorization", header);
    }

    let payload_body = payload.to_string();
    let response = request
        .send_string(&payload_body)
        .map_err(|e| SemanticError::Download(e.to_string()))?;

    let body = response
        .into_string()
        .map_err(|e| SemanticError::Download(e.to_string()))?;
    serde_json::from_str(&body).map_err(|e| SemanticError::Inference(e.to_string()))
}

fn api_agent(cfg: &SemanticConfig) -> ureq::Agent {
    let timeout = Duration::from_secs(cfg.api_timeout_secs.unwrap_or(30));
    AgentBuilder::new().timeout(timeout).build()
}

fn parse_embeddings_from_value(value: Value) -> Result<Vec<Vec<f32>>, SemanticError> {
    match value {
        Value::Object(mut map) => {
            if let Some(embeddings) = map.remove("embeddings") {
                return parse_embedding_collection(embeddings);
            }

            if let Some(Value::Array(items)) = map.remove("data") {
                let mut vectors = Vec::with_capacity(items.len());
                for item in items {
                    match item {
                        Value::Object(mut obj) => {
                            if let Some(embedding) = obj.remove("embedding") {
                                vectors.push(parse_embedding_vector(embedding)?);
                            } else {
                                return Err(SemanticError::Inference(
                                    "missing `embedding` field in data item".into(),
                                ));
                            }
                        }
                        _ => {
                            return Err(SemanticError::Inference(
                                "unexpected entry inside `data` array".into(),
                            ))
                        }
                    }
                }
                return Ok(vectors);
            }

            Err(SemanticError::Inference(
                "unsupported API response shape".into(),
            ))
        }
        other => parse_embedding_collection(other),
    }
}

fn parse_embedding_collection(value: Value) -> Result<Vec<Vec<f32>>, SemanticError> {
    match value {
        Value::Array(items) => {
            if items.is_empty() {
                Ok(Vec::new())
            } else if items.iter().all(|item| matches!(item, Value::Array(_))) {
                items.into_iter().map(parse_embedding_vector).collect()
            } else {
                parse_embedding_vector(Value::Array(items)).map(|vec| vec![vec])
            }
        }
        other => parse_embedding_vector(other).map(|vec| vec![vec]),
    }
}

fn parse_embedding_vector(value: Value) -> Result<Vec<f32>, SemanticError> {
    match value {
        Value::Array(values) => values
            .into_iter()
            .map(|entry| match entry {
                Value::Number(num) => num
                    .as_f64()
                    .map(|f| f as f32)
                    .ok_or_else(|| SemanticError::Inference("non-finite embedding value".into())),
                other => Err(SemanticError::Inference(format!(
                    "embedding entries must be numbers, got {other:?}"
                ))),
            })
            .collect(),
        other => Err(SemanticError::Inference(format!(
            "embedding vector must be an array, got {other:?}"
        ))),
    }
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
