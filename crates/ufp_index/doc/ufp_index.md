# UCFP Index (`ufp_index`)

## Purpose

`ufp_index` is the storage layer for UCFP fingerprints + embeddings. It
provides deterministic quantization, compression, and search primitives while
remaining backend-agnostic so deployments can pick RocksDB for on-disk storage
or the fast in-memory map for ephemeral workloads (and you can still inject a
custom backend that implements the trait if you need one later).

The crate exposes a single entry point (`UfpIndex`) that accepts a runtime
[`IndexConfig`](#configuration) describing the backend, compression and
quantization strategy, then offers CRUD and similarity search operations.

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

## Configuration

`IndexConfig` is configured entirely through Rust builders -- no TOML/JSON/env
parsing required:

```rust
let cfg = IndexConfig::new()
    .with_backend(BackendConfig::rocksdb("data/ufp_index"))
    .with_compression(CompressionConfig::default().with_level(5))
    .with_quantization(QuantizationConfig::default().with_scale(80.0));
```

Construct or tweak the config wherever you initialize `UfpIndex` and pass it
directly to `UfpIndex::new(cfg)`.

### Backend options (`BackendConfig`)

| Variant | Notes |
| --- | --- |
| `RocksDb { path }` | Default LSM store (requires `backend-rocksdb` feature; libclang dependency). |
| `InMemory` | Non-persistent `HashMap`, great for tests/demos. |

### Backend setup guide

#### RocksDB (default)
- Enable feature: default (`backend-rocksdb`).
- Host requirements: LLVM/Clang accessible via `clang.dll` / `libclang.so`.
  - On Windows install LLVM and set `LIBCLANG_PATH=C:\Program Files\LLVM\bin`.
  - On Linux/macOS ensure `libclang` is on the library path or install via package manager.
- Configure in code:
  ```rust
  let cfg = IndexConfig::new()
      .with_backend(BackendConfig::rocksdb("data/ufp_index"))
      .with_compression(CompressionConfig::default());
  ```

#### In-memory
- Feature-free, no persistence.
- Use for CI, fuzzing, or ephemeral demos.
  ```rust
  let cfg = IndexConfig::new().with_backend(BackendConfig::in_memory());
  ```


Both built-in backends implement the same trait, so you can inject your own via
`UfpIndex::with_backend` (e.g., to reuse an existing connection pool).

### Compression (`CompressionConfig`)

```rust
#[serde(rename_all = "lowercase")]
pub enum CompressionCodec {
    None,
    Zstd,
}
```

`CompressionConfig` also carries a zstd level (default = 3). Payloads are
compressed before writing to the backend and decompressed when scanning /
reading, independent of the backend’s own compression features.

### Quantization (`QuantizationConfig`)

Currently supports a single deterministic strategy:

```rust
QuantizationConfig::Int8 { scale: f32 }
```

`UfpIndex::quantize_with_strategy` applies the configured scale and clamps to
`[-128, 127]`, producing compact `Vec<i8>` embeddings that stay consistent across
hardware. Additional quantizers can plug in later without changing the API.

## Examples

### RocksDB (default)

```rust
use serde_json::json;
use ufp_index::{
    BackendConfig, IndexConfig, IndexRecord, QueryMode, UfpIndex, INDEX_SCHEMA_VERSION,
};

let cfg = IndexConfig::new().with_backend(BackendConfig::rocksdb("data/ufp_index"));
let index = UfpIndex::new(cfg)?;

index.upsert(&IndexRecord {
    schema_version: INDEX_SCHEMA_VERSION,
    canonical_hash: "doc-1".into(),
    perceptual: Some(vec![111, 222, 333]),
    embedding: Some(vec![10, -3, 7]),
    metadata: json!({"source": "guide.md"}),
})?;

let query = IndexRecord {
    schema_version: INDEX_SCHEMA_VERSION,
    canonical_hash: "query".into(),
    perceptual: Some(vec![222, 333]),
    embedding: Some(vec![9, -2, 6]),
    metadata: json!({}),
};

let hits = index.search(&query, QueryMode::Semantic, 5)?;
```

### In-memory backend (tests / CI)

```rust
use ufp_index::{BackendConfig, IndexConfig, InMemoryBackend, UfpIndex};

let cfg = IndexConfig::new().with_backend(BackendConfig::in_memory());
let index = UfpIndex::new(cfg)?;
// or: UfpIndex::with_backend(cfg, Box::new(InMemoryBackend::new()));
```

## Feature Flags

| Feature | Enables |
| --- | --- |
| `backend-rocksdb` *(default)* | RocksDB backend (requires libclang at build). |
Disable default features (`--no-default-features`) to run purely in-memory
without pulling in RocksDB or its libclang toolchain.

## Testing

```bash
# In-memory only (no RocksDB/libclang needed)
cargo test -p ufp_index --no-default-features

# Full suite with RocksDB enabled
cargo test -p ufp_index
```

Unit tests cover serialization roundtrips, backend swaps, and query correctness.
Integration tests/examples exercise both in-memory and RocksDB paths; enable the
default feature set when you want parity with production deployments.

## Integration

`ufp_index` sits after `ufp_perceptual` and `ufp_semantic`: ingest documents,
canonicalize + fingerprint them, build an `IndexRecord`, and persist via
`UfpIndex`. Downstream services like `ufp_match` or REST/gRPC APIs can then call
`search` to retrieve near-duplicate candidates based on either quantized
embeddings or perceptual MinHash signatures.
