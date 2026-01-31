# UCFP Semantic

## Purpose

`semantic` converts canonicalized text into dense vector embeddings using transformer models
packaged as ONNX graphs. The crate keeps runtime requirements tiny (CPU-only by default), caches
tokenizers and ONNX sessions per thread for low-latency reuse, provides stubbed outputs for offline
or "fast" tiers (plus automatic fallback when model assets are missing), and normalizes every
embedding so downstream search and similarity components receive consistent vectors.

The pipeline:

1. loads a tokenizer JSON produced by the Hugging Face ecosystem,
2. encodes the prompt into `input_ids`, `attention_mask`, and optional `token_type_ids`,
3. runs the tensors through ONNX Runtime,
4. collects the first output tensor, and
5. (optionally) L2-normalizes the vector for cosine distance compatibility.

Setting `tier` or `mode` to `"fast"` returns a deterministic stub generator for offline testing.
Other tiers expect ONNX assets to exist locally or be downloadable through the configured
`model_url` / `tokenizer_url`; when those artifacts are missing (or temporarily unreachable) the
crate automatically falls back to the deterministic stub so pipelines keep running. Fatal
`SemanticError`s only arise from misconfiguration (e.g., unknown inputs, invalid URLs, etc.).

## Key Types

```rust
pub struct SemanticConfig {
    pub tier: String,
    pub mode: String,
    pub model_name: String,
    pub model_path: PathBuf,
    pub model_url: Option<String>,
    pub api_url: Option<String>,
    pub api_auth_header: Option<String>,
    pub api_provider: Option<String>,
    pub api_timeout_secs: Option<u64>,
    pub tokenizer_path: Option<PathBuf>,
    pub tokenizer_url: Option<String>,
    pub normalize: bool,
    pub device: String,
    // Resilience configuration
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

`SemanticConfig::default()` expects the balanced tier of
`models/bge-small-en-v1.5/{onnx/model.onnx,tokenizer.json}` and normalizes the output vector.

## Public API

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

- `semanticize` orchestrates tokenizer resolution, cached ONNX session creation/reuse, inference,
  and optional normalization. Unknown ONNX inputs still produce a descriptive `SemanticError`.
- `semanticize_batch` runs a single batched ONNX request or batch-capable API call for higher
  throughput. The outputs are normalized and labeled in the same way as the single-call API.
- The fallback generator used by the `"fast"` tier is exposed internally through `make_stub_embedding`
  (useful for deterministic fixtures and integration tests).

## Module Layout

- `config.rs`: `SemanticConfig` defaults and configuration fields.
- `types.rs`: `SemanticEmbedding` metadata payload.
- `error.rs`: `SemanticError` error taxonomy.
- `api.rs`: API payload shaping and response parsing.
- `assets.rs`: model/tokenizer resolution and downloads.
- `cache.rs`: thread-local tokenizer + ONNX session caching.
- `onnx.rs`: tokenization + ONNX runtime execution.
- `normalize.rs`: L2 normalization helpers.
- `stub.rs`: deterministic stub vector generation.
- `resilience.rs`: circuit breaker, retry, and rate limiting for API calls.

## Resilience & Production Features

The `semantic` crate provides built-in resilience patterns for production deployments using external API providers (OpenAI, Hugging Face, etc.):

### Circuit Breaker

Prevents cascading failures when embedding services are unavailable:

- `circuit_breaker_failure_threshold`: Number of consecutive failures before opening the circuit (default: 5)
- `circuit_breaker_reset_timeout_secs`: Time to wait before attempting to close the circuit (default: 30)

When the circuit is open, requests fail fast without hitting the API, allowing the service to recover.

### Retry with Exponential Backoff

Handles transient failures gracefully:

- `retry_max_retries`: Maximum number of retry attempts (default: 3)
- `retry_base_delay_ms`: Initial delay before first retry in milliseconds (default: 100)
- `retry_max_delay_ms`: Maximum delay between retries in milliseconds (default: 10000)
- `retry_jitter`: Add random jitter to prevent thundering herd (default: true)

Retries are automatically skipped when the circuit breaker is open.

### Rate Limiting

Protects downstream services and manages costs using a token bucket algorithm:

- `rate_limit_requests_per_second`: Sustained request rate per provider (default: 10.0)
- `rate_limit_burst_size`: Maximum burst size for short spikes (default: 5)

Rate limits are enforced per provider, allowing independent policies for different embedding services.

### Enabling/Disabling Resilience

Set `enable_resilience: true` in `SemanticConfig` to activate all resilience features. When disabled, API calls proceed without circuit breaker, retry, or rate limiting protection.

### Monitoring APIs

```rust
// Get circuit breaker statistics for a provider
pub fn get_circuit_breaker_stats(provider: &str) -> CircuitBreakerStats;

// Check if a provider is healthy
pub fn is_provider_healthy(provider: &str) -> bool;
```

`CircuitBreakerStats` provides:
- Current state (Closed, Open, HalfOpen)
- Failure count
- Success count
- Last failure time
- State change history

### Example

```rust
use semantic::{semanticize, SemanticConfig};

let cfg = SemanticConfig {
    mode: "api".into(),
    api_url: Some("https://api.openai.com/v1/embeddings".into()),
    api_auth_header: Some("Bearer sk-xxx".into()),
    api_provider: Some("openai".into()),
    // Enable resilience features
    enable_resilience: true,
    // Circuit breaker: open after 5 failures, reset after 60 seconds
    circuit_breaker_failure_threshold: 5,
    circuit_breaker_reset_timeout_secs: 60,
    // Retry: up to 3 retries with exponential backoff
    retry_max_retries: 3,
    retry_base_delay_ms: 100,
    retry_max_delay_ms: 5000,
    retry_jitter: true,
    // Rate limit: 5 req/s with burst of 10
    rate_limit_requests_per_second: 5.0,
    rate_limit_burst_size: 10,
    ..Default::default()
};

// Check provider health before making requests
if semantic::is_provider_healthy("openai") {
    let embedding = semanticize("doc-42", "hello world", &cfg)?;
}

// Monitor circuit breaker stats
let stats = semantic::get_circuit_breaker_stats("openai");
println!("Circuit state: {:?}", stats.state);
```

## Configuration & Model Assets

- `tier`: `"fast"` produces stub embeddings while `"balanced"` / `"accurate"` drive the ONNX or API
  paths. Set the tier to match your deployment policy.
- `mode`: `"onnx"` (default) runs local ONNX Runtime inference, `"api"` targets hosted HTTP
  providers, and `"fast"` always emits the deterministic stub without touching models.
- `model_name`: purely descriptive, echoed in the resulting `SemanticEmbedding`.
- `model_path`: local filesystem path to the ONNX export. Provide `model_url` (and `tokenizer_url`)
  when you want the crate to download assets on demand; when assets are missing or the download
  fails the crate falls back to the deterministic stub instead of erroring out.
- `api_url` / `api_auth_header` / `api_provider` / `api_timeout_secs`: configure remote inference.
  API mode always requires `api_url`; the other fields are optional helpers for authentication and
  request shaping.
- `tokenizer_path`: defaults to a sibling path next to `model_path`. Override it when you keep
  assets elsewhere.
- `normalize`: set to `true` to produce unit-length vectors (recommended for cosine similarity).
- `device`: currently `"cpu"` only; future work can expose GPU providers once ONNX Runtime is built
  with CUDA support.

### Resilience Configuration (API mode)

- `enable_resilience`: master switch for all resilience features (default: `false`).
- `circuit_breaker_failure_threshold`: failures before opening circuit (default: 5).
- `circuit_breaker_reset_timeout_secs`: seconds before attempting reset (default: 30).
- `retry_max_retries`: maximum retry attempts for transient failures (default: 3).
- `retry_base_delay_ms`: initial retry delay in milliseconds (default: 100).
- `retry_max_delay_ms`: maximum retry delay in milliseconds (default: 10000).
- `retry_jitter`: add randomization to retry delays (default: true).
- `rate_limit_requests_per_second`: sustained rate limit per provider (default: 10.0).
- `rate_limit_burst_size`: burst allowance for short spikes (default: 5).

Drop your ONNX model under `models/<name>/onnx/<file>.onnx` alongside its tokenizer JSON; the new
integration test `test_real_model_inference` shows the expected layout using the BGE small encoder.
Once a session loads successfully, it is cached within the thread so subsequent calls reuse the same
tokenizer and ONNX graph without paying I/O or compilation costs.

### Remote API mode

Set `mode` to `api` when routing inference through hosted services such as Hugging
Face Inference Endpoints or OpenAI. Populate `api_url` with the HTTPS endpoint,
`api_auth_header` with the bearer/API key, and `api_provider` with `hf`, `openai`, or `custom` to
pick the payload shape. `api_timeout_secs` controls the HTTP deadline. `semanticize_batch`
automatically builds provider-specific batched payloads and maintains deterministic normalization.

```rust
use semantic::{semanticize, SemanticConfig};

let cfg = SemanticConfig {
    mode: "api".into(),
    api_url: Some("https://api-inference.huggingface.co/models/BAAI/bge-small-en-v1.5".into()),
    api_auth_header: Some("Bearer hf_xxx".into()),
    api_provider: Some("hf".into()),
    ..Default::default()
};
let embedding = semanticize("doc-42", "hello world", &cfg)?;
```

Need stub behavior? Skip API mode entirely and set `mode = "fast"` (or `tier = "fast"`) so the
deterministic generator returns immediately. When `mode = "onnx"` but models are missing, the stub
is used automatically.

## Example

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

If the ONNX model or tokenizer is missing and no download URL is configured, the call still
completes by falling back to the deterministic stub. Configure `model_url` + `tokenizer_url` when
you want automatic downloading, or set `mode = "fast"` to force stub behavior explicitly.

### Running the example

```
cargo run -p semantic --example embed -- "Doc Title" "Some text to embed"
```

The example expects `models/bge-small-en-v1.5/onnx/model.onnx` plus its tokenizer JSON to exist (or
to be downloadable via the optional URLs). Without those assets the program exits with a descriptive
`SemanticError`, prompting you to either provide the files or use stub mode.

## Testing

```
cargo test -p semantic
cargo test -p semantic test_real_model_inference -- --ignored    # runs the ONNX-backed test
```

Unit tests cover deterministic stubs, while the ignored integration test exercises an actual ONNX
graph (enabled once the model assets are available locally).

## Integration

`SemanticEmbedding` is designed to feed the ingest + pipeline crates. Each embedding carries the
document id, tier, and normalization flag so downstream services (e.g., ANN indexes) can select the
right scorer or apply batching heuristics without re-reading configuration. The cached ONNX sessions
and batched inputs used by `semanticize_batch` keep steady-state pipelines efficient even under high
load.
