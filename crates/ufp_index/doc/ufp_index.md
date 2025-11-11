# UCFP Index (`ufp_index`)

## Purpose

`ufp_index` is the storage layer for UCFP fingerprints + embeddings. It
provides deterministic quantization, compression, and search primitives while
remaining backend-agnostic so deployments can pick RocksDB, Redis, Postgres,
MongoDB, pure Rust KV stores (redb/sled), in-memory maps, or dynamically loaded
plugins.

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
| `Redis { url, namespace }` | Remote cache / distributed store (feature `backend-redis`). |
| `Postgres { dsn, table }` | SQL backend (feature `backend-postgres`), automatically creates the table if missing. |
| `Mongo { uri, database, collection }` | Document store (feature `backend-mongo`). |
| `Redb { path }` | Pure Rust B+tree storage, no FFI (feature `backend-redb`). |
| `Sled { path }` | Another pure-Rust embedded KV store (feature `backend-sled`). |
| `Plugin { library_path, symbol, config }` | Dynamically load a backend via `libloading` (`plugin-loader` feature) calling an `extern "C"` constructor. |

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

#### Redis
- Enable feature: `--features backend-redis`.
- Start Redis locally or supply `url` for hosted service.
- Optional namespace isolates keys per deployment.
  ```rust
  let cfg = IndexConfig::new()
      .with_backend(BackendConfig::redis("redis://localhost:6379", "ucfp"));
  ```
- Recommended: configure persistence (`appendonly yes`) so fingerprints survive restarts.

#### Postgres
- Enable feature: `--features backend-postgres`.
- Requires a Postgres instance (local or managed) with `pgvector` optional for advanced similarity queries outside this crate.
- `dsn` example: `postgres://user:pass@localhost:5432/ucfp`.
- Table auto-created: `CREATE TABLE IF NOT EXISTS "<table>" (key TEXT PRIMARY KEY, value BYTEA)`.
- Configure via:
  ```rust
  let cfg = IndexConfig::new()
      .with_backend(BackendConfig::postgres("postgres://user:pass@localhost/ucfp", "fingerprints"));
  ```

#### MongoDB
- Enable feature: `--features backend-mongo`.
- Works with local `mongod`, MongoDB Atlas, or DocumentDB (compatible API).
- Provide `uri`, `database`, and `collection` names. Documents are `{ key, value: Binary }`.
  ```rust
  let cfg = IndexConfig::new().with_backend(BackendConfig::mongo(
      "mongodb://localhost:27017",
      "ucfp",
      "fingerprints",
  ));
  ```

#### redb (pure Rust)
- Enable feature: `--features backend-redb`.
- Path points to a file (created if missing). Good for environments without libclang.
- Example:
  ```rust
  let cfg = IndexConfig::new().with_backend(BackendConfig::redb("data/ufp.redb"));
  ```

#### sled (pure Rust)
- Enable feature: `--features backend-sled`.
- Path is a directory for sled tree files.
- Useful for tests / embedded deployments; supports crash recovery out of the box.
  ```rust
  let cfg = IndexConfig::new().with_backend(BackendConfig::sled("data/sled-index"));
  ```

#### In-memory
- Feature-free, no persistence.
- Use for CI, fuzzing, or ephemeral demos.
  ```rust
  let cfg = IndexConfig::new().with_backend(BackendConfig::in_memory());
  ```

#### Plugin loader
- Enable feature: `--features plugin-loader`.
- Build a shared library exposing `extern "C" fn create_backend(json: *const c_char) -> *mut dyn IndexBackend`.
- Configure in code:
  ```rust
  use serde_json::json;

  let cfg = IndexConfig::new().with_backend(BackendConfig::plugin(
      "./libcustom_backend.so",
      "create_backend",
      json!({ "endpoint": "https://api.example.com" }),
  ));
  ```
- The JSON payload is passed verbatim to the plugin constructor for custom settings.

All backends implement the same trait, so you can inject your own via
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

### Plugin backend

1. Build a shared library exposing:
   ```rust
   #[no_mangle]
   pub extern "C" fn create_backend(config_json: *const c_char) -> *mut dyn IndexBackend { ... }
   ```
2. Configure:
   ```rust
   use serde_json::json;

   let cfg = IndexConfig::new().with_backend(BackendConfig::plugin(
       "./libmy_backend.so",
       "create_backend",
       json!({ "replicas": 3 }),
   ));
   ```

## Feature Flags

| Feature | Enables |
| --- | --- |
| `backend-rocksdb` *(default)* | RocksDB backend (requires libclang at build). |
| `backend-redis` | Redis backend via `redis` crate. |
| `backend-postgres` | Postgres backend using `postgres` crate. |
| `backend-mongo` | MongoDB backend (sync driver). |
| `backend-redb` | Pure-Rust redb backend. |
| `backend-sled` | Pure-Rust sled backend. |
| `plugin-loader` | Dynamic backend loading via `libloading`. |

Disable default features (`--no-default-features`) to avoid RocksDB/FFI
dependencies when using pure-Rust or remote backends in constrained
environments.

## Testing

```bash
# Pure Rust (no RocksDB/libclang needed)
cargo test -p ufp_index --no-default-features --features backend-sled

# Full suite with RocksDB
cargo test -p ufp_index
```

Unit tests cover serialization roundtrips, backend swaps, and query correctness.
Integration tests/examples can be run per backend once the corresponding service
is available (Redis/Postgres/Mongo).

## Integration

`ufp_index` sits after `ufp_perceptual` and `ufp_semantic`: ingest documents,
canonicalize + fingerprint them, build an `IndexRecord`, and persist via
`UfpIndex`. Downstream services like `ufp_match` or REST/gRPC APIs can then call
`search` to retrieve near-duplicate candidates based on either quantized
embeddings or perceptual MinHash signatures.
