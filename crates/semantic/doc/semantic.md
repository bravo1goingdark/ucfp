# UCFP Semantic

## What this does

The semantic crate turns text into dense vector embeddings using transformer models packaged as ONNX graphs. Here's what you're getting:

- Tiny runtime (CPU only by default)
- Per-thread caching of tokenizers and ONNX sessions for low latency
- Stub outputs for testing or "fast" tier (plus auto-fallback when models are missing)
- Every embedding gets normalized so downstream search works consistently

## How it works

1. Load a tokenizer (Hugging Face JSON format)
2. Encode text into input_ids and attention_mask
3. Run tensors through ONNX Runtime
4. Grab the first output tensor
5. (Optional) L2-normalize for cosine distance

Set `tier` or `mode` to `"fast"` for deterministic stubs in tests. Other tiers need ONNX assets locally or through `model_url`/`tokenizer_url`. Missing assets? We fall back to stubs instead of crashing. Real errors only come from bad config.

## Key Types

```rust
pub struct SemanticConfig {
    pub tier: String,         // "fast", "balanced", "accurate"
    pub mode: String,         // "onnx", "api", "fast"
    pub model_name: String,
    pub model_path: PathBuf,
    pub model_url: Option<String>,
    pub api_url: Option<String>,
    pub api_auth_header: Option<String>,
    pub api_provider: Option<String>,  // "hf", "openai", "custom"
    pub api_timeout_secs: Option<u64>,
    pub tokenizer_path: Option<PathBuf>,
    pub tokenizer_url: Option<String>,
    pub normalize: bool,      // L2 normalize output?
    pub device: String,        // "cpu" for now

    // Model-specific stuff
    pub max_sequence_length: usize,   // model's token limit
    pub enable_chunking: bool,         // sliding window for long docs
    pub chunk_overlap_ratio: f32,     // 0.0-1.0, default 0.5
    pub pooling_strategy: String,      // "mean", "weighted_mean", "max", "first"

    // Resilience settings
    pub enable_resilience: bool,
    pub circuit_breaker_failure_threshold: u32,
    pub circuit_breaker_reset_timeout_secs: u64,
    pub retry_max_retries: u32,
    pub retry_base_delay_ms: u64,
    pub retry_max_delay_ms: u64,
    pub retry_jitter: bool,
    pub rate_limit_requests_per_second: f64,
    pub rate_limit_burst_size: u32,
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

`SemanticConfig::default()` uses the "balanced" tier with `models/bge-small-en-v1.5/{onnx/model.onnx,tokenizer.json}` and normalizes output.

## Main API

```rust
pub fn semanticize(
    doc_id: &str,
    text: &str,
    cfg: &SemanticConfig,
) -> Result<SemanticEmbedding, SemanticError>;

pub fn semanticize_batch<D, T>(
    docs: &[(D, T)],
    cfg: &SemanticConfig,
) -> Result<Vec<SemanticEmbedding>, SemanticError>;
```

- `semanticize` - handles tokenizer loading, ONNX session reuse, inference, normalization
- `semanticize_batch` - batches requests for better throughput
- Stub generator is available via `make_stub_embedding` for tests

## Module structure

| File | What it does |
|------|--------------|
| `config.rs` | Config and defaults |
| `types.rs` | Embedding type |
| `error.rs` | Error types |
| `api.rs` | API request/response handling |
| `assets.rs` | Model/tokenizer downloads |
| `cache.rs` | Thread-local caching |
| `onnx.rs` | Tokenization + inference |
| `normalize.rs` | L2 normalization |
| `stub.rs` | Stub vector generation |
| `resilience.rs` | Circuit breaker, retry, rate limit |

## Resilience features

We include production-grade resilience for API calls (OpenAI, Hugging Face, etc.):

### Circuit Breaker

Prevents cascading failures:

- `circuit_breaker_failure_threshold`: consecutive failures before opening (default: 5)
- `circuit_breaker_reset_timeout_secs`: seconds before trying again (default: 30)

Open circuit = fail fast, don't hammer the API.

### Retry with Exponential Backoff

Handles transient issues:

- `retry_max_retries`: max attempts (default: 3)
- `retry_base_delay_ms`: initial delay (default: 100ms)
- `retry_max_delay_ms`: max delay (default: 10s)
- `retry_jitter`: add randomness to prevent thundering herd (default: true)

Skipped when circuit is open.

### Rate Limiting

Token bucket algorithm:

- `rate_limit_requests_per_second`: sustained rate (default: 10.0)
- `rate_limit_burst_size`: spike allowance (default: 5)

Per-provider, so you can have different limits for different services.

### Enabling resilience

Set `enable_resilience: true` in `SemanticConfig`.

### Health checks

```rust
pub fn get_circuit_breaker_stats(provider: &str) -> CircuitBreakerStats;
pub fn is_provider_healthy(provider: &str) -> bool;
```

`CircuitBreakerStats` gives you state (Closed/Open/HalfOpen), failure/success counts, last failure time, and state change history.

## Configuration options

- `tier` - "fast" = stubs, "balanced"/"accurate" = real models
- `mode` - "onnx" (local, default), "api" (remote), or "fast" (stubs)
- `model_name` - just for logging/display
- `model_path` - local ONNX file. Add `model_url` if you want auto-downloads. Missing? We'll use stubs.
- `api_url`, `api_auth_header`, `api_provider`, `api_timeout_secs` - for remote calls. `api_url` is required.
- `tokenizer_path` - defaults to same dir as model_path
- `normalize` - set true for unit vectors (good for cosine similarity)
- `device` - "cpu" for now. GPU later maybe.

### Resilience settings (API mode)

- `enable_resilience` - master switch (default: false)
- `circuit_breaker_failure_threshold` - failures before opening (5)
- `circuit_breaker_reset_timeout_secs` - seconds before retrying (30)
- `retry_max_retries` - max retry attempts (3)
- `retry_base_delay_ms` - first retry delay (100ms)
- `retry_max_delay_ms` - max delay between retries (10s)
- `retry_jitter` - add randomness? (true)
- `rate_limit_requests_per_second` - sustained rate (10.0)
- `rate_limit_burst_size` - spike allowance (5)

### Chunking long documents

When text exceeds the model's token limit:

- `max_sequence_length` - model's max tokens (512 for BERT, 4096 for Longformer, etc.)
- `enable_chunking` - turn on sliding window? (default: false)
- `chunk_overlap_ratio` - 0.5 = 50% overlap (chunk 2 starts at 256 when chunk size is 512)
- `pooling_strategy` - how to combine chunk embeddings:
  - `"mean"` - simple average
  - `"weighted_mean"` - center chunks get more weight (default)
  - `"max"` - element-wise max
  - `"first"` - just use the first chunk

### How chunking works

1. Split long text into overlapping chunks
2. Embed each chunk separately
3. Pool them together
4. Return one embedding for the whole document

**Speed tradeoff:** chunking means N inference calls for N chunks. A 1000-word doc with 512-token chunks needs ~3-4 calls.

Drop ONNX models in `models/<name>/onnx/<file>.onnx` alongside the tokenizer JSON. Check `test_real_model_inference` for the expected layout. Sessions cache per-thread after first load.

### API mode

Set `mode = "api"` for hosted services (Hugging Face, OpenAI, etc.). Configure:
- `api_url` - HTTPS endpoint
- `api_auth_header` - your bearer/API key
- `api_provider` - "hf", "openai", or "custom"
- `api_timeout_secs` - HTTP timeout

```rust
use semantic::{semanticize, SemanticConfig};

let cfg = SemanticConfig {
    mode: "api".into(),
    api_url: Some("https://router.huggingface.co/hf-inference/models/BAAI/bge-small-en-v1.5/pipeline/feature-extraction".into()),
    api_auth_header: Some("Bearer hf_xxx".into()),
    api_provider: Some("hf".into()),
    ..Default::default()
};
let embedding = semanticize("doc-42", "hello world", &cfg)?;
```

Want stubs? Use `mode = "fast"` or `tier = "fast"`. Missing ONNX assets with `mode = "onnx"`? Stubs again.

## Quick example

```rust
use semantic::{semanticize, SemanticConfig};

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

Missing model/tokenizer? We fall back to stubs. Set `model_url` + `tokenizer_url` for auto-download, or `mode = "fast"` to force stubs.

### Run it

```
cargo run -p semantic --example embed -- "Doc Title" "Some text to embed"
```

Needs `models/bge-small-en-v1.5/onnx/model.onnx` + tokenizer JSON, or configurable URLs. Missing assets = error telling you what to fix.

## Tests

```
cargo test -p semantic
cargo test -p semantic test_real_model_inference -- --ignored    # ONNX test
```

Unit tests cover stubs. Ignored test runs actual ONNX (enable when you have models).

## Integration

`SemanticEmbedding` feeds ingest + pipeline crates. Each embedding carries doc_id, tier, and normalization flag. Cached ONNX sessions + batched inputs keep pipelines fast under load.
