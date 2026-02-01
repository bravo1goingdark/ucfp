<div align="center">

# Universal Content Fingerprinting (UCFP)

**Deterministic, reproducible content fingerprints for text, audio, image, video, and documents**

[![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![CI](https://img.shields.io/github/actions/workflow/status/bravo1goingdark/ucfp/ci.yml?style=for-the-badge&label=CI)](https://github.com/bravo1goingdark/ucfp/actions)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg?style=for-the-badge)](LICENSE)

</div>

---

<div align="center">

### One Pipeline. Multiple Modalities. Infinite Possibilities.

</div>

UCFP is an **open-source Rust framework** that unifies **exact hashing**, **perceptual similarity**, and **semantic embeddings** into a single, coherent pipeline. Built for speed and reliability, it powers:

- **Deduplication** — Find exact and near-duplicate content
- **Plagiarism Detection** — Identify paraphrased content
- **Content Provenance** — Track content across systems
- **Multimodal Search** — Search by meaning, not just keywords

---

## About

UCFP solves a fundamental problem in content systems: traditional hashes fail when content changes even slightly, while semantic meaning requires understanding beyond byte-level matching. This framework provides three complementary layers—**exact hashes** for identical matching, **perceptual fingerprints** for near-duplicates, and **semantic embeddings** for meaning-based comparison—all in a single deterministic pipeline.

Built in Rust for performance and safety, each pipeline stage (ingest, canonical, perceptual, semantic, index, match) operates as a standalone crate with comprehensive observability. The modular design allows you to adopt only what you need while maintaining clean architectural boundaries and reproducible results across environments.

---

## Quickstart

### Prerequisites

- **Rust 1.76+** — install with `rustup toolchain install stable`
- `cargo` on your `PATH`

### Build & Test

```bash
# Format, lint, and test
cargo fmt --all
cargo clippy --all --all-targets -- -D warnings
cargo test --all
```

### Run Examples

```bash
# Individual stage examples
cargo run --package ingest --example ingest_demo
cargo run --package canonical --example demo
cargo run --package perceptual --example fingerprint_demo
cargo run --package semantic --example embed "Title" "Text to embed"
cargo run --package index --example index_demo

# Full pipeline
cargo run --example full_pipeline
cargo run --example pipeline_metrics  # with observability
cargo run                              # end-to-end demo
```

---

## API Documentation

Complete REST API documentation is available:

- **Server API Reference**: [`crates/server/API.md`](crates/server/API.md) - Full REST API documentation with examples
- **Server Quick Start**: [`crates/server/README.md`](crates/server/README.md) - Getting started guide for the HTTP server

### Quick API Example

```bash
# Process a document with chunking enabled for long text
curl -X POST http://localhost:8080/api/v1/process \
  -H "Content-Type: application/json" \
  -H "X-API-Key: your-api-key" \
  -d '{
    "text": "Your document content here...",
    "enable_semantic": true,
    "semantic_config": {
      "max_sequence_length": 512,
      "enable_chunking": true,
      "chunk_overlap_ratio": 0.5,
      "pooling_strategy": "weighted_mean"
    }
  }'
```

---

## Usage

### Simple Example

```rust
use ucfp::{
    CanonicalizeConfig, IngestPayload, IngestSource,
    PerceptualConfig, RawIngestRecord, process_record_with_perceptual,
};

let record = RawIngestRecord {
    id: "demo".into(),
    source: IngestSource::RawText,
    metadata: Default::default(),
    payload: Some(IngestPayload::Text("Hello world".into())),
};

let (doc, fingerprint) = process_record_with_perceptual(
    record,
    &CanonicalizeConfig::default(),
    &PerceptualConfig::default(),
)?;

println!("Canonical: {}", doc.canonical_text);
println!("MinHash bands: {}", fingerprint.minhash_bands.len());
```

---

## Full Pipeline Example

Complete workflow from ingest to matching:

```rust
use ucfp::{
    CanonicalizeConfig, IngestConfig, IngestMetadata, IngestPayload, IngestSource,
    PerceptualConfig, RawIngestRecord, SemanticConfig,
    process_record_with_perceptual, semanticize_document,
};
use ucfp_index::{BackendConfig, IndexConfig, IndexRecord, UfpIndex};
use ucfp_matcher::{DefaultMatcher, MatchConfig, MatchRequest, Matcher};

// 1. Configure all stages
let ingest_cfg = IngestConfig::default();
let canonical_cfg = CanonicalizeConfig::default();
let perceptual_cfg = PerceptualConfig::default();
let semantic_cfg = SemanticConfig::default();

// 2. Create index
let index_cfg = IndexConfig::new().with_backend(BackendConfig::InMemory);
let index = UfpIndex::new(index_cfg).unwrap();

// 3. Ingest a document
let record = RawIngestRecord {
    id: "doc-001".into(),
    source: IngestSource::RawText,
    metadata: IngestMetadata {
        tenant_id: Some("tenant-a".to_string()),
        doc_id: Some("my-doc".to_string()),
        ..Default::default()
    },
    payload: Some(IngestPayload::Text("Rust memory safety features".into())),
};

// 4. Process through pipeline (ingest -> canonical -> perceptual)
let (doc, fingerprint) =
    process_record_with_perceptual(record, &canonical_cfg, &perceptual_cfg)?;

// 5. Generate semantic embedding
let embedding = semanticize_document(&doc, &semantic_cfg)?;

// 6. Store in index
let record = IndexRecord {
    doc_id: doc.doc_id.clone(),
    tenant_id: "tenant-a".to_string(),
    canonical_hash: doc.canonical_hash.clone(),
    perceptual_fingerprint: Some(fingerprint),
    semantic_embedding: Some(embedding),
    ..Default::default()
};
index.upsert(record)?;

// 7. Search with matcher
let matcher = DefaultMatcher::new(
    index,
    ingest_cfg,
    canonical_cfg,
    perceptual_cfg,
    semantic_cfg,
);

let req = MatchRequest {
    tenant_id: "tenant-a".to_string(),
    query_text: "Rust safety".to_string(),
    config: MatchConfig::default(),
    ..Default::default()
};

let hits = matcher.match_document(&req)?;
println!("Found {} matches", hits.len());
```

---

## Configuration

### YAML Config

```yaml
version: "1.0"

ingest:
  default_tenant_id: "acme-corp"
  max_payload_bytes: 10485760

canonical:
  normalize_unicode: true
  lowercase: true

perceptual:
  k: 9              # shingle size
  w: 4              # winnow window
  minhash_bands: 16
  use_parallel: true

semantic:
  tier: "balanced"
  mode: "fast"

index:
  backend: "redb"
  redb_path: "./data/index"
```

### Load in Code

```rust
use ucfp::config::UcfpConfig;

let config = UcfpConfig::from_file("config.yaml")?;
let ingest_cfg = config.to_ingest_config();
let perceptual_cfg = config.to_perceptual_config();
```

---

## Runtime Configuration

All UCFP features are **runtime-configurable** — no restarts or redeploys required:

- **Pipeline stages**: Enable/disable semantic or perceptual processing per request
- **Backend selection**: Switch between in-memory, Redb, or remote stores without code changes

---

## Architecture

```
+---------+    +-----------+    +--------------------+    +---------+    +-------+
|  ingest |--->| canonical |--->|perceptual/semantic |--->|  index  |--->| match |
+---------+    +-----------+    +--------------------+    +---------+    +-------+
```

The pipeline consists of six stages, each with a specific responsibility. Each crate can be used independently, or you can use the root `ucfp` crate for convenient orchestration.

| Stage | Responsibility | Key Types |
|:------|:---------------|:----------|
| **ingest** | Validation, metadata normalization, ID derivation | `IngestConfig`, `RawIngestRecord`, `CanonicalIngestRecord` |
| **canonical** | Unicode NFKC normalization, tokenization, SHA-256 hashing | `CanonicalizeConfig`, `CanonicalizedDocument`, `Token` |
| **perceptual** | Rolling-hash shingles, winnowing, MinHash signatures | `PerceptualConfig`, `PerceptualFingerprint` |
| **semantic** | Dense embeddings via ONNX, API, or deterministic stub | `SemanticConfig`, `SemanticEmbedding` |
| **index** | Storage backend abstraction, retrieval, similarity search | `IndexConfig`, `UfpIndex`, `QueryResult` |
| **match** | Query-time matching with tenant isolation | `MatchConfig`, `DefaultMatcher`, `MatchResult` |

---

## Workspace Layout

```
crates/
├── ingest/       # Stage 1: validation & normalization
├── canonical/    # Stage 2: canonical text pipeline
├── perceptual/   # Stage 3a: shingling, winnowing, MinHash
├── semantic/     # Stage 3b: embedding generation
├── index/        # Stage 4: storage backend
└── match/        # Stage 5: query-time matching

src/              # CLI demo & re-exports
tests/            # Integration tests
examples/         # Workspace demos
```

---

## Metrics & Observability

Hook into pipeline stages:

```rust
use ucfp::{set_pipeline_metrics, set_pipeline_logger};

set_pipeline_metrics(my_metrics_recorder);
set_pipeline_logger(my_structured_logger);
```

### Pipeline Performance Metrics

All pipeline stages emit detailed metrics. Benchmarked on a typical development machine running optimized release builds:

| Stage | Purpose | Latency | Throughput |
|:------|:--------|:--------|:-----------|
| `ingest` | Validation and normalization | ~45 μs | validation + metadata |
| `canonical` | Text canonicalization | ~180 μs | Unicode NFKC + SHA-256 |
| `perceptual` | Fingerprint generation | ~320 μs | MinHash LSH |
| `semantic` | Embedding generation | ~8.5 ms | ONNX embedding |
| `index` | Storage operations | ~95 μs | upsert operation |
| `match` | Query execution | ~450 μs | similarity search |

#### Handling Long Documents (Chunking)

For documents exceeding the model's token limit (e.g., 512 tokens for BERT), UCFP supports **sliding-window chunking** with weighted pooling:

```rust
use ucfp::{semanticize_document, SemanticConfig};

// Configure for long documents
let semantic_cfg = SemanticConfig {
    max_sequence_length: 512,                // Model's token limit
    enable_chunking: true,                   // Enable sliding-window chunking
    chunk_overlap_ratio: 0.5,                // 50% overlap between chunks
    pooling_strategy: "weighted_mean".into(), // Center-weighted pooling
    ..Default::default()
};

// Long document (1000+ words) is automatically chunked and pooled
let long_text = "Very long document content...".repeat(100);
let doc = canonicalize("doc-001", &long_text, &canonical_cfg)?;
let embedding = semanticize_document(&doc, &semantic_cfg)?;
```

**How it works:**
1. Long text is split into overlapping chunks (50% overlap by default)
2. Each chunk is embedded independently via ONNX
3. Embeddings are pooled using center-weighted mean
4. Returns a single embedding representing the entire document

**Performance:** Chunking requires N inference calls for N chunks. A 1000-word document produces ~3-4 chunks, requiring ~30ms total (vs 8.5ms for short text).

#### End-to-End Performance

- **Small doc (100 words)**: ~1.2 ms (full pipeline)
- **Medium doc (1K words)**: ~10 ms (full pipeline)
- **Large doc (10K words)**: ~95 ms (full pipeline)
- **Batch throughput**: ~650 μs per doc (100 docs)

> **Note**: Disable the semantic stage for ~100x faster processing (~100 μs per doc) when only exact + perceptual matching is needed.

#### Example Output

```
timestamp="2025-02-10T02:15:01.234Z" stage=ingest status=success latency_us=45
timestamp="2025-02-10T02:15:01.241Z" stage=canonical status=success latency_us=180
timestamp="2025-02-10T02:15:01.245Z" stage=perceptual status=success latency_us=320
timestamp="2025-02-10T02:15:01.259Z" stage=semantic status=success latency_ms=8.5
timestamp="2025-02-10T02:15:01.269Z" stage=index status=success latency_us=95
timestamp="2025-02-10T02:15:01.273Z" stage=match status=success latency_us=450
```

Run the metrics example:
```bash
cargo run --example pipeline_metrics
```

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

## Contributing

We welcome fixes, optimizations, and new modalities!

Please read [`CONTRIBUTING.md`](CONTRIBUTING.md) for:
- Workflow guidelines
- Required checks (`cargo fmt`, `cargo clippy`, `cargo test`)
- Documentation expectations

---
