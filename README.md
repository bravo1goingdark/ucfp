# Universal Content Fingerprinting (UCFP)

UCFP (Universal Content Fingerprint) is an open-source framework for generating unique, reproducible,
and meaning-aware fingerprints across text, audio, image, video, and document payloads. It unifies
exact hashing, perceptual similarity, and semantic embeddings into one coherent pipeline, so developers
can identify, compare, and link content deterministically and perceptually. Built in Rust for speed and
reliability, UCFP powers use cases such as deduplication, plagiarism detection, data cleaning, 
content provenance, and multimodal search.


## Why UCFP?

- **Deterministic ingest** – strict metadata validation, canonical IDs, and consistent whitespace
  normalization keep upstream feeds clean.
- **Reproducible canonical text** – Unicode NFKC, lowercasing, punctuation stripping, token offsets,
  and SHA-256 digests are exposed as standalone helpers.
- **Perceptual fingerprints** – rolling-hash shingles, winnowing, and MinHash signatures make
  similarity search and near-duplicate detection straightforward.
- **Semantic embeddings** – `ufp_semantic` turns canonical text into ONNX/API-backed dense vectors
  with deterministic fallbacks for offline tiers.
- **Pluggable indexing** - `ufp_index` provides a backend-agnostic index for storing and searching
  canonical hashes, perceptual fingerprints, and quantized semantic embeddings.
- **Single entry point** – the root `ucfp` crate wires every stage into `process_record`,
  `process_record_with_perceptual`, and `process_record_with_semantic`, so applications can adopt the
  full pipeline one call at a time.
- **Built-in observability** – plug in a `PipelineMetrics` recorder to capture latency and results for
  ingest, canonical, perceptual, and semantic stages.

## Use cases

| Use case              | What UCFP contributes                                                                      | Layers & configs                                           |
|-----------------------|-------------------------------------------------------------------------------------------|------------------------------------------------------------|
| Dataset deduplication | Deterministic IDs and canonical hashes collapse byte-identical submissions                | `ufp_ingest` + `IngestConfig`, `ufp_canonical` SHA-256     |
| Plagiarism detection  | Token offsets, shingles, and MinHash detect paraphrased overlaps                          | `ufp_canonical` tokens, `ufp_perceptual` tuned `k`/`w`     |
| Content provenance    | Canonical metadata + perceptual signatures trace assets across feeds, storage, and audits | `ufp_ingest`, `PipelineMetrics`, `PerceptualConfig` seeds  |
| Multimodal search     | Canonical text + binary passthrough feed embedding stores and downstream modalities       | `ufp_index` + `IndexConfig`, `ufp_semantic` embeddings |

## Quickstart

### Prerequisites

- Rust 1.76+ (`rustup toolchain install stable`)
- `cargo` available on your `PATH`

### Build, lint, and test

```bash
cargo fmt --all
cargo clippy --all --all-targets -- -D warnings
cargo test --all
```

### Explore the examples

```bash
cargo run --package ufp_ingest --example ingest_demo
cargo run --package ufp_ingest --example batch_ingest
cargo run --package ufp_canonical --example demo
cargo run --package ufp_canonical --example helpers
cargo run --package ufp_perceptual --example fingerprint_demo
cargo run --package ufp_semantic --example embed "Doc Title" "Some text to embed"
cargo run --package ufp_index --example index_demo
cargo run --package ufp_match --example match_demo
cargo run --example full_pipeline              # ingest + semantic + perceptual + index
cargo run                              # end-to-end demo on big_text.txt
cargo run --example pipeline_metrics   # observe metrics events
```

## Performance Optimizations

### Recent Improvements
- **Query Performance**: Replaced O(n) linear scans with O(log n) indexed lookups using auxiliary indexes
- **Memory Efficiency**: Implemented bounded LRU caches for semantic models and optimized MinHash allocations  
- **Unicode Handling**: Proper grapheme cluster segmentation for complex scripts and emojis
- **Error Handling**: Standardized across all crates for consistency and reliability
- **Input Validation**: Comprehensive sanitization and size limits for robustness

### Benchmarks
- Perceptual fingerprinting: ~20% performance improvement from optimization
- Memory usage: Reduced allocations through pre-allocation and bounded caches
- Query latency: Improved through inverted and vector indexing

## Architecture Overview

UCFP is a layered pipeline:

1. **Ingest (`ufp_ingest`)** – validates metadata, derives deterministic IDs, normalizes text/binary
   payloads, and emits `CanonicalIngestRecord`.
2. **Canonical (`ufp_canonical`)** – converts normalized text into lowercase NFKC strings, token
   streams with byte offsets, and SHA-256 hashes.
3. **Perceptual (`ufp_perceptual`)** – shingles canonical tokens, applies winnowing, and produces
   MinHash fingerprints tuned by `PerceptualConfig`.
4. **Semantic (`ufp_semantic`)** – turns canonical text into dense embeddings via ONNX Runtime or
   remote HTTP APIs, then normalizes/stubs vectors based on the configured tier.
5. **Index (`ufp_index`) - stores, retrieves, and searches fingerprints and embeddings using a
   pluggable backend (e.g., RocksDB, in-memory).
6. **Match (`ufp_match`)** – executes query-time matching over `ufp_index` using semantic,
   perceptual, or hybrid scoring modes.

The root `ucfp` crate re-exports all public types and orchestrates the stages through:

- `process_record` (ingest + canonicalize),
- `process_record_with_perceptual` (full ingest → canonical → perceptual),
- `process_record_with_semantic` (ingest → canonical → semantic embedding),
- `process_record_with_*_configs` helpers when explicit configuration objects are needed,
- `process_semantic_document` / `semanticize_document` when you need only the embedding,
- `big_text_demo` for the bundled integration example,
- `set_pipeline_metrics` / `PipelineMetrics` and `set_pipeline_logger` for observability hooks.

### Layer responsibilities

| Layer           | Responsibilities                                                                                                       | Key types                                                               |
|-----------------|------------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------|
| `ufp_ingest`    | Required metadata enforcement, timestamp defaulting, control-character stripping, whitespace normalization, UTF-8 decode | `IngestConfig`, `RawIngestRecord`, `CanonicalIngestRecord`, `CanonicalPayload` |
| `ufp_canonical` | Unicode normalization, casing/punctuation policies, tokenization with byte offsets, SHA-256 hashing                     | `CanonicalizeConfig`, `CanonicalizedDocument`, `Token`                  |
| `ufp_perceptual`| Rolling-hash shingles, winnowing, MinHash signatures with deterministic seeding and optional parallelism                | `PerceptualConfig`, `PerceptualFingerprint`, `WinnowedShingle`, `PerceptualMeta` |
| `ufp_semantic`  | ONNX/API inference, tokenizer lifecycle management, deterministic stub embeddings for offline or “fast” tiers          | `SemanticConfig`, `SemanticEmbedding`, `SemanticError`                  |
| `ufp_index` | Pluggable storage (RocksDB/in-memory), retrieval, and similarity search for fingerprints and embeddings | `IndexConfig`, `IndexRecord`, `UfpIndex`, `QueryResult` |

### Documentation map

- [`crates/ufp_ingest/doc/ucfp_ingest.md`](crates/ufp_ingest/doc/ucfp_ingest.md) – ingest invariants, metadata normalization flow, and error taxonomy.
- [`crates/ufp_canonical/doc/ufp_canonical.md`](crates/ufp_canonical/doc/ufp_canonical.md) – canonical transforms, token semantics, and checksum derivation.
- [`crates/ufp_perceptual/doc/ufp_perceptual.md`](crates/ufp_perceptual/doc/ufp_perceptual.md) – shingling/winnowing internals, MinHash tuning guidance, and performance notes.
- [`crates/ufp_semantic/doc/ufp_semantic.md`](crates/ufp_semantic/doc/ufp_semantic.md) – ONNX/API setup, deterministic stub tiers, and embedding configuration tips.
- [`crates/ufp_index/doc/ufp_index.md`](crates/ufp_index/doc/ufp_index.md) – backend configuration, query modes, and indexing strategies.
- [`crates/ufp_match/doc/ufp_match.md`](crates/ufp_match/doc/ufp_match.md) – query-time matching over `ufp_index` and multi-tenant policies.

### Config quick reference
| | Config type          | Knobs you probably care about                                                | Default highlights                              |
|---|----------------------|------------------------------------------------------------------------------|-------------------------------------------------|
| | `IngestConfig`       | `default_tenant_id`, `doc_id_namespace`, `strip_control_chars`, `metadata_policy.*`, `max_payload_bytes`, `max_normalized_bytes` | v1, default_tenant_id="default", doc_id_namespace=NAMESPACE_OID, strip-on, policies off |
| | `CanonicalizeConfig` | `normalize_unicode`, `strip_punctuation`, `lowercase`                        | v1, Unicode NFKC + lowercase, punctuation kept  |
| | `PerceptualConfig`   | `k`, `w`, `minhash_bands`, `minhash_rows_per_band`, `seed`, `use_parallel`, `include_intermediates`   | v1, 9-token shingles, 4-window winnowing, 16x8 MinHash, serial mode, intermediates included |
| | `IndexConfig`        | `backend`, `compression`, `quantization`                                    | v1, InMemory backend, zstd compression, i8 quantization |
| | `MatchConfig`        | `version`, `policy_id`, `policy_version`, `mode`, `strategy`, `max_results`, `tenant_enforce`, `oversample_factor`, `explain` | v1, policy_id="default-policy", policy_version="v1", semantic mode, 10 results, tenant isolation on, oversample x2, no explanation, strategy=Semantic(min_score=0.0) |

```rust
```rust
use ucfp::{
    CanonicalizeConfig, IngestConfig, IngestMetadata, IngestPayload, IngestSource, PerceptualConfig,
    RawIngestRecord,
};
use ucfp_index::{BackendConfig, IndexConfig};
use uuid::Uuid;

let ingest_cfg = IngestConfig {
    default_tenant_id: "tenant-acme".into(),
    doc_id_namespace: Uuid::parse_str("3ba60f64-7d5a-11ee-b962-0242ac120002")?,
    strip_control_chars: true,
    ..Default::default()
};

let canonical_cfg = CanonicalizeConfig {
    strip_punctuation: true,
    lowercase: true,
    ..Default::default()
};

let perceptual_cfg = PerceptualConfig {
    k: 7,
    minhash_bands: 32,
    minhash_rows_per_band: 4,
    use_parallel: true,
    ..Default::default()
};

let index_cfg = IndexConfig::new().with_backend(BackendConfig::InMemory);

let (doc, fingerprint) = ucfp::process_record_with_perceptual(
    RawIngestRecord {
        id: "demo".into(),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: None,
            doc_id: None,
            received_at: None,
            original_source: None,
            attributes: None,
        },
        payload: Some(IngestPayload::Text("Streamlined config demo".into())),
    },
    &canonical_cfg,
    &perceptual_cfg,
)?;
assert_eq!(doc.canonical_text, "streamlined config demo");
```

### Pipeline in code

```rust
use chrono::{Duration, Utc};
use ucfp::{
    CanonicalizeConfig, IngestMetadata, IngestPayload, IngestSource, PerceptualConfig,
    RawIngestRecord, SemanticConfig, process_record_with_perceptual, semanticize_document,
};
use ufp_index::{BackendConfig, IndexConfig, UfpIndex};

let canonical_cfg = CanonicalizeConfig::default();
let perceptual_cfg = PerceptualConfig { k: 5, ..Default::default() };
let index_cfg = IndexConfig::new().with_backend(BackendConfig::InMemory);
let index = UfpIndex::new(index_cfg)?;

let record = RawIngestRecord {
    id: "ingest-1".into(),
    source: IngestSource::RawText,
    metadata: IngestMetadata {
        tenant_id: Some("tenant".to_string()),
        doc_id: Some("doc".to_string()),
        received_at: Some(Utc::UNIX_EPOCH + Duration::days(1_700_000_000 / 86400)),
        original_source: None,
        attributes: None,
    },
    payload: Some(IngestPayload::Text("  Hello   world  ".into())),
};

let (doc, fingerprint) =
    process_record_with_perceptual(record, &canonical_cfg, &perceptual_cfg)?;
assert_eq!(doc.canonical_text, "hello world");
assert_eq!(fingerprint.meta.k, 5);

let semantic_cfg = SemanticConfig {
    mode: "fast".into(),
    tier: "fast".into(),
    ..Default::default()
};
let embedding = semanticize_document(&doc, &semantic_cfg)?;
assert_eq!(embedding.doc_id, doc.doc_id);
```

Call `process_record_with_semantic(...)` to obtain the document and embedding together, or
`semanticize_document(...)` when you already have a canonical document on hand. Once you have a
fingerprint and/or embedding, use `ufp_index::UfpIndex` to store and search them.

### Query-time matching (`ufp_match`)

Once you are writing `IndexRecord` values into `ufp_index`, use `ufp_match::DefaultMatcher` at
query time to turn free-text searches into ranked hits:

```rust
use std::sync::Arc;
use ucfp::{CanonicalizeConfig, IngestConfig, PerceptualConfig, SemanticConfig};
use ufp_index::{BackendConfig, IndexConfig, UfpIndex};
use ufp_match::{DefaultMatcher, MatchConfig, MatchExpr, MatchRequest, Matcher};

let index_cfg = IndexConfig::new().with_backend(BackendConfig::in_memory());
let index = UfpIndex::new(index_cfg).unwrap();

let matcher = DefaultMatcher::new(
    index,
    IngestConfig::default(),
    CanonicalizeConfig::default(),
    PerceptualConfig::default(),
    SemanticConfig::default(),
);

let req = MatchRequest {
    tenant_id: "tenant-a".to_string(),
    query_text: "Rust memory safety".to_string(),
    config: MatchConfig {
        strategy: MatchExpr::Weighted {
            semantic_weight: 0.7,
            min_overall: 0.3,
        },
        max_results: 10,
        tenant_enforce: true,
        oversample_factor: 2.0,
        explain: true,
        ..Default::default()
    },
    attributes: None,
    pipeline_version: None,
    fingerprint_versions: None,
    query_canonical_hash: None,
};

let hits = matcher.match_document(&req)?;
assert!(hits.len() <= req.config.max_results);

Failures bubble up as `MatchError::InvalidConfig(_)`, `MatchError::Pipeline(_)`, or
`MatchError::Index(_)`. The CLI binary in `src/main.rs` invokes `big_text_demo`
and prints the final MinHash signature generated from `crates/ufp_canonical/examples/big_text.txt`.

## Metrics & Observability

Hook a recorder into `set_pipeline_metrics(...)` to track stage-level latency and outcomes, or attach
a structured logger via `set_pipeline_logger(...)`. The `KeyValueLogger` helper emits key/value lines
such as:

```
timestamp="2025-02-10T02:15:01.234Z" stage=ingest status=success latency_us=640 record_id="demo"
timestamp="2025-02-10T02:15:01.241Z" stage=canonical status=success latency_us=488 record_id="demo" doc_id="demo"
timestamp="2025-02-10T02:15:01.245Z" stage=perceptual status=success latency_us=377 record_id="demo" doc_id="demo"
timestamp="2025-02-10T02:15:01.249Z" stage=semantic status=success latency_us=512 record_id="demo" doc_id="demo"
timestamp="2025-02-10T02:15:01.252Z" stage=index status=success latency_us=270 record_id="demo" doc_id="demo"
```

`examples/pipeline_metrics.rs` now wires both metrics and structured logging. Run it with:

```bash
cargo run --example pipeline_metrics
```

## Workspace Layout

```
crates/
  ufp_ingest/        # ingest validation and normalization
  ufp_canonical/     # canonical text pipeline
  ufp_perceptual/    # shingling, winnowing, MinHash
  ufp_semantic/      # embedding generation
  ufp_index/         # pluggable backend for search/storage
src/                 # workspace exports + CLI demo
tests/               # integration tests (determinism, errors, pipeline)
docs/                # static documentation site
proto/               # schema sketches and diagrams
examples/            # workspace-level demos (metrics, etc.)
```

## Roadmap

- Expand ingest metadata policies and validation rules.
- Add more `ufp_index` backends (e.g., Elasticsearch, managed vector DBs).
- Extend the pipeline with cross-modal canonicalizers, fingerprints, and embedding backends:

| Modality | Canonicalizer | Fingerprint | Embedding Model |
| --- | --- | --- | --- |
| Text     | NFKC + tokenization | MinHash | BGE / E5 |
| Image    | DCT normalization | pHash | CLIP / SigLIP |
| Audio    | Mel-spectrogram | Winnowing | SpeechCLIP / Whisper |
| Video    | Keyframes | Scene hashes | VideoCLIP / XCLIP |
| Document | OCR + layout | Layout graph | LayoutLMv3 |

- Introduce semantic extraction and multi-modality pathways (e.g., text + binary embeddings) feeding the existing canonical/perceptual layers.
- Enrich observability with structured logging backends and metrics exporters.

## Contributing

We welcome fixes, optimizations, and new modalities. Please read [`CONTRIBUTING.md`](CONTRIBUTING.md)
for the workflow, required checks (`cargo fmt`, `cargo clippy`, `cargo test`), documentation
expectations, and guidance on updating the architecture diagram as the pipeline evolves.
