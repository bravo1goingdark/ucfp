# UCFP Index (`index`)

## What this is

`index` handles storage for UCFP fingerprints and embeddings. It gives you quantization, compression, and search tools while staying backend-agnostic - pick Redb for disk storage or use a fast in-memory map for ephemeral stuff. Or roll your own backend if you need something custom.

Main entry point is `UfpIndex`. Give it an `IndexConfig` describing what backend you want, compression, and quantization, then do CRUD and similarity searches.

## What's included

- **Storage backends**: Redb (default, on-disk) or in-memory for tests/lambda-style stuff
- **Redb is optional**: Disable `backend-redb` for dependency-light builds
- **Compression**: zstd or none, configurable at runtime
- **Quantization**: i8 conversion for semantic vectors, deterministic across runs
- **MinHash storage** for perceptual fingerprints
- **Schema versioning** via `INDEX_SCHEMA_VERSION` for safe upgrades
- **Full-scan search** with SIMD cosine and fast Jaccard scoring
- **ANN search** using HNSW for large datasets (1000+ vectors)

## Key types

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

`QueryMode` has two flavors: `Semantic` for cosine similarity on embeddings, and `Perceptual` for Jaccard on MinHash. `QueryResult` gives you the matching hash, score, and any stored metadata.

## Quick start

```rust,ignore
use serde_json::json;
use index::{
    BackendConfig, IndexConfig, IndexRecord, QueryMode, UfpIndex, INDEX_SCHEMA_VERSION
};

let cfg = IndexConfig::new().with_backend(BackendConfig::redb("data/index"));
let index = UfpIndex::new(cfg)?;

index.upsert(&IndexRecord {
    schema_version: INDEX_SCHEMA_VERSION,
    canonical_hash: "doc-1".into(),
    perceptual: Some(vec![111, 222, 333]),
    embedding: Some(vec![10, -3, 7, 5]),
    metadata: json!({"source": "guide.md"}),
)?;

let query = IndexRecord { 
    schema_version: INDEX_SCHEMA_VERSION,
    canonical_hash: "query-hash".into(),
    perceptual: Some(vec![111, 222, 333]),
    embedding: None,
    metadata: json!({}),
};
let hits = index.search(&query, QueryMode::Perceptual, 10)?;
```

### Swapping backends

```rust,ignore
use index::{BackendConfig, IndexConfig, InMemoryBackend, UfpIndex};

// In-memory for tests/demos
let in_mem = UfpIndex::new(IndexConfig::new().with_backend(BackendConfig::InMemory))?;

// Bring your own backend (e.g., shared connection pool)
let cfg = IndexConfig::new().with_backend(BackendConfig::InMemory);
let index = UfpIndex::with_backend(cfg, Box::new(InMemoryBackend::new()));
```

Run `cargo run -p index --example index_demo` to see insert + queries in action. Default is Redb; swap to in-memory with `IndexConfig::with_backend(...)`.

## ANN (Approximate Nearest Neighbor) Search

We use HNSW for efficient vector search on large collections:

### How it works

- **Small datasets (< 1000 vectors)**: Linear scan, exact results
- **Large datasets (1000+ vectors)**: HNSW, O(log n) queries
- **Auto fallback**: Falls back to linear scan if ANN is disabled

### Configure it

```rust
use index::{AnnConfig, BackendConfig, IndexConfig, UfpIndex};

let cfg = IndexConfig::new()
    .with_backend(BackendConfig::redb("data/index"))
    .with_ann(AnnConfig {
        enabled: true,
        min_vectors_for_ann: 1000,
        ef_construction: 100,
        ef_search: 50,
        m: 16,
    });

let index = UfpIndex::new(cfg)?;
```

- `enabled`: Master switch (default: true)
- `min_vectors_for_ann`: When to switch to HNSW (default: 1000)
- `ef_construction`: Build accuracy vs speed tradeoff
- `ef_search`: Query accuracy vs speed tradeoff
- `m`: HNSW layer connections

### Linear scan fallback

When ANN is off or dataset is small:

- Exact cosine similarity
- Deterministic ordering for pagination
- No HNSW memory overhead
- SIMD-optimized distances

### Full config example

```rust
use index::{AnnConfig, BackendConfig, CompressionConfig, IndexConfig};

let cfg = IndexConfig::new()
    .with_backend(BackendConfig::redb("data/index"))
    .with_compression(CompressionConfig::zstd())
    .with_ann(AnnConfig {
        enabled: true,
        min_vectors_for_ann: 500,
        ef_construction: 128,
        ef_search: 64,
        m: 24,
    });
```

Update ANN config at runtime via control plane. Changes apply to new queries; existing HNSW indices stay valid.

## How it all fits together

- **Data model**: `IndexRecord` has canonical hash, optional MinHash, optional quantized embedding, and a JSON metadata blob. Metadata is raw JSON bytes so you can add fields without migrations.
- **Entry point**: `UfpIndex` owns the backend, exposes CRUD + search.
- **Config**: `IndexConfig` describes backend, compression, quantization. Clone it to share settings across workers.
- **Storage abstraction**: `IndexBackend` trait lets you swap implementations. Six methods to implement.
- **Compression**: `CompressionConfig` (none/zstd) shrinks data before storage. `QuantizationConfig` converts semantic vectors to i8 deterministically.
- **Query engine**: `QueryMode::Semantic` does cosine on embeddings. `QueryMode::Perceptual` does Jaccard on MinHash. Ties break lexicographically for consistent pagination.

## Using with the upper layer (`ucfp`)

`ucfp` (workspace root crate) runs ingest, canonical, perceptual, semantic stages. `index` is the persistence layer behind them:

1. `ucfp::process_perceptual` → canonical doc + MinHash
2. `ucfp::process_semantic` → canonical doc + semantic embedding
3. Convert to `IndexRecord` and `upsert`
4. Build partial record for lookups and `search`

### Write path

```rust,ignore
// After processing with ucfp pipeline...
let quantized = UfpIndex::quantize_with_strategy(
    &Array1::from(embedding.vector.clone()),
    &index_cfg.quantization,
);

let record = IndexRecord {
    schema_version: INDEX_SCHEMA_VERSION,
    canonical_hash: doc.sha256_hex.clone(),
    perceptual: Some(fingerprint.minhash.clone()),
    embedding: Some(quantized),
    metadata: json!({ /* your metadata */ }),
};

index.upsert(&record)?;
```

### Read path

Build a query `IndexRecord` with just the modality you need (perceptual OR embedding), pick your `QueryMode`, and get `QueryResult`s back.

## Feature flags

| Feature | What it does |
|---------|--------------|
| `backend-redb` *(default)* | Redb backend (pure Rust) |

Disable default features for purely in-memory runs: `cargo test -p index --no-default-features`

## Tests

```bash
# In-memory only
cargo test -p index --no-default-features

# Full suite with Redb
cargo test -p index
```

Unit tests cover serialization, backend swaps, and query correctness. Integration tests exercise both paths.

Runnable example: `cargo run -p ucfp --example full_pipeline`
