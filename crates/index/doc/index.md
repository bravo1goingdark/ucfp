# UCFP Index (`index`)

## Purpose

`index` is the storage layer for UCFP fingerprints + embeddings. It
provides deterministic quantization, compression, and search primitives while
remaining backend-agnostic so deployments can pick RocksDB for on-disk storage
or the fast in-memory map for ephemeral workloads (and you can still inject a
custom backend that implements the trait if you need one later).

The crate exposes a single entry point (`UfpIndex`) that accepts a runtime
[`IndexConfig`](#configuration) describing the backend, compression and
quantization strategy, then offers CRUD and similarity search operations.

## Features
- Storage options: RocksDB (default) for durable indexing or the fast
  in-memory backend when you only need ephemeral state (tests, demos, lambdas).
- RocksDB lives behind the `backend-rocksdb` feature so you can build a
  dependency-light, in-memory-only binary when native libraries are
  unavailable.
- Runtime-configurable compression (zstd or none) and quantization strategies.
- Perceptual MinHash storage with deterministic metadata.
- Schema versioning via the exported `INDEX_SCHEMA_VERSION` constant for safe migrations.
- Full-scan semantic/perceptual retrieval with SIMD-friendly cosine and fast
  Jaccard scoring (HashSet reuse avoids per-record allocations).
- ANN (Approximate Nearest Neighbor) search with HNSW for sub-linear semantic
  search on large datasets (1000+ vectors).

## Key Types

```rust
pub struct IndexRecord {
    pub schema_version: u16,
    pub canonical_hash: String,
    pub perceptual: Option<Vec<u64>>,
    pub embedding: Option<Vec<i8>>,
    pub metadata: serde_json::Value,
}

pub struct UfpIndex {
    pub fn new(cfg: IndexConfig) -> Result<Self, IndexError>;
    pub fn with_backend(cfg: IndexConfig, backend: Box<dyn IndexBackend>) -> Self;
    pub fn upsert(&self, rec: &IndexRecord) -> Result<(), IndexError>;
    pub fn batch_insert(&self, records: &[IndexRecord]) -> Result<(), IndexError>;
    pub fn get(&self, hash: &str) -> Result<Option<IndexRecord>, IndexError>;
    pub fn delete(&self, hash: &str) -> Result<(), IndexError>;
    pub fn flush(&self) -> Result<(), IndexError>;
    pub fn search(&self, query: &IndexRecord, mode: QueryMode, top_k: usize)
        -> Result<Vec<QueryResult>, IndexError>;
}

pub trait IndexBackend: Send + Sync {
    fn put(&self, key: &str, value: &[u8]) -> Result<(), IndexError>;
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, IndexError>;
    fn delete(&self, key: &str) -> Result<(), IndexError>;
    fn batch_put(&self, entries: Vec<(String, Vec<u8>)>) -> Result<(), IndexError>;
    fn scan(&self, visit: &mut dyn FnMut(&[u8]) -> Result<(), IndexError>)
        -> Result<(), IndexError>;
    fn flush(&self) -> Result<(), IndexError> { Ok(()) }
}
```

`QueryMode` covers cosine similarity for embeddings (`Semantic`) and Jaccard
similarity for MinHash (`Perceptual`). The `QueryResult` always returns the
matching canonical hash, score, and stored metadata blob.

## Quick start
```rust,ignore
use serde_json::json;
use index::{
    BackendConfig, IndexConfig, IndexRecord, QueryMode, UfpIndex, INDEX_SCHEMA_VERSION
};

let cfg = IndexConfig::new().with_backend(BackendConfig::rocksdb("data/index"));
let index = UfpIndex::new(cfg)?;

index.upsert(&IndexRecord {
    schema_version: INDEX_SCHEMA_VERSION,
    canonical_hash: "doc-1".into(),
    perceptual: Some(vec![111, 222, 333]),
    embedding: Some(vec![10, -3, 7, 5]),
    metadata: json!({"source": "guide.md"}),
})?;

let query = IndexRecord { 
    schema_version: INDEX_SCHEMA_VERSION,
    canonical_hash: "query-hash".into(),
    perceptual: Some(vec![111, 222, 333]), // or populate from your query payload
    embedding: None, // or populate semantic embedding if needed
    metadata: json!({}),
};let hits = index.search(&query, QueryMode::Perceptual, 10)?;
```

### Swapping backends
Select the backend via the builder API or by injecting your own implementation:

```rust,ignore
use index::{BackendConfig, IndexConfig, InMemoryBackend, UfpIndex};

// In-memory (tests/demos)
let in_mem = UfpIndex::new(IndexConfig::new().with_backend(BackendConfig::InMemory))?;

// Inject a custom backend instance (e.g., to reuse a shared connection pool)
let cfg = IndexConfig::new().with_backend(BackendConfig::InMemory);
let index = UfpIndex::with_backend(cfg, Box::new(InMemoryBackend::new()));
```

From the workspace root, run `cargo run -p index --example index_demo` to
see end-to-end insert + semantic/perceptual queries with the default RocksDB
backend. Switch to the fast in-memory backend by calling
`IndexConfig::with_backend(...)` in your initialization code—no config files
required.

## ANN (Approximate Nearest Neighbor) Search

UCFP implements HNSW (Hierarchical Navigable Small World) for efficient approximate nearest neighbor search on large vector collections:

### HNSW Integration

The index automatically switches between linear scan and HNSW based on collection size:

- **Small datasets (< 1000 vectors)**: Linear scan for exact results with minimal overhead
- **Large datasets (1000+ vectors)**: HNSW for sub-linear O(log n) query complexity
- **Automatic fallback**: Seamlessly returns to linear scan if HNSW is disabled or fails

### Runtime Configurability

All ANN parameters are runtime-configurable through `IndexConfig`:

```rust
use index::{AnnConfig, BackendConfig, IndexConfig, UfpIndex};

let cfg = IndexConfig::new()
    .with_backend(BackendConfig::rocksdb("data/index"))
    .with_ann(AnnConfig {
        enabled: true,                    // Enable/disable ANN entirely
        min_vectors_for_ann: 1000,        // Auto-switch threshold
        ef_construction: 100,             // HNSW build-time accuracy
        ef_search: 50,                    // HNSW query-time accuracy
        m: 16,                            // HNSW layer count
    });

let index = UfpIndex::new(cfg)?;
```

- `enabled`: Master switch for ANN (default: `true`)
- `min_vectors_for_ann`: Threshold for automatic HNSW activation (default: 1000)
- `ef_construction`: Higher = more accurate index, slower build (default: 100)
- `ef_search`: Higher = more accurate search, slower queries (default: 50)
- `m`: Number of bi-directional links per node (default: 16)

### Automatic Linear Scan Fallback

When ANN is disabled or the dataset is small, the index automatically uses linear scan:

- Exact cosine similarity computation
- Deterministic ordering for consistent pagination
- Zero memory overhead from HNSW structures
- SIMD-optimized distance calculations

### Configuring in IndexConfig

```rust
use index::{AnnConfig, BackendConfig, CompressionConfig, IndexConfig};

let cfg = IndexConfig::new()
    .with_backend(BackendConfig::rocksdb("data/index"))
    .with_compression(CompressionConfig::zstd())
    .with_ann(AnnConfig {
        enabled: true,
        min_vectors_for_ann: 500,         // Use HNSW for 500+ vectors
        ef_construction: 128,
        ef_search: 64,
        m: 24,
    });
```

**Runtime updates**: Modify ANN configuration without restart via the control plane. Changes apply to new queries while existing HNSW indices remain valid.

## Architecture at a glance
- **Data model:** `IndexRecord` captures the canonical hash, optional perceptual
  MinHash vector, optional quantized embedding, and an arbitrary JSON metadata
  blob. The metadata is serialized as raw JSON bytes so additions do not require
  schema migrations.
- **Entry point:** `UfpIndex` owns a selected backend and exposes CRUD + search
  via `upsert`, `batch_insert`, `get`, `delete`, `flush`, and `search`.
- **Runtime wiring:** `IndexConfig` describes the backend, compression, and
  quantization strategy. Clone it when you need to keep the same knobs in
  application state or mirror them inside background workers.
- **Storage abstraction:** The `IndexBackend` trait isolates persistence so you
  can pick RocksDB or the in-memory map today (and keep the door open for
  bespoke implementations later). Each backend only needs to implement six
  methods.
- **Compression + quantization:** `CompressionConfig` (currently none/zstd)
  shrinks serialized payloads before they hit the backend; `QuantizationConfig`
  performs deterministic `i8` conversion on semantic vectors so cosine scores
  behave the same regardless of hardware.
- **Query engine:** `QueryMode::Semantic` runs cosine similarity over
  quantized embeddings; `QueryMode::Perceptual` runs Jaccard similarity over
  MinHash shingles using scratch `HashSet`s that are reused across records for
  allocation-free scans. Ties are broken lexicographically for deterministic
  paging.

## Working with the upper layer (`ucfp`)
The workspace root crate (`ucfp`) orchestrates ingest, canonical, perceptual,
and semantic stages. `index` is the persistence/search layer that sits
behind those stages:

1. `ucfp::process_record_with_perceptual_configs` runs ingest +
   canonicalization + perceptual fingerprinting and returns the canonical
   document plus its MinHash values.
2. `ucfp::semanticize_document` (or `process_record_with_semantic_configs`)
   consumes that canonical document to produce a semantic embedding.
3. The resulting structures are converted into an `IndexRecord` and written via
   `UfpIndex::upsert`.
4. When serving lookups or dedupe checks, `ucfp` builds a partial `IndexRecord`
   (usually just perceptual hashes or a quantized embedding) and calls
   `UfpIndex::search` in the desired `QueryMode`.

### Write path (ingest ➜ index)
- `RawIngestRecord` enters the pipeline through `ucfp`.
- After canonical/perceptual processing, capture the canonical hash
  (`CanonicalizedDocument::sha256_hex`) and MinHash vector
  (`PerceptualFingerprint::minhash`).
- Produce a semantic embedding via `semanticize_document` and quantize it with
  `UfpIndex::quantize_with_strategy` so the write path never stores full `f32`
  vectors.
- Persist everything with `index.upsert`, attaching any tenant/user metadata as
  JSON so higher-level services can perform authorization or filtering without
  another lookup.

### Read path (index ➜ upper layer)
- Build a query `IndexRecord` that mirrors the modality you care about (provide
  just `perceptual` or just `embedding`).
- Choose `QueryMode::Perceptual` for near-duplicate detection or
  `QueryMode::Semantic` for semantic similarity search.
- The upper layer merges `QueryResult` metadata with its own domain objects
  (e.g., fetches full documents, triggers alerts, or shows UI previews).
- Because backends share the same trait, you can run the exact same read path
  against in-memory, embedded, or remote stores depending on the deployment
  tier.

### Example: wiring the pipeline output
```rust,ignore
use ndarray::Array1;
use serde_json::json;
use ucfp::{
    CanonicalizeConfig, IngestConfig, PerceptualConfig, SemanticConfig,
    RawIngestRecord, PipelineError,
    process_record_with_perceptual_configs, semanticize_document,
};
use index::{
    BackendConfig, IndexConfig, IndexRecord, QueryMode, UfpIndex, INDEX_SCHEMA_VERSION,
};

fn upsert_pipeline_record(
    index: &UfpIndex,
    index_cfg: &IndexConfig,
    raw: RawIngestRecord,
    ingest_cfg: &IngestConfig,
    canonical_cfg: &CanonicalizeConfig,
    perceptual_cfg: &PerceptualConfig,
    semantic_cfg: &SemanticConfig,
) -> Result<(), PipelineError> {
    let tenant = raw.metadata.tenant_id.clone();
    let source = raw.source.clone();
    let (doc, fingerprint) = process_record_with_perceptual_configs(
        raw,
        ingest_cfg,
        canonical_cfg,
        perceptual_cfg,
    )?;
    let embedding = semanticize_document(&doc, semantic_cfg)?;

    let quantized = UfpIndex::quantize_with_strategy(
        &Array1::from(embedding.vector.clone()),
        &index_cfg.quantization,
    );

    let record = IndexRecord {
        schema_version: INDEX_SCHEMA_VERSION,
        canonical_hash: doc.sha256_hex.clone(),
        perceptual: Some(fingerprint.minhash.clone()),
        embedding: Some(quantized),
        metadata: json!({
            "tenant": tenant,
            "doc_id": doc.doc_id,
            "model": embedding.model_name,
            "tier": embedding.tier,
            "source": source,
        }),
    };

    index.upsert(&record)?;
    Ok(())
}

// Later, to surface candidates inside an API handler:
let hits = index.search(&query_record, QueryMode::Perceptual, 10)?;
```

This pattern keeps the upper layer focused on pipeline orchestration and
business logic while `index` handles storage details, compression,
quantization, and query semantics in a single place.

For a runnable walkthrough, run `cargo run -p ucfp --example full_pipeline` from the
workspace root; it wires ingest + canonical + perceptual + semantic stages
directly into the in-memory backend and prints both semantic and perceptual
matches.

## Feature Flags

| Feature | Enables |
| --- | --- |
| `backend-rocksdb` *(default)* | RocksDB backend (requires libclang at build). |

Disable default features (`--no-default-features`) to run purely in-memory
without pulling in RocksDB or its libclang toolchain.

## Testing

```bash
# In-memory only (no RocksDB/libclang needed)
cargo test -p index --no-default-features

# Full suite with RocksDB enabled
cargo test -p index
```

Unit tests cover serialization roundtrips, backend swaps, and query correctness.
Integration tests/examples exercise both in-memory and RocksDB paths; enable the
default feature set when you want parity with production deployments.
