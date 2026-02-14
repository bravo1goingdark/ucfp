# UCFP Index Crate

> **Storage and search for UCFP fingerprints and embeddings**

[![API Docs](https://img.shields.io/badge/docs-api-blue)](https://docs.rs/index)
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

The `index` crate handles storage for UCFP fingerprints and embeddings. It provides quantization, compression, and search tools while staying backend-agnostic.

### What This Crate Does

| Function | Description |
|----------|-------------|
| **Storage** | Redb (disk) or in-memory backends |
| **Compression** | zstd or none, configurable at runtime |
| **Quantization** | i8 conversion for semantic vectors |
| **Search** | Full-scan and ANN (HNSW) similarity search |
| **Schema Versioning** | Safe upgrades via `INDEX_SCHEMA_VERSION` |

### Key Properties

- **Backend-agnostic**: Swap Redb, in-memory, or custom
- **Optional Redb**: Disable `backend-redb` for dependency-light builds
- **Deterministic**: Quantization produces consistent results
- **SIMD-optimized**: Fast cosine and Jaccard scoring

### Pipeline Position

```
┌─────────┐     ┌──────────┐     ┌──────────────────┐     ┌───────┐     ┌───────┐
│  Ingest │────▶│Canonical │────▶│Perceptual/Semantic│────▶│ Index │────▶│ Match │
│         │     │          │     │                  │     │(this) │     │       │
└─────────┘     └──────────┘     └──────────────────┘     └───────┘     └───────┘
```

---

## Quick Start

### Basic Index Usage

```rust
use index::{BackendConfig, IndexConfig, IndexRecord, QueryMode, UfpIndex};
use serde_json::json;

let cfg = IndexConfig::new().with_backend(BackendConfig::redb("data/index"));
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
    perceptual: Some(vec![111, 222, 333]),
    embedding: None,
    metadata: json!({}),
};
let hits = index.search(&query, QueryMode::Perceptual, 10)?;
```

### In-Memory Index

```rust
use index::{BackendConfig, IndexConfig, UfpIndex};

let in_mem = UfpIndex::new(IndexConfig::new().with_backend(BackendConfig::InMemory))?;
```

---

## Architecture

### Data Flow

```
IndexRecord (with hash, perceptual, embedding, metadata)
      │
      ▼
┌─────────────────────────────────────────┐
│              UfpIndex                   │
├─────────────────────────────────────────┤
│  1. Quantization (if embedding)        │
│     - Convert f32 to i8 deterministically│
├─────────────────────────────────────────┤
│  2. Compression (optional)              │
│     - zstd or none                      │
├─────────────────────────────────────────┤
│  3. Backend Storage                     │
│     - Redb (disk) or InMemory           │
├─────────────────────────────────────────┤
│  4. Search                              │
│     - Semantic: Cosine similarity        │
│     - Perceptual: Jaccard similarity     │
│     - ANN: HNSW for large datasets      │
└─────────────────────────────────────────┘
      │
      ▼
QueryResult (hash, score, metadata)
```

### Storage Backends

| Backend | Use Case |
|---------|----------|
| Redb (default) | Persistent disk storage |
| InMemory | Tests, ephemeral data, lambdas |

---

## Core Concepts

### IndexRecord

```rust
pub struct IndexRecord {
    pub schema_version: u16,
    pub canonical_hash: String,
    pub perceptual: Option<Vec<u64>>,  // MinHash for perceptual search
    pub embedding: Option<Vec<i8>>,     // Quantized embedding
    pub metadata: serde_json::Value,
}
```

### Query Modes

- **Semantic**: Cosine similarity on embeddings
- **Perceptual**: Jaccard similarity on MinHash

### Schema Versioning

The `INDEX_SCHEMA_VERSION` ensures safe migrations. Increment when changing the data model.

---

## Configuration

### IndexConfig

```rust
pub struct IndexConfig {
    pub backend: BackendConfig,
    pub compression: CompressionConfig,
    pub quantization: QuantizationConfig,
    pub ann: AnnConfig,
}
```

### BackendConfig

```rust
pub enum BackendConfig {
    Redb { path: String },  // Path to database file
    InMemory,
}
```

**Methods:**
- `BackendConfig::redb(path)` - Create Redb backend config
- `BackendConfig::in_memory()` - Create in-memory backend config

### CompressionConfig

```rust
pub enum CompressionCodec {
    None,
    Zstd,
}

pub struct CompressionConfig {
    pub codec: CompressionCodec,
    pub level: i32,  // 1-22 for Zstd
}
```

### QuantizationConfig

```rust
pub enum QuantizationConfig {
    Int8 { scale: f32 },  // default scale: 100.0
}
```

Converts f32 embeddings to i8 deterministically using: `quantized = (value * scale).clamp(-128.0, 127.0) as i8`

### AnnConfig

```rust
pub struct AnnConfig {
    pub enabled: bool,
    pub min_vectors_for_ann: usize,
    pub ef_construction: usize,
    pub ef_search: usize,
    pub m: usize,
}
```

**Fields:**
- `enabled`: Master switch (default: true)
- `min_vectors_for_ann`: When to switch to HNSW (default: 1000)
- `ef_construction`: Build accuracy vs speed
- `ef_search`: Query accuracy vs speed
- `m`: HNSW layer connections

---

## API Reference

### Main Types

#### UfpIndex

```rust
pub fn new(cfg: IndexConfig) -> Result<Self, IndexError>;
pub fn with_backend(cfg: IndexConfig, backend: Box<dyn IndexBackend>) -> Self;
pub fn upsert(&self, rec: &IndexRecord) -> Result<(), IndexError>;
pub fn batch_insert(&self, records: &[IndexRecord]) -> Result<(), IndexError>;
pub fn get(&self, hash: &str) -> Result<Option<IndexRecord>, IndexError>;
pub fn delete(&self, hash: &str) -> Result<(), IndexError>;
pub fn flush(&self) -> Result<(), IndexError>;
pub fn scan(&self, visitor: &mut dyn FnMut(&IndexRecord) -> Result<(), IndexError>) -> Result<(), IndexError>;
pub fn search(&self, query: &IndexRecord, mode: QueryMode, top_k: usize) 
    -> Result<Vec<QueryResult>, IndexError>;
pub fn semantic_vector_count(&self) -> usize;
pub fn should_use_ann(&self) -> bool;
pub fn rebuild_ann_if_needed(&self);
```

#### IndexBackend Trait

```rust
pub trait IndexBackend: Send + Sync {
    fn put(&self, key: &str, value: &[u8]) -> Result<(), IndexError>;
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, IndexError>;
    fn delete(&self, key: &str) -> Result<(), IndexError>;
    fn batch_put(&self, entries: Vec<(String, Vec<u8>)>) -> Result<(), IndexError>;
    fn scan(&self, visit: &mut dyn FnMut(&[u8]) -> Result<(), IndexError>) -> Result<(), IndexError>;
    fn flush(&self) -> Result<(), IndexError> { Ok(()) }
}
```

---

## Error Handling

### IndexError Variants

| Error | Trigger | Recovery |
|-------|---------|----------|
| `Backend(msg)` | Storage backend failure | Check disk space, permissions |
| `Encode(msg)` | Serialization encode error | Check record format |
| `Decode(msg)` | Serialization decode error | Check data corruption |
| `Zstd(msg)` | Compression/decompression error | Check data integrity |

---

## Examples

### Example 1: Basic CRUD

```rust
use index::{BackendConfig, IndexConfig, IndexRecord, UfpIndex};
use serde_json::json;

let index = UfpIndex::new(
    IndexConfig::new().with_backend(BackendConfig::InMemory)
)?;

let record = IndexRecord {
    schema_version: 1,
    canonical_hash: "abc123".into(),
    perceptual: Some(vec![1, 2, 3]),
    embedding: Some(vec![10, -5, 3]),
    metadata: json!({"title": "Test Doc"}),
};

index.upsert(&record)?;
let retrieved = index.get("abc123")?.unwrap();
assert_eq!(retrieved.canonical_hash, "abc123");
```

### Example 2: Semantic Search

```rust
use index::{BackendConfig, IndexConfig, IndexRecord, QueryMode, UfpIndex};

let index = UfpIndex::new(
    IndexConfig::new().with_backend(BackendConfig::InMemory)
)?;

// Insert documents with embeddings
for (hash, embedding) in documents {
    index.upsert(&IndexRecord {
        schema_version: 1,
        canonical_hash: hash,
        perceptual: None,
        embedding: Some(embedding),
        metadata: json!({}),
    })?;
}

// Search
let query = IndexRecord {
    schema_version: 1,
    canonical_hash: "".into(),
    perceptual: None,
    embedding: Some(query_embedding),
    metadata: json!({}),
};

let results = index.search(&query, QueryMode::Semantic, 10)?;
```

### Example 3: Perceptual Search

```rust
use index::{IndexRecord, QueryMode};

let query = IndexRecord {
    schema_version: 1,
    canonical_hash: "".into(),
    perceptual: Some(query_minhash),  // MinHash from perceptual stage
    embedding: None,
    metadata: json!({}),
};

let results = index.search(&query, QueryMode::Perceptual, 10)?;
```

### Example 4: ANN Search Configuration

```rust
use index::{AnnConfig, BackendConfig, IndexConfig};

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

---

## Best Practices

### 1. Use Schema Versioning

```rust
const INDEX_SCHEMA_VERSION: u16 = 1;

let record = IndexRecord {
    schema_version: INDEX_SCHEMA_VERSION,
    // ...
};
```

### 2. Batch Inserts for Large Data

```rust
index.batch_insert(&records)?;
```

### 3. Configure ANN for Large Datasets

```rust
let cfg = IndexConfig::new()
    .with_ann(AnnConfig {
        enabled: true,
        min_vectors_for_ann: 1000,  // Switch to HNSW at 1000 vectors
        ..Default::default()
    });
```

### 4. Flush Before Shutdown

```rust
index.flush()?;
```

---

## Performance

### Search Modes

| Mode | Dataset Size | Complexity |
|------|--------------|------------|
| Linear Scan | < 1000 vectors | O(n) |
| ANN (HNSW) | 1000+ vectors | O(log n) |

### Benchmarks

Typical performance on modern hardware:

| Operation | Latency |
|-----------|---------|
| Single insert | ~100 μs |
| Batch insert (1000) | ~50 ms |
| Linear search (1K vectors) | ~1 ms |
| ANN search (100K vectors) | ~10 ms |

### Optimization Tips

1. **Enable ANN** for datasets > 1000 vectors
2. **Use batch inserts** for bulk loading
3. **Quantize embeddings** to i8 for storage savings
4. **Enable compression** for large datasets

---

## Troubleshooting

### Common Issues

#### "BackendError: No such file or directory"

**Problem**: Redb database path doesn't exist.

**Solutions:**
```rust
// Create directory or use in-memory
let cfg = IndexConfig::new().with_backend(BackendConfig::InMemory);
```

#### Slow Search Performance

**Problem**: Search is taking too long.

**Solutions:**
1. Enable ANN for large datasets
2. Increase `ef_search` parameter
3. Use quantized embeddings (i8)

#### Schema Version Mismatch

**Problem**: Can't read old records after upgrade.

**Solutions:**
- Implement migration logic
- Keep schema version in sync across deployments

---

## Integration Guide

### With UCFP Pipeline

```rust
use ucfp::{process_pipeline, PipelineStageConfig};
use index::{BackendConfig, IndexConfig, IndexRecord, UfpIndex};
use serde_json::json;

// 1. Process through pipeline
let result = process_pipeline(
    raw_record,
    PipelineStageConfig::all().with_semantic(),
)?;

// 2. Create index record
let record = IndexRecord {
    schema_version: INDEX_SCHEMA_VERSION,
    canonical_hash: result.canonical.sha256_hex,
    perceptual: result.perceptual.map(|p| p.minhash),
    embedding: result.semantic.map(|s| s.vector),
    metadata: json!({"source": "uploaded"}),
};

// 3. Insert into index
let index = UfpIndex::new(IndexConfig::new().with_backend(BackendConfig::redb("data/index")))?;
index.upsert(&record)?;
```

### Custom Backend

```rust
use index::{IndexBackend, IndexConfig, UfpIndex};

struct MyBackend { /* ... */ }

impl IndexBackend for MyBackend {
    fn put(&self, key: &str, value: &[u8]) -> Result<(), IndexError> { /* ... */ }
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, IndexError> { /* ... */ }
    fn delete(&self, key: &str) -> Result<(), IndexError> { /* ... */ }
    fn batch_put(&self, entries: Vec<(String, Vec<u8>)>) -> Result<(), IndexError> { /* ... */ }
    fn scan(&self, visit: &mut dyn FnMut(&[u8]) -> Result<(), IndexError>) { /* ... */ }
    fn flush(&self) -> Result<(), IndexError> { /* ... */ }
}

let index = UfpIndex::with_backend(
    IndexConfig::new().with_backend(BackendConfig::InMemory),
    Box::new(MyBackend::new()),
);
```

---

## Testing

### Running Tests

```bash
# In-memory only (no dependencies)
cargo test -p index --no-default-features

# Full suite with Redb
cargo test -p index
```

### Test Coverage

- Serialization/deserialization
- Backend swaps
- Query correctness
- ANN fallback behavior
- Quantization determinism

### Example Programs

```bash
# Index demo
cargo run -p index --example index_demo

# Full pipeline
cargo run -p ucfp --example full_pipeline
```

---

## Feature Flags

| Feature | Description |
|---------|-------------|
| `backend-redb` *(default)* | Redb backend for disk storage |

Disable default features for purely in-memory runs:
```bash
cargo test -p index --no-default-features
```

---

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](../../LICENSE) for details.

---

## Contributing

Contributions are welcome! Please ensure:
- All tests pass: `cargo test -p index`
- Documentation is updated
- Examples are provided for new features

---

## Support

For issues and questions:
- GitHub Issues: [github.com/bravo1goingdark/ufcp/issues](https://github.com/bravo1goingdark/ufcp/issues)
- Documentation: [docs.rs/index](https://docs.rs/index)
