# Universal Content Fingerprinting (UCFP) — Complete Usage Guide

> Deterministic, reproducible content fingerprints for text, combining exact hashing, perceptual similarity, and semantic embeddings into a single pipeline.

---

## Table of Contents

- [Why UCFP?](#why-ucfp)
- [Installation](#installation)
- [Quick Start](#quick-start)
  - [Canonicalization Only](#canonicalization-only)
  - [With Perceptual Fingerprint](#with-perceptual-fingerprint)
  - [With Semantic Embedding](#with-semantic-embedding)
  - [Both Perceptual and Semantic](#both-perceptual-and-semantic)
- [Pipeline Architecture](#pipeline-architecture)
  - [Ingest Stage](#ingest-stage)
  - [Canonical Stage](#canonical-stage)
  - [Perceptual Stage](#perceptual-stage)
  - [Semantic Stage](#semantic-stage)
- [Index and Search](#index-and-search)
  - [Index Configuration](#index-configuration)
  - [Upserting Records](#upserting-records)
  - [Query Modes](#query-modes)
  - [Matcher API](#matcher-api)
- [Observability](#observability)
  - [Metrics](#metrics)
  - [Structured Logging](#structured-logging)
- [Configuration](#configuration)
  - [YAML Configuration](#yaml-configuration)
  - [Programmatic Configuration](#programmatic-configuration)
- [REST API](#rest-api)
- [Performance](#performance)
- [Feature Flags](#feature-flags)
- [Roadmap](#roadmap)

---

## Why UCFP?

| Feature | UCFP |Traditional Hash | Semantic-only |
|---------|------|-----------------|---------------|
| Exact matching | SHA-256 canonical hash | Yes | No |
| Near-duplicate detection | MinHash LSH | No | Partial |
| Semantic similarity | Dense embeddings | No | Yes |
| Deterministic output | Yes | Yes | No |
| Single pipeline | All three | Hash only | Embedding only |

UCFP unifies **exact hashing**, **perceptual similarity**, and **semantic embeddings** into one deterministic pipeline:

- **Deduplication** — Find exact and near-duplicate content
- **Plagiarism Detection** — Identify paraphrased text  
- **Content Provenance** — Track content across systems
- **Similarity Search** — Search by meaning, not just keywords

---

## Installation

```toml
[dependencies]
ucfp = "0.1"
ucfp_index = "0.1"
ucfp_matcher = "0.1"
```

UCFP is a workspace with multiple crates:

| Crate | Purpose |
|-------|---------|
| `ucfp` | Core pipeline: ingest, canonical, perceptual, semantic |
| `ucfp_index` | Storage and ANN search with pluggable backends |
| `ucfp_matcher` | Query-time matching combining multiple signals |

---

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
ucfp = { path = "crates/lib" }
ucfp_index = { path = "crates/index" }
ucfp_matcher = { path = "crates/matcher" }
```

### Canonicalization Only

Process a record through just ingest + canonicalization (SHA-256 hashing + Unicode normalization):

```rust
use ucfp::{
    process_pipeline, PipelineStageConfig, 
    IngestConfig, CanonicalizeConfig, IngestMetadata, IngestPayload, IngestSource,
};

let ingest_cfg = IngestConfig::default();
let canonical_cfg = CanonicalizeConfig::default();

let raw = RawIngestRecord {
    id: "doc-001".into(),
    source: IngestSource::RawText,
    metadata: IngestMetadata {
        tenant_id: Some("tenant-a".into()),
        doc_id: Some("my-doc".into()),
        ..Default::default()
    },
    payload: Some(IngestPayload::Text("Hello, World!".into())),
};

let (doc, _, _) = process_pipeline(
    raw,
    PipelineStageConfig::Canonical,
    &ingest_cfg,
    &canonical_cfg,
    None,
    None,
)?;

println!("SHA-256: {}", doc.sha256_hex);
println!("Canonicalized: {}", doc.canonical_text);  // "hello world"
```

### With Perceptual Fingerprint

Add MinHash-based perceptual fingerprinting for near-duplicate detection:

```rust
use ucfp::{
    process_pipeline, PipelineStageConfig, 
    IngestConfig, CanonicalizeConfig, PerceptualConfig,
    IngestMetadata, IngestPayload, IngestSource,
};

let ingest_cfg = IngestConfig::default();
let canonical_cfg = CanonicalizeConfig::default();
let perceptual_cfg = PerceptualConfig::default();  // k=9, w=4 by default

let raw = RawIngestRecord { /* ... */ };

let (doc, fingerprint, _) = process_pipeline(
    raw,
    PipelineStageConfig::Perceptual,
    &ingest_cfg,
    &canonical_cfg,
    Some(&perceptual_cfg),
    None,
)?;

let fp = fingerprint.unwrap();
println!("MinHash bands: {}", fp.minhash_bands.len());
println!("Shingles: {}", fp.shingles.len());
```

### With Semantic Embedding

Add dense embeddings for meaning-based similarity:

```rust
use ucfp::{
    process_pipeline, PipelineStageConfig,
    IngestConfig, CanonicalizeConfig, SemanticConfig,
    IngestMetadata, IngestPayload, IngestSource,
};

let ingest_cfg = IngestConfig::default();
let canonical_cfg = CanonicalizeConfig::default();
let semantic_cfg = SemanticConfig {
    tier: "fast".into(),
    mode: "onnx".into(),
    model_name: "bge-small-en-v1.5".into(),
    model_path: PathBuf::from("models/bge-small-en-v1.5/onnx/model.onnx"),
    tokenizer_path: Some(PathBuf::from("models/bge-small-en-v1.5/tokenizer.json")),
    ..Default::default()
};

let raw = RawIngestRecord { /* ... */ };

let (doc, _, embedding) = process_pipeline(
    raw,
    PipelineStageConfig::Semantic,
    &ingest_cfg,
    &canonical_cfg,
    None,
    Some(&semantic_cfg),
)?;

let emb = embedding.unwrap();
println!("Embedding dimension: {}", emb.embedding_dim);
println!("Vector sample: {:?}", &emb.vector[..4]);
```

### Both Perceptual and Semantic

Run both fingerprinting stages in a single pipeline call:

```rust
use ucfp::{
    process_pipeline, PipelineStageConfig,
    IngestConfig, CanonicalizeConfig, PerceptualConfig, SemanticConfig,
    IngestMetadata, IngestPayload, IngestSource,
};

let raw = RawIngestRecord { /* ... */ };

let (doc, fingerprint, embedding) = process_pipeline(
    raw,
    PipelineStageConfig::PerceptualAndSemantic,  // New!
    &IngestConfig::default(),
    &CanonicalizeConfig::default(),
    Some(&PerceptualConfig::default()),
    Some(&SemanticConfig::default()),
)?;

// Now you have canonical hash, perceptual fingerprint, AND semantic embedding
```

---

## Pipeline Architecture

UCFP processes documents through four optional stages:

```
RawIngestRecord
       │
       ▼
  ┌─────────┐
  │  Ingest │  → Validation, metadata normalization
  └─────────┘
       │
       ▼
  ┌───────────┐
  │Canonical  │  → NFKC normalization, SHA-256 hashing
  └───────────┘
       │
       ▼
  ┌────────────┐
  │ Perceptual │  → Rolling-hash shingles, winnowing, MinHash LSH
  └────────────┘
       │
       ▼
  ┌──────────┐
  │ Semantic │  → ONNX-based dense embeddings
  └──────────┘
```

### Ingest Stage

The `ingest` crate validates and normalizes incoming records.

```rust
use ucfp::{IngestConfig, IngestError, RawIngestRecord, ingest};

let cfg = IngestConfig::default();
// Returns CanonicalIngestRecord on success
let canonical = ingest(raw, &cfg)?;
```

### Canonical Stage

The `canonical` crate produces deterministic hashes.

```rust
use ucfp::{canonicalize, CanonicalizeConfig, CanonicalizedDocument, Token};

let cfg = CanonicalizeConfig {
    normalize_unicode: true,
    lowercase: true,
    ..Default::default()
};

let doc = canonicalize("doc-id", "Héllo Wörld", &cfg)?;
// doc.canonical_text -> "hello world"
// doc.sha256_hex -> deterministic SHA-256 hex
// doc.tokens -> vec![Token { text: "hello", ... }, Token { text: "world", ... }]
```

### Perceptual Stage

The `perceptual` crate creates MinHash signatures for near-duplicate detection.

```rust
use ucfp::{perceptualize_tokens, PerceptualConfig, PerceptualFingerprint};

let cfg = PerceptualConfig {
    k: 9,              // shingle size (character n-grams)
    w: 4,              // winnow window
    minhash_bands: 16, // LSH bands for Jaccard similarity
    ..Default::default()
};

let tokens = ["hello", "world"];
let fp = perceptualize_tokens(&tokens, cfg)?;
// fp.minhash -> vec![u64; 128] MinHash signature
// fp.shingles -> all shingles generated
// fp.minhash_bands -> LSH band hashes for quick filtering
```

### Semantic Stage

The `semantic` crate produces dense embeddings via ONNX runtime.

```rust
use ucfp::{semanticize, SemanticConfig, SemanticEmbedding};

let cfg = SemanticConfig {
    tier: "balanced".into(),   // or "fast", "quality"
    mode: "onnx".into(),       // ONNX inference
    model_name: "bge-small-en-v1.5".into(),
    model_path: PathBuf::from("models/bge-small-en-v1.5/onnx/model.onnx"),
    tokenizer_path: Some(PathBuf::from("models/bge-small-en-v1.5/tokenizer.json")),
    enable_chunking: true,   // for docs > 512 tokens
    ..Default::default()
};

let embedding = semanticize("doc-id", "Your text here", &cfg).await?;
// embedding.vector -> Vec<f32> dense embedding
// embedding.embedding_dim -> e.g., 384 for bge-small-en-v1.5
```

---

## Index and Search

The `ucfp_index` crate provides persistent storage with ANN search.

### Index Configuration

Choose a backend:

```rust
use index::{UfpIndex, IndexConfig, BackendConfig};

// Redb (default, persistent)
let config = IndexConfig::new()
    .with_backend(BackendConfig::redb("./data/ucfp.redb"));

// In-memory (testing, ephemeral)
let config = IndexConfig::new()
    .with_backend(BackendConfig::in_memory());

let index = UfpIndex::new(config)?;
```

Backend comparison:

| Backend | Use Case | Dependencies | Persistence |
|---------|----------|--------------|-------------|
| **Redb** (default) | Production | None (pure Rust) | Yes |
| **InMemory** | Testing | None | No |

### Upserting Records

Store documents with their fingerprints and embeddings:

```rust
use index::{IndexRecord, UfpIndex, INDEX_SCHEMA_VERSION};
use ndarray::Array1;
use serde_json::json;

// Embedding must be quantized to i8 for storage
let quantized = UfpIndex::quantize(
    &Array1::from(embedding.vector.clone()),
    127.0,  // scale factor
);

let record = IndexRecord {
    schema_version: INDEX_SCHEMA_VERSION,
    canonical_hash: doc.sha256_hex.clone(),
    perceptual: Some(fingerprint.minhash.clone()),
    embedding: Some(quantized),
    metadata: json!({
        "doc_id": doc.doc_id,
        "tenant_id": "tenant-a",
    }),
};

index.upsert(&record)?;
```

### Query Modes

Search with different strategies:

```rust
use index::{QueryMode, QueryResult};

// Perceptual search (MinHash Jaccard similarity)
let results = index.search(&query_record, QueryMode::Perceptual, top_k)?;

// Semantic search (cosine similarity on embeddings)
let results = index.search(&query_record, QueryMode::Semantic, top_k)?;

// Combined: weighted blend of both signals
let results = index.search(&query_record, QueryMode::Hybrid, top_k)?;
```

### Matcher API

The `matcher` crate combines pipeline processing with index search:

```rust
use ucfp_matcher::{Matcher, MatchConfig, MatchRequest, MatchHit};

let matcher = Matcher::new(
    index.clone(),
    ingest_cfg,
    canonical_cfg,
    perceptual_cfg,
    semantic_cfg,
);

let req = MatchRequest {
    tenant_id: "tenant-a".into(),
    query_text: "Rust memory safety".into(),
    config: MatchConfig {
        max_results: 10,
        query_mode: QueryMode::Hybrid,
        ..Default::default()
    },
};

let hits = matcher.match_document(&req)?;
// hits: Vec<MatchHit> with scores and canonical hashes
```

---

## Observability

### Metrics

Install a metrics recorder to capture latency per stage:

```rust
use std::sync::Arc;
use std::time::Duration;
use ucfp::{PipelineMetrics, set_pipeline_metrics};
use ucfp_index::IndexError;
use ucfp_matcher::MatchError;

struct MyMetrics;

impl PipelineMetrics for MyMetrics {
    fn record_ingest(&self, latency: Duration, result: Result<(), ucfp::IngestError>) {
        println!("ingest: {}µs", latency.as_micros());
    }
    fn record_canonical(&self, latency: Duration, result: Result<(), ucfp::PipelineError>) {
        println!("canonical: {}µs", latency.as_micros());
    }
    fn record_perceptual(&self, latency: Duration, result: Result<(), ucfp::PerceptualError>) {
        println!("perceptual: {}µs", latency.as_micros());
    }
    fn record_semantic(&self, latency: Duration, result: Result<(), ucfp::SemanticError>) {
        println!("semantic: {}µs", latency.as_micros());
    }
    fn record_index(&self, latency: Duration, result: Result<(), IndexError>) {
        println!("index: {}µs", latency.as_micros());
    }
    fn record_match(&self, latency: Duration, result: Result<(), MatchError>) {
        println!("match: {}µs", latency.as_micros());
    }
}

set_pipeline_metrics(Some(Arc::new(MyMetrics)));
```

### Structured Logging

Install a logger to capture structured events:

```rust
use std::sync::Arc;
use ucfp::{PipelineEventLogger, PipelineEvent, KeyValueLogger, set_pipeline_logger};

let logger = KeyValueLogger::stdout();
set_pipeline_logger(Some(Arc::new(logger)));

// Output:
// timestamp="2025-01-01T00:00:00.000Z" stage=ingest status=success latency_us=45 record_id="doc-001" tenant_id="tenant-a"
// timestamp="2025-01-01T00:00:00.000Z" stage=canonical status=success latency_us=180 record_id="doc-001" tenant_id="tenant-a"
// timestamp="2025-01-01T00:00:00.000Z" stage=perceptual status=success latency_us=180 record_id="doc-001" tenant_id="tenant-a"
```

---

## Configuration

### YAML Configuration

Load from a YAML file:

```yaml
version: "1.0"

ingest:
  default_tenant_id: "acme-corp"
  max_payload_bytes: 10485760

canonical:
  normalize_unicode: true
  lowercase: true

perceptual:
  k: 9
  w: 4
  minhash_bands: 16

semantic:
  tier: "balanced"
  enable_chunking: true
  model_name: "bge-small-en-v1.5"

index:
  backend: "redb"
  ann:
    enabled: true
    min_vectors_for_ann: 1000
```

```rust
use ucfp::config::UcfpConfig;

let config = UcfpConfig::from_file("config.yaml")?;
```

### Programmatic Configuration

```rust
// Ingest
let ingest_cfg = IngestConfig {
    default_tenant_id: Some("acme-corp".into()),
    max_payload_bytes: 10 * 1024 * 1024,
    ..Default::default()
};

// Canonical
let canonical_cfg = CanonicalizeConfig {
    normalize_unicode: true,
    lowercase: true,
    ..Default::default()
};

// Perceptual
let perceptual_cfg = PerceptualConfig {
    k: 9,              // shingle size
    w: 4,              // winnow window
    minhash_bands: 16, // LSH bands
    ..Default::default()
};

// Semantic
let semantic_cfg = SemanticConfig {
    tier: "balanced".into(),
    enable_chunking: true,
    chunk_overlap_ratio: 0.5,
    pooling_strategy: "weighted_mean".into(),
    model_name: "bge-small-en-v1.5".into(),
    model_path: PathBuf::from("models/bge-small-en-v1.5/onnx/model.onnx"),
    tokenizer_path: Some(PathBuf::from("models/bge-small-en-v1.5/tokenizer.json")),
    ..Default::default()
};
```

---

## REST API

Start the server:

```bash
cargo run --package ucfp-server
```

Process a document:

```bash
curl -X POST http://localhost:8080/api/v1/process \
  -H "Content-Type: application/json" \
  -H "X-API-Key: your-api-key" \
  -d '{
    "text": "Your document content here",
    "enable_perceptual": true,
    "enable_semantic": true,
    "tenant_id": "tenant-a"
  }'
```

Search for matches:

```bash
curl -X POST http://localhost:8080/api/v1/match \
  -H "Content-Type: application/json" \
  -H "X-API-Key: your-api-key" \
  -d '{
    "query_text": "Rust memory safety",
    "tenant_id": "tenant-a",
    "max_results": 10
  }'
```

See [`crates/server/API.md`](crates/server/API.md) for full API reference.

---

## Performance

| Stage | Latency | Notes |
|:------|:--------|:------|
| `ingest` | ~45 μs | Validation + metadata |
| `canonical` | ~180 μs | Unicode NFKC + SHA-256 |
| `perceptual` | ~180 μs | Parallel MinHash LSH |
| `semantic` | ~8.5 ms | ONNX embedding |
| `index` | ~50 μs | Lock-free DashMap |
| `match` | ~50-450 μs | ANN O(log n) at >1K vectors |

**Optimizations:**
- Lock-free concurrency via DashMap
- Parallel MinHash computation
- HNSW ANN search for semantic queries
- HTTP/2 connection pooling for external embedding APIs
- Quantized i8 embeddings to reduce storage and improve cache locality

Disable semantic stage for ~100 μs/doc when exact + perceptual matching is sufficient.

---

## Feature Flags

| Flag | Default | Description |
|------|---------|-------------|
| `backend-redb` | On | Redb storage backend |
| `backend-inmemory` | On | In-memory backend for testing |

---

## Roadmap

| Modality | Status | Canonicalizer | Fingerprint | Embedding |
|:---------|:-------|:--------------|:------------|:----------|
| **Text** | Ready | NFKC + tokenization | MinHash | BGE / E5 |
| **Image** | Planned | DCT normalization | pHash | CLIP / SigLIP |
| **Audio** | Planned | Mel-spectrogram | Winnowing | SpeechCLIP / Whisper |
| **Video** | Planned | Keyframes | Scene hashes | VideoCLIP / XCLIP |
| **Document** | Planned | OCR + layout | Layout graph | LayoutLMv3 |

---

## Minimum Supported Rust Version

**Rust 1.76+**