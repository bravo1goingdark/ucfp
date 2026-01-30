<div align="center">

# 🔐 Universal Content Fingerprinting (UCFP)

**Deterministic, reproducible content fingerprints for text, audio, image, video, and documents**

<br>

[![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![CI](https://img.shields.io/github/actions/workflow/status/bravo1goingdark/ucfp/ci.yml?style=for-the-badge&label=CI)](https://github.com/bravo1goingdark/ucfp/actions)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg?style=for-the-badge)](LICENSE)

<br>

[🚀 Quickstart](#quickstart) • [📖 Usage](#usage) • [🏗️ Architecture](#architecture) • [📊 Performance](#metrics--observability) • [🗺️ Roadmap](#roadmap)

</div>

---

<div align="center">

### ✨ One Pipeline. Multiple Modalities. Infinite Possibilities.

</div>

UCFP is an **open-source Rust framework** that unifies **exact hashing**, **perceptual similarity**, and **semantic embeddings** into a single, coherent pipeline. Built for speed and reliability, it powers:

- 🔍 **Deduplication** — Find exact and near-duplicate content
- 📝 **Plagiarism Detection** — Identify paraphrased content
- 🕵️ **Content Provenance** — Track content across systems
- 🔎 **Multimodal Search** — Search by meaning, not just keywords

---

## 🎯 Features

| Feature | What It Does |
|:--------|:-------------|
| 📥 **Deterministic Ingest** | Metadata validation, canonical IDs, whitespace normalization |
| 📝 **Canonical Text** | Unicode NFKC, lowercasing, punctuation stripping, SHA-256 digests |
| 🎨 **Perceptual Fingerprints** | Rolling-hash shingles, winnowing, MinHash signatures |
| 🧠 **Semantic Embeddings** | ONNX/API-backed dense vectors with deterministic fallbacks |
| 🗄️ **Pluggable Indexing** | Backend-agnostic storage (RocksDB, in-memory) |
| ⚡ **Clean Architecture** | Linear pipeline with no circular dependencies |

---

## 🚀 Quickstart

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

## 📖 Usage

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

## 🏗️ Full Pipeline Example

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

// 4. Process through pipeline (ingest → canonical → perceptual)
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

## ⚙️ Configuration

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
  backend: "rocksdb"
  rocksdb_path: "./data/index"
```

### Load in Code

```rust
use ucfp::config::UcfpConfig;

let config = UcfpConfig::from_file("config.yaml")?;
let ingest_cfg = config.to_ingest_config();
let perceptual_cfg = config.to_perceptual_config();
```

---

## 🏗️ Architecture

```
┌─────────┐    ┌───────────┐    ┌──────────────────┐    ┌─────────┐    ┌───────┐
│  ingest │───▶│ canonical │───▶│perceptual/semantic│───▶│  index  │───▶│ match │
└─────────┘    └───────────┘    └──────────────────┘    └─────────┘    └───────┘
```

### Stage Responsibilities

| Stage | What It Does | Key Types |
|:------|:-------------|:----------|
| `ingest` | Validation, metadata, ID derivation | `IngestConfig`, `RawIngestRecord`, `CanonicalIngestRecord` |
| `canonical` | Unicode NFKC, tokenization, SHA-256 | `CanonicalizeConfig`, `CanonicalizedDocument`, `Token` |
| `perceptual` | Shingles, winnowing, MinHash | `PerceptualConfig`, `PerceptualFingerprint` |
| `semantic` | Dense embeddings (ONNX/API/stub) | `SemanticConfig`, `SemanticEmbedding` |
| `index` | Storage, retrieval, search | `IndexConfig`, `UfpIndex`, `QueryResult` |
| `match` | Query-time matching, tenant isolation | `MatchConfig`, `DefaultMatcher`, `MatchResult` |

Each crate can be used independently. The root `ucfp` crate provides convenience orchestration.

---

## 📦 Workspace Layout

```
crates/
├── 📥 ingest/       # Stage 1: validation & normalization
├── 📝 canonical/    # Stage 2: canonical text pipeline
├── 🎨 perceptual/   # Stage 3a: shingling, winnowing, MinHash
├── 🧠 semantic/     # Stage 3b: embedding generation
├── 🗄️ index/        # Stage 4: storage backend
└── 🎯 match/        # Stage 5: query-time matching

src/              # CLI demo & re-exports
tests/            # Integration tests
examples/         # Workspace demos
```

---

## 📊 Metrics & Observability

Hook into pipeline stages:

```rust
use ucfp::{set_pipeline_metrics, set_pipeline_logger};

set_pipeline_metrics(my_metrics_recorder);
set_pipeline_logger(my_structured_logger);
```

### Stage Metrics

All pipeline stages emit detailed metrics:

| Stage | Purpose | Metric Type |
|:------|:--------|:------------|
| `ingest` | Validation and normalization | Latency, throughput |
| `canonical` | Text canonicalization | Latency, token count |
| `perceptual` | Fingerprint generation | Latency, shingles/sec |
| `semantic` | Embedding generation | Latency, vectors/sec |
| `index` | Storage operations | Latency, query time |
| `match` | Query execution | Latency, match count |

### ⚡ Real-Time Performance Metrics

Benchmarked on a typical development machine (Windows, unoptimized debug build):

| Stage | Latency | Throughput |
|:------|:--------|:-----------|
| `ingest` | ~113 µs | validation + normalization |
| `canonical` | ~249 µs | Unicode NFKC + tokenization |
| `perceptual` | ~143-708 µs | MinHash fingerprinting |
| `semantic` | ~109 µs | embedding generation |
| `index` | ~180 µs | storage operation |
| `match` | ~320 µs | query execution |

#### 📈 End-to-End Performance

- **Single 1,000-word doc**: ~30ms (full pipeline)
- **Large 10,000-word doc**: ~150ms (full pipeline)
- **Batch throughput**: ~1.7ms per doc (100 docs)
- **Small docs**: ~244µs per doc (1,000 docs)

#### 📝 Example Output

```
timestamp="2025-02-10T02:15:01.234Z" stage=ingest status=success latency_us=113
timestamp="2025-02-10T02:15:01.241Z" stage=canonical status=success latency_us=249
timestamp="2025-02-10T02:15:01.245Z" stage=perceptual status=success latency_us=143
timestamp="2025-02-10T02:15:01.249Z" stage=semantic status=success latency_us=109
timestamp="2025-02-10T02:15:01.252Z" stage=index status=success latency_us=180
timestamp="2025-02-10T02:15:01.255Z" stage=match status=success latency_us=320
```

Run the metrics example:
```bash
cargo run --example pipeline_metrics
```

---

## 🗺️ Roadmap

| Modality | Status | Canonicalizer | Fingerprint | Embedding |
|:---------|:-------|:--------------|:------------|:----------|
| **Text** | ✅ Ready | NFKC + tokenization | MinHash | BGE / E5 |
| **Image** | 🔮 Planned | DCT normalization | pHash | CLIP / SigLIP |
| **Audio** | 🔮 Planned | Mel-spectrogram | Winnowing | SpeechCLIP / Whisper |
| **Video** | 🔮 Planned | Keyframes | Scene hashes | VideoCLIP / XCLIP |
| **Document** | 🔮 Planned | OCR + layout | Layout graph | LayoutLMv3 |

---

## 🤝 Contributing

We welcome fixes, optimizations, and new modalities!

Please read [`CONTRIBUTING.md`](CONTRIBUTING.md) for:
- Workflow guidelines
- Required checks (`cargo fmt`, `cargo clippy`, `cargo test`)
- Documentation expectations

---

## 📜 License

<div align="center">

**MIT** OR **Apache-2.0**

Choose whichever works best for your project.

</div>

---

<div align="center">

Made with ❤️ in Rust

</div>
