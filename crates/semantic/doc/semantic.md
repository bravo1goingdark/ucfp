# UCFP Semantic Crate

> **Dense vector embeddings for the Universal Content Fingerprinting pipeline**

[![API Docs](https://img.shields.io/badge/docs-api-blue)](https://docs.rs/semantic)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)

## Table of Contents

- [Overview](#overview)
- [Quick Start](#quick-start)
- [Architecture](#architecture)
- [Core Concepts](#core-concepts)
- [Configuration](#configuration)
- [API Reference](#api-reference)
- [Error Handling](#error-handling)
- [Examples](#examples)
- [Best Practices](#best-practices)
- [Performance](#performance)
- [Troubleshooting](#troubleshooting)
- [Integration Guide](#integration-guide)
- [Testing](#testing)

---

## Overview

The `semantic` crate is part of the UCFP pipeline, transforming text into dense vector embeddings using transformer models packaged as ONNX graphs.

### What This Crate Does

| Function | Description |
|----------|-------------|
| **Embedding Generation** | Convert text to dense vectors using transformer models |
| **Multi-Tier Support** | Fast (stubs), balanced, accurate tiers |
| **Local Inference** | CPU-only ONNX Runtime by default |
| **Batch Processing** | Efficient batch embedding generation |
| **Resilience** | Circuit breaker, retry, rate limiting for API calls |

### Key Properties

- **Lightweight**: Tiny runtime, CPU-only by default
- **Cached**: Per-thread caching of tokenizers and ONNX sessions
- **Resilient**: Auto-fallback to stubs when models are missing
- **Normalized**: All embeddings normalized for consistent downstream search

### Pipeline Position

```
┌─────────┐     ┌──────────┐     ┌──────────────────┐     ┌───────┐     ┌───────┐
│  Ingest │────▶│Canonical │────▶│Perceptual/Semantic│────▶│ Index │────▶│ Match │
│         │     │          │     │      (this)      │     │       │     │       │
└─────────┘     └──────────┘     └──────────────────┘     └───────┘     └───────┘
```

---

## Quick Start

### Basic Embedding

```rust
use semantic::{semanticize, SemanticConfig};

let cfg = SemanticConfig {
    tier: "balanced".into(),
    ..Default::default()
};

let embedding = semanticize("doc-42", "hello world", &cfg)?;
println!("dim: {}, normalized: {}", embedding.embedding_dim, embedding.normalized);
```

### Using Fast Tier (for testing)

```rust
use semantic::{semanticize, SemanticConfig};

let cfg = SemanticConfig {
    mode: "fast".into(),
    ..Default::default()
};

let embedding = semanticize("doc-42", "hello world", &cfg)?;
// Returns deterministic stub vector
```

---

## Architecture

### Processing Pipeline

```
Input Text
    │
    ▼
┌─────────────────────────────────────────┐
│           Semantic Pipeline              │
├─────────────────────────────────────────┤
│  1. Tokenizer Loading                   │
│     - Load Hugging Face JSON tokenizer  │
│     - Per-thread caching                │
├─────────────────────────────────────────┤
│  2. Text Encoding                       │
│     - Encode text to input_ids          │
│     - Generate attention_mask           │
├─────────────────────────────────────────┤
│  3. ONNX Inference                      │
│     - Run tensors through ONNX Runtime  │
│     - Extract output tensor             │
├─────────────────────────────────────────┤
│  4. Pooling                             │
│     - Mean/WeightedMean/Max/First       │
│     - Handle chunking if enabled        │
├─────────────────────────────────────────┤
│  5. Normalization (Optional)            │
│     - L2 normalize for cosine distance │
└─────────────────────────────────────────┘
    │
    ▼
SemanticEmbedding
```

### Tier Modes

| Tier | Mode | Description |
|------|------|-------------|
| `fast` | `fast` | Deterministic stubs for testing |
| `balanced` | `onnx` | Local ONNX model, balanced performance |
| `accurate` | `onnx` | Local ONNX model, best quality |

---

## Core Concepts

### Embeddings

Dense vector representations of text that capture semantic meaning. Similar texts produce similar vectors.

### ONNX Runtime

Cross-platform inference engine running transformer models locally without API calls.

### Pooling Strategies

How to combine token embeddings into a single vector:

- **Mean**: Simple average of all token embeddings
- **WeightedMean**: Center chunks weighted more heavily (default)
- **Max**: Element-wise maximum
- **First**: Use first token (CLS) embedding

### Chunking

For documents exceeding model's token limit:
1. Split into overlapping chunks
2. Embed each chunk separately
3. Pool together

---

## Configuration

### SemanticConfig

Central configuration struct controlling semantic embedding:

```rust
pub struct SemanticConfig {
    // Tier and mode
    pub tier: String,           // "fast", "balanced", "accurate"
    pub mode: String,           // "onnx", "api", "fast"
    
    // Model configuration
    pub model_name: String,
    pub model_path: PathBuf,
    pub model_url: Option<String>,
    pub tokenizer_path: Option<PathBuf>,
    pub tokenizer_url: Option<String>,
    
    // Embedding settings
    pub normalize: bool,         // L2 normalize output
    pub device: String,          // "cpu" for now
    
    // Model-specific
    pub max_sequence_length: usize,
    pub enable_chunking: bool,
    pub chunk_overlap_ratio: f32,
    pub pooling_strategy: String,
    
    // API settings
    pub api_url: Option<String>,
    pub api_auth_header: Option<String>,
    pub api_provider: Option<String>,
    pub api_timeout_secs: Option<u64>,
    
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
```

### Configuration Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `tier` | String | "balanced" | "fast", "balanced", "accurate" |
| `mode` | String | "onnx" | "onnx", "api", "fast" |
| `model_name` | String | "bge-small-en-v1.5" | Model identifier |
| `model_path` | PathBuf | (see below) | Local ONNX file path |
| `normalize` | bool | true | L2 normalize output |
| `device` | String | "cpu" | Compute device |
| `max_sequence_length` | usize | 512 | Model token limit |
| `enable_chunking` | bool | false | Enable sliding window |
| `pooling_strategy` | String | "weighted_mean" | Pooling method |

### Default Paths

```
models/bge-small-en-v1.5/onnx/model.onnx
models/bge-small-en-v1.5/tokenizer.json
```

### Hugging Face URL Format

When downloading models from Hugging Face, use `/resolve/` instead of `/blob/`:

```rust
// Correct: /resolve/ returns raw file content
model_url: Some("https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/main/onnx/model.onnx".into()),
tokenizer_url: Some("https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/main/tokenizer.json".into()),

// Wrong: /blob/ returns HTML page (won't work for downloading)
model_url: Some("https://huggingface.co/BAAI/bge-small-en-v1.5/blob/main/onnx/model.onnx".into()),
```

### Configuration Validation

```rust
fn main() {
    let cfg = load_semantic_config();
    if let Err(e) = cfg.validate() {
        eprintln!("Invalid semantic configuration: {}", e);
        std::process::exit(1);
    }
}
```

---

## API Reference

### Main Functions

#### `semanticize()`

```rust
pub fn semanticize(
    doc_id: &str,
    text: &str,
    cfg: &SemanticConfig,
) -> Result<SemanticEmbedding, SemanticError>
```

Generate embedding for a single document.

**Parameters:**
- `doc_id`: Document identifier
- `text`: Input text to embed
- `cfg`: Configuration struct

**Returns:**
- `Ok(SemanticEmbedding)`: Vector embedding
- `Err(SemanticError)`: Various error types

#### `semanticize_batch()`

```rust
pub fn semanticize_batch<D, T>(
    docs: &[(D, T)],
    cfg: &SemanticConfig,
) -> Result<Vec<SemanticEmbedding>, SemanticError>
```

Generate embeddings for multiple documents.

### Utility Functions

#### `make_stub_embedding()`

```rust
pub fn make_stub_embedding(doc_id: &str, cfg: &SemanticConfig) -> SemanticEmbedding
```

Generate deterministic stub embedding for testing.

### Health Checks

```rust
pub fn get_circuit_breaker_stats(provider: &str) -> CircuitBreakerStats;
pub fn is_provider_healthy(provider: &str) -> bool;
```

### Data Types

See the [types module](src/types.rs) for complete definitions of:
- `SemanticConfig` - Configuration struct
- `SemanticEmbedding` - Output structure
- `SemanticError` - Error variants

---

## Error Handling

### SemanticError Variants

| Error | Trigger | Recovery |
|-------|---------|----------|
| `ModelNotFound(msg)` | ONNX file missing | Add model file or use fast tier |
| `TokenizerMissing(msg)` | Tokenizer JSON missing | Add tokenizer or use fast tier |
| `InvalidConfig(msg)` | Bad configuration | Fix configuration |
| `Download(msg)` | Model download failed | Check network/model URL |
| `Io(err)` | File I/O error | Check file permissions |
| `Inference(msg)` | ONNX runtime error | Check model compatibility |

### Error Handling Patterns

**Pattern 1: Graceful Fallback**

```rust
use semantic::{semanticize, SemanticError};

match semanticize(doc_id, text, &cfg) {
    Ok(embedding) => {
        tracing::info!(
            doc_id = %doc_id,
            dim = embedding.embedding_dim,
            "embedding_success"
        );
    }
    Err(SemanticError::ModelNotFound(_) | SemanticError::TokenizerMissing(_)) => {
        tracing::warn!("Model missing, falling back to fast tier");
        // Use fast tier instead
    }
    Err(e) => {
        tracing::error!(error = %e, "embedding_failed");
    }
}
```

**Pattern 2: Validate Configuration**

```rust
let cfg = SemanticConfig::default();
if let Err(e) = cfg.validate() {
    return Err(format!("Invalid config: {}", e));
}
```

---

## Examples

### Example 1: Basic Embedding

```rust
use semantic::{semanticize, SemanticConfig};

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
```

### Example 2: Fast Tier (Testing)

```rust
use semantic::{semanticize, SemanticConfig};

let cfg = SemanticConfig {
    mode: "fast".into(),
    ..Default::default()
};

let embedding = semanticize("doc-42", "hello world", &cfg)?;
// Returns deterministic stub vector
```

### Example 3: API Mode (Hugging Face)

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

### Example 4: Chunking Long Documents

```rust
use semantic::{semanticize, SemanticConfig};

let cfg = SemanticConfig {
    enable_chunking: true,
    max_sequence_length: 512,
    chunk_overlap_ratio: 0.5,
    pooling_strategy: "weighted_mean".into(),
    ..Default::default()
};

let long_text = "..."; // Long document
let embedding = semanticize("doc-long", long_text, &cfg)?;
```

### Example 5: Batch Processing

```rust
use semantic::{semanticize_batch, SemanticConfig};

let docs = vec![
    ("doc-1", "first text"),
    ("doc-2", "second text"),
    ("doc-3", "third text"),
];

let cfg = SemanticConfig::default();
let embeddings = semanticize_batch(&docs, &cfg)?;
```

### Example 6: Resilience Configuration

```rust
use semantic::SemanticConfig;

let cfg = SemanticConfig {
    mode: "api".into(),
    enable_resilience: true,
    circuit_breaker_failure_threshold: 5,
    circuit_breaker_reset_timeout_secs: 30,
    retry_max_retries: 3,
    retry_base_delay_ms: 100,
    retry_max_delay_ms: 10000,
    retry_jitter: true,
    rate_limit_requests_per_second: 10.0,
    rate_limit_burst_size: 5,
    ..Default::default()
};
```

---

## Best Practices

### 1. Use Fast Tier for Testing

```rust
let cfg = SemanticConfig {
    mode: "fast".into(),
    ..Default::default()
};
// Deterministic stubs for unit tests
```

### 2. Enable Normalization

```rust
let cfg = SemanticConfig {
    normalize: true,  // Essential for cosine similarity
    ..Default::default()
};
```

### 3. Cache Configuration

```rust
lazy_static::lazy_static! {
    static ref SEMANTIC_CONFIG: SemanticConfig = SemanticConfig {
        tier: "balanced".into(),
        normalize: true,
        ..Default::default()
    };
}
```

### 4. Handle Missing Models

```rust
match semanticize(doc_id, text, &cfg) {
    Ok(embedding) => process(embedding),
    Err(SemanticError::ModelNotFound(_) | SemanticError::TokenizerMissing(_)) => {
        // Fall back to fast tier
        let fast_cfg = SemanticConfig { mode: "fast".into(), ..Default::default() };
        semanticize(doc_id, text, &fast_cfg)
    }
    Err(e) => Err(e),
}
```

### 5. Enable Resilience for API Mode

```rust
let cfg = SemanticConfig {
    mode: "api".into(),
    enable_resilience: true,
    ..Default::default()
};
```

---

## Performance

### Benchmarks

Typical performance on modern hardware:

| Mode | Latency | Throughput |
|------|---------|------------|
| Fast (stubs) | ~1μs | 1M ops/sec |
| ONNX (small) | ~10ms | 100 ops/sec |
| ONNX (batch) | ~50ms | 20 ops/sec |
| API | ~100ms | 10 ops/sec |

### Memory Usage

| Component | Memory |
|-----------|--------|
| ONNX session | ~100-500MB |
| Tokenizer | ~10MB |
| Embedding vector | ~4KB (for 512-dim) |

### Optimization Tips

1. **Enable batching** - Use `semanticize_batch` for multiple documents
2. **Cache ONNX sessions** - Already cached per-thread automatically
3. **Disable chunking** - If documents fit in token limit
4. **Use appropriate tier** - Fast for tests, balanced for production

---

## Troubleshooting

### Common Issues

#### "ModelNotFound" Error

**Problem**: ONNX model file not found.

**Solutions:**
```rust
// Option 1: Use fast tier (stub embedding)
let cfg = SemanticConfig { mode: "fast".into(), ..Default::default() };

// Option 2: Provide model path
let cfg = SemanticConfig {
    model_path: PathBuf::from("models/bge-small-en-v1.5/onnx/model.onnx"),
    ..Default::default()
};

// Option 3: Auto-download
// Note: Use /resolve/ not /blob/ for direct file downloads
let cfg = SemanticConfig {
    model_url: Some("https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/main/onnx/model.onnx".into()),
    tokenizer_url: Some("https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/main/tokenizer.json".into()),
    ..Default::default()
};
```

#### "TokenizerMissing" Error

**Problem**: Tokenizer JSON not found.

**Solutions:**
```rust
// Provide tokenizer path
let cfg = SemanticConfig {
    tokenizer_path: Some(PathBuf::from("models/bge-small-en-v1.5/tokenizer.json")),
    ..Default::default()
};
```

#### API Errors

**Problem**: API calls failing.

**Solutions:**
1. Check API key and URL
2. Enable resilience features
3. Check rate limits

```rust
let cfg = SemanticConfig {
    mode: "api".into(),
    enable_resilience: true,
    rate_limit_requests_per_second: 5.0,
    ..Default::default()
};
```

#### Slow Embedding Generation

**Problem**: Embedding is slow.

**Solutions:**
1. Use ONNX mode instead of API
2. Enable chunking only when needed
3. Use appropriate pooling strategy

---

## Integration Guide

### With the UCFP Pipeline

```rust
use ingest::{ingest, CanonicalPayload};
use canonical::canonicalize;
use semantic::semanticize;

// 1. Ingest
let canonical_record = ingest(raw_record, &ingest_cfg)?;

// 2. Extract text and canonicalize
if let Some(CanonicalPayload::Text(text)) = canonical_record.normalized_payload {
    let canonical = canonicalize(&canonical_record.doc_id, &text, &canonical_cfg)?;
    
    // 3. Generate embedding
    let embedding = semanticize(&canonical.doc_id, &canonical.canonical_text, &semantic_cfg)?;
    
    // 4. Index...
}
```

### Custom Pooling

```rust
use semantic::SemanticConfig;

let cfg = SemanticConfig {
    pooling_strategy: "mean".into(),  // Simple mean pooling
    ..Default::default()
};
```

### Health Monitoring

```rust
use semantic::{get_circuit_breaker_stats, is_provider_healthy};

// Check provider health
if is_provider_healthy("hf") {
    // Use Hugging Face API
}

// Get detailed stats
let stats = get_circuit_breaker_stats("hf");
println!("State: {:?}", stats.state);
```

---

## Testing

### Running Tests

```bash
# Run all tests
cargo test -p semantic

# Run with output
cargo test -p semantic -- --nocapture

# Run ONNX test (requires models)
cargo test -p semantic test_real_model_inference -- --ignored
```

### Test Coverage

Unit tests cover:
- Stub embedding generation
- Configuration validation
- Tokenizer handling
- Error cases
- Resilience features (when enabled)

### Example Programs

```bash
# Basic embedding
cargo run -p semantic --example embed -- "Doc Title" "Some text to embed"
```

---

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](../../LICENSE) for details.

---

## Contributing

Contributions are welcome! Please ensure:
- All tests pass: `cargo test -p semantic`
- Documentation is updated
- Examples are provided for new features

---

## Support

For issues and questions:
- GitHub Issues: [github.com/bravo1goingdark/ufcp/issues](https://github.com/bravo1goingdark/ufcp/issues)
- Documentation: [docs.rs/semantic](https://docs.rs/semantic)
