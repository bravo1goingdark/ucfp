# ufp_index

`ufp_index` provides a backend-agnostic index that stores canonical hashes,
perceptual MinHash signatures, and quantized semantic embeddings for UCFP
pipelines.

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

## Quick start
```rust,ignore
use serde_json::json;
use ufp_index::{
    BackendConfig, IndexConfig, IndexRecord, QueryMode, UfpIndex, INDEX_SCHEMA_VERSION
};

let cfg = IndexConfig::new().with_backend(BackendConfig::rocksdb("data/ufp_index"));
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
use ufp_index::{BackendConfig, IndexConfig, InMemoryBackend, UfpIndex};

// In-memory (tests/demos)
let in_mem = UfpIndex::new(IndexConfig::new().with_backend(BackendConfig::InMemory))?;

// Inject a custom backend instance (e.g., to reuse a shared connection pool)
let cfg = IndexConfig::new().with_backend(BackendConfig::InMemory);
let index = UfpIndex::with_backend(cfg, Box::new(InMemoryBackend::new()));
```

From the workspace root, run `cargo run -p ufp_index --example index_demo` to
see end-to-end insert + semantic/perceptual queries with the default RocksDB
backend. Switch to the fast in-memory backend by calling
`IndexConfig::with_backend(...)` in your initialization code—no config files
required.

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
and semantic stages. `ufp_index` is the persistence/search layer that sits
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
use ufp_index::{
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
business logic while `ufp_index` handles storage details, compression,
quantization, and query semantics in a single place.

For a runnable walkthrough, run `cargo run -p ucfp --example full_pipeline` from the
workspace root; it wires ingest + canonical + perceptual + semantic stages
directly into the in-memory backend and prints both semantic and perceptual
matches.
