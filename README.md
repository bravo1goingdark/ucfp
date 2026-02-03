<div align="center">

# Universal Content Fingerprinting (UCFP)

**Deterministic, reproducible content fingerprints for text, audio, image, video, and documents**

[![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![CI](https://img.shields.io/github/actions/workflow/status/bravo1goingdark/ucfp/ci.yml?style=for-the-badge&label=CI)](https://github.com/bravo1goingdark/ucfp/actions)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg?style=for-the-badge)](LICENSE)
[![GitHub stars](https://img.shields.io/github/stars/bravo1goingdark/ucfp?style=for-the-badge&logo=github&color=yellow)](https://github.com/bravo1goingdark/ucfp/stargazers)

</div>

UCFP is a Rust framework that unifies **exact hashing**, **perceptual similarity**, and **semantic embeddings** into a single pipeline.

Traditional hashes fail when content changes slightly. Semantic search requires understanding beyond byte matching. UCFP gives you both—exact matches and meaning-based similarity—in one deterministic pipeline.

- **Deduplication** — Find exact and near-duplicate content
- **Plagiarism Detection** — Identify paraphrased text
- **Content Provenance** — Track content across systems
- **Similarity Search** — Search by meaning, not just keywords

## Quickstart

**Prerequisites**: Rust 1.76+ (`rustup toolchain install stable`)

```bash
# Build & test
cargo test --all

# Run examples
cargo run --example full_pipeline          # complete pipeline
cargo run --example pipeline_metrics       # with observability
cargo run --package perceptual --example fingerprint_demo
```

## Usage

```rust
use ucfp::{
    CanonicalizeConfig, IngestPayload, IngestSource,
    PerceptualConfig, RawIngestRecord, process_record_with_perceptual,
};

let record = RawIngestRecord {
    id: "demo".into(),
    source: IngestSource::RawText,
    payload: Some(IngestPayload::Text("Hello world".into())),
    ..Default::default()
};

let (doc, fingerprint) = process_record_with_perceptual(
    record,
    &CanonicalizeConfig::default(),
    &PerceptualConfig::default(),
)?;

println!("Canonical hash: {}", doc.canonical_hash);
println!("MinHash bands: {}", fingerprint.minhash_bands.len());
```

See [`examples/`](examples/) for full pipeline demonstrations.

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

## Architecture

| Stage | Responsibility | Key Types |
|:------|:---------------|:----------|
| **ingest** | Validation, metadata normalization | `RawIngestRecord`, `CanonicalIngestRecord` |
| **canonical** | Unicode NFKC normalization, SHA-256 hashing | `CanonicalizedDocument` |
| **perceptual** | Rolling-hash shingles, winnowing, MinHash LSH | `PerceptualFingerprint` |
| **semantic** | Dense embeddings via ONNX | `SemanticEmbedding` |
| **index** | Storage with HNSW ANN search | `UfpIndex`, `QueryResult` |
| **match** | Query-time matching | `DefaultMatcher`, `MatchResult` |

![UCFP Architecture Diagram](ucfp.png)

## Configuration

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

semantic:
  tier: "balanced"
  enable_chunking: true  # For documents > 512 tokens

index:
  backend: "redb"
  ann:
    enabled: true
    min_vectors_for_ann: 1000
```

Load in code:
```rust
use ucfp::config::UcfpConfig;
let config = UcfpConfig::from_file("config.yaml")?;
```

## Performance

| Stage | Latency | Notes |
|:------|:--------|:------|
| `ingest` | ~45 μs | Validation + metadata |
| `canonical` | ~180 μs | Unicode NFKC + SHA-256 |
| `perceptual` | ~180 μs | Parallel MinHash LSH |
| `semantic` | ~8.5 ms | ONNX embedding |
| `index` | ~50 μs | Lock-free DashMap |
| `match` | ~50-450 μs | ANN O(log n) at >1K vectors |

**Optimizations**: Lock-free concurrency, parallel MinHash, HNSW ANN search, HTTP/2 connection pooling, SIMD vector operations.

Disable semantic stage for ~100 μs/doc when exact + perceptual matching is sufficient.

## API

REST API server included. Quick example:

```bash
curl -X POST http://localhost:8080/api/v1/process \
  -H "Content-Type: application/json" \
  -H "X-API-Key: your-api-key" \
  -d '{
    "text": "Your document content...",
    "enable_semantic": true
  }'
```

See [`crates/server/API.md`](crates/server/API.md) for full API reference.

## Roadmap

| Modality | Status | Canonicalizer | Fingerprint | Embedding |
|:---------|:-------|:--------------|:------------|:----------|
| **Text** | Ready | NFKC + tokenization | MinHash | BGE / E5 |
| **Image** | Planned | DCT normalization | pHash | CLIP / SigLIP |
| **Audio** | Planned | Mel-spectrogram | Winnowing | SpeechCLIP / Whisper |
| **Video** | Planned | Keyframes | Scene hashes | VideoCLIP / XCLIP |
| **Document** | Planned | OCR + layout | Layout graph | LayoutLMv3 |

## Development

```bash
./run-ci-local.sh  # Format, lint, test, build
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

Apache-2.0
