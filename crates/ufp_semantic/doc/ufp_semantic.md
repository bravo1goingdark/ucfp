# UCFP Semantic

## Purpose

`ufp_semantic` converts canonicalized text into dense vector embeddings using transformer models
packaged as ONNX graphs. The crate keeps runtime requirements tiny (CPU-only by default), provides
stubbed outputs for offline or "fast" tiers, and normalizes every embedding so downstream search and
similarity components receive consistent vectors.

The pipeline:

1. loads a tokenizer JSON produced by the Hugging Face ecosystem,
2. encodes the prompt into `input_ids`, `attention_mask`, and optional `token_type_ids`,
3. runs the tensors through ONNX Runtime,
4. collects the first output tensor, and
5. (optionally) L2-normalizes the vector for cosine distance compatibility.

If the requested tier is `"fast"` or the model files are missing, the crate falls back to a
deterministic stub generator so the rest of the UCFP pipeline can keep flowing.

## Key Types

```rust
pub struct SemanticConfig {
    pub tier: String,
    pub model_name: String,
    pub model_path: PathBuf,
    pub model_url: Option<String>,
    pub tokenizer_path: Option<PathBuf>,
    pub tokenizer_url: Option<String>,
    pub normalize: bool,
    pub device: String,
}

pub struct SemanticEmbedding {
    pub doc_id: String,
    pub vector: Vec<f32>,
    pub model_name: String,
    pub tier: String,
    pub embedding_dim: usize,
    pub normalized: bool,
}

pub enum SemanticError {
    ModelNotFound(String),
    TokenizerMissing(String),
    InvalidConfig(String),
    Download(String),
    Io(std::io::Error),
    Inference(String),
}
```

`SemanticConfig::default()` expects the balanced tier of
`models/bge-small-en-v1.5/{onnx/model.onnx,tokenizer.json}` and normalizes the output vector.

## Public API

```rust
pub fn semanticize(
    doc_id: &str,
    text: &str,
    cfg: &SemanticConfig,
) -> Result<SemanticEmbedding, SemanticError>;
```

- `semanticize` orchestrates tokenizer loading, ONNX session creation, inference, and optional
  normalization. Unknown ONNX inputs produce a descriptive `SemanticError`.
- The fallback generator used by the `"fast"` tier is exposed internally through `make_stub_embedding`
  (useful for deterministic fixtures and integration tests).

## Configuration & Model Assets

- `tier`: `"fast"` uses the stub, `"balanced"` and `"accurate"` expect ONNX assets. You can plug in
  any compatible encoder by pointing `model_path` and `tokenizer_path` to the exported files.
- `model_name`: purely descriptive, echoed in the resulting `SemanticEmbedding`.
- `model_url` / `tokenizer_url`: optional HTTPS endpoints used to fetch files when the local paths
  are missing. Downloads are cached at the configured paths and reused on subsequent calls.
- `normalize`: set to `true` to produce unit-length vectors (recommended for cosine similarity).
- `device`: currently `"cpu"` only; future work can expose GPU providers once ONNX Runtime is built
  with CUDA support.

Drop your ONNX model under `models/<name>/onnx/<file>.onnx` alongside its tokenizer JSON; the new
integration test `test_real_model_inference` shows the expected layout using the BGE small encoder.

### Remote API mode

Set `mode` to `api` when routing inference through hosted services such as Hugging Face Inference Endpoints or OpenAI.
Populate `api_url` with the HTTPS endpoint, `api_auth_header` with the bearer/API key, and `api_provider` with `hf`, `openai`, or `custom` to pick the payload shape.
`semanticize_batch` automatically builds provider-specific batched payloads and maintains deterministic normalization.

```rust
use ufp_semantic::{semanticize, SemanticConfig};

let cfg = SemanticConfig {
    mode: "api".into(),
    api_url: Some("https://api-inference.huggingface.co/models/BAAI/bge-small-en-v1.5".into()),
    api_auth_header: Some("Bearer hf_xxx".into()),
    api_provider: Some("hf".into()),
    ..Default::default()
};
let embedding = semanticize("doc-42", "hello world", &cfg)?;
```

The deterministic stub tier still works in API mode: leave the URL empty and set `tier = "fast"` when you need offline smoke tests.

## Example

```rust
use ufp_semantic::{semanticize, SemanticConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = SemanticConfig {
        tier: "balanced".into(),
        ..Default::default()
    };

    let embedding = semanticize("doc-42", "hello world", &cfg)?;
    println!(
        "embedding[0..5]: {:?} (dim={}, normalized={})",
        &embedding.vector[..5.min(embedding.vector.len())],
        embedding.embedding_dim,
        embedding.normalized,
    );

    Ok(())
}
```

If the ONNX model is missing, the call will emit a console warning and fall back to the deterministic
stub so the API surface remains the same.

### Running the example

```
cargo run -p ufp_semantic --example embed -- "Doc Title" "Some text to embed"
```

The example automatically checks for `models/bge-small-en-v1.5/onnx/model.onnx`; when it
is not available it transparently uses the stub tier and annotates the output.

## Testing

```
cargo test -p ufp_semantic
cargo test -p ufp_semantic test_real_model_inference -- --ignored    # runs the ONNX-backed test
```

Unit tests cover deterministic stubs, while the ignored integration test exercises an actual ONNX
graph (enabled once the model assets are available locally).

## Integration

`SemanticEmbedding` is designed to feed the ingest + pipeline crates. Each embedding carries the
document id, tier, and normalization flag so downstream services (e.g., ANN indexes) can select the
right scorer or apply batching heuristics without re-reading configuration.
