use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
