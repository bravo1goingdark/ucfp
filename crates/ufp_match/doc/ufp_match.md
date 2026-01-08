# UCFP Match (`ufp_match`)

## Purpose

`ufp_match` is the query-time matching layer for the Universal Content
Fingerprinting pipeline. It consumes indexed fingerprints + embeddings from
`ufp_index` and exposes a small, opinionated API for finding near-duplicate or
semantically related documents in multi-tenant deployments.

Rather than dealing with raw embeddings or MinHash vectors directly, callers
construct a `MatchRequest` (tenant id + query text + `MatchConfig`).
`ufp_match` uses `ucfp` to canonicalize and fingerprint/embed the query, then
searches the index and returns ranked `MatchHit` results.

## Key Types

```rust
pub enum MatchMode {
    Semantic,
    Perceptual,
    Hybrid { semantic_weight: f32 },
}

pub struct MatchConfig {
    pub mode: MatchMode,
    pub max_results: usize,
    pub min_score: f32,
    pub tenant_enforce: bool,
    pub oversample_factor: f32,
    pub explain: bool,
}

pub struct MatchRequest {
    pub tenant_id: String,
    pub query_text: String,
    pub config: MatchConfig,
    pub attributes: Option<serde_json::Value>,
}

pub struct MatchHit {
    pub canonical_hash: String,
    pub score: f32,
    pub semantic_score: Option<f32>,
    pub perceptual_score: Option<f32>,
    pub metadata: serde_json::Value,
}

pub trait Matcher {
    fn match_document(&self, req: &MatchRequest) -> Result<Vec<MatchHit>, MatchError>;
}

pub struct DefaultMatcher { /* ... */ }
```

`DefaultMatcher` wires `ucfp` and `ufp_index` together and is suitable for
production use in most services. You can implement your own `Matcher` on top of
a different index or metric strategy if needed.

## Configuration

`MatchConfig` is built directly in Rust code and is serde-friendly for use in
JSON/TOML service configs:

```rust
use ufp_match::{MatchConfig, MatchMode};

let cfg = MatchConfig {
    mode: MatchMode::Hybrid { semantic_weight: 0.7 },
    max_results: 20,
    min_score: 0.4,
    tenant_enforce: true,
    oversample_factor: 2.0,
    explain: true,
};
```

### Tenant isolation

When `tenant_enforce` is `true` (the default), `DefaultMatcher` filters all
results so that only hits with `metadata["tenant"] == req.tenant_id` are
returned. This is critical for SaaS-style multi-tenant deployments.

### Oversampling

`oversample_factor` controls how many candidates are requested from the index:

```text
internal_top_k = ceil(max_results * oversample_factor)
```

Oversampling gives the matcher room to drop low-scoring or wrong-tenant hits
before returning the final `max_results` list.

## Scoring model

`DefaultMatcher` combines per-mode scores into a single `score` field that is
used for ranking and filtering. Given a semantic score `s` and a perceptual
score `p` (both treated as `f32`), the final score is:

- Semantic mode:

  ```text
  score = s.unwrap_or(0.0)
  ```

- Perceptual mode:

  ```text
  score = p.unwrap_or(0.0)
  ```

- Hybrid mode (`MatchMode::Hybrid { semantic_weight }`):

  ```text
  alpha = clamp(semantic_weight, 0.0, 1.0)
  s_eff = s.unwrap_or(0.0)
  p_eff = p.unwrap_or(0.0)
  score = alpha * s_eff + (1.0 - alpha) * p_eff
  ```

Scores are pure functions of the inputs and configuration: for a fixed index
state, config, and query, the same candidates will always receive the same
scores. Results are then sorted by descending `score` and truncated to
`max_results`; ties are resolved by the underlying sort implementation after
sorting by score.

## Example

```rust
use ucfp::{
    CanonicalizeConfig, IngestConfig, IngestMetadata, IngestPayload, IngestSource,
    PerceptualConfig, RawIngestRecord, SemanticConfig, process_record_with_perceptual_configs,
    process_record_with_semantic_configs,
};
use ufp_index::{BackendConfig, IndexConfig, IndexRecord, QueryMode, QueryResult, UfpIndex};
use ufp_match::{DefaultMatcher, MatchConfig, MatchMode, MatchRequest, MatchHit, Matcher};
use serde_json::json;

// Build an in-memory index and ingest a single demo document.
let ingest_cfg = IngestConfig::default();
let canonical_cfg = CanonicalizeConfig::default();
let perceptual_cfg = PerceptualConfig::default();
let semantic_cfg = SemanticConfig::default();

let raw = RawIngestRecord {
    id: "doc-1".into(),
    source: IngestSource::RawText,
    metadata: IngestMetadata {
        tenant_id: Some("tenant-a".into()),
        doc_id: Some("doc-1".into()),
        received_at: None,
        original_source: None,
        attributes: None,
    },
    payload: Some(IngestPayload::Text(
        "Rust gives you memory safety without garbage collection.".into(),
    )),
};

let (doc, fp) = process_record_with_perceptual_configs(
    raw.clone(),
    &ingest_cfg,
    &canonical_cfg,
    &perceptual_cfg,
)?;
let (_, emb) = process_record_with_semantic_configs(
    raw,
    &ingest_cfg,
    &canonical_cfg,
    &semantic_cfg,
)?;

let index_cfg = IndexConfig::new().with_backend(BackendConfig::in_memory());
let index = UfpIndex::new(index_cfg.clone())?;

// Quantize the embedding in the same way as `DefaultMatcher`.
let scale = index_cfg.quantization.scale();
let quantized: Vec<i8> = emb
    .vector
    .iter()
    .map(|v| (v * scale).clamp(-128.0, 127.0) as i8)
    .collect();

let rec = IndexRecord {
    schema_version: ufp_index::INDEX_SCHEMA_VERSION,
    canonical_hash: doc.sha256_hex.clone(),
    perceptual: Some(fp.minhash.clone()),
    embedding: Some(quantized),
    metadata: json!({
        "tenant": "tenant-a",
        "doc_id": "doc-1",
    }),
};

index.upsert(&rec)?;

let matcher = DefaultMatcher::new(
    index,
    ingest_cfg,
    canonical_cfg,
    perceptual_cfg,
    semantic_cfg,
);

let req = MatchRequest {
    tenant_id: "tenant-a".into(),
    query_text: "Rust and memory safety".into(),
    config: MatchConfig::default(),
    attributes: None,
};

let hits: Vec<MatchHit> = matcher.match_document(&req)?;
assert!(!hits.is_empty());
```

### Hybrid scoring example

```rust
let hybrid_cfg = MatchConfig {
    mode: MatchMode::Hybrid { semantic_weight: 0.8 },
    max_results: 5,
    min_score: 0.3,
    tenant_enforce: true,
    oversample_factor: 2.0,
    explain: true,
};

let hybrid_req = MatchRequest { config: hybrid_cfg, ..req };
let hybrid_hits = matcher.match_document(&hybrid_req)?;
for hit in hybrid_hits {
    println!(
        "hash={} score={} semantic={:?} perceptual={:?}",
        hit.canonical_hash, hit.score, hit.semantic_score, hit.perceptual_score,
    );
}
```

## Observability

Install a `MatchMetrics` implementation via `set_match_metrics(Some(recorder))`
to capture per-request latency and hit counts. This is a lightweight hook
intended to integrate with existing metrics systems (Prometheus, OpenTelemetry,
etc.) without coupling `ufp_match` to any specific backend.

## Testing

The crate includes unit tests for:

- `MatchConfig` validation and defaults.
- End-to-end `DefaultMatcher` behavior over an in-memory `ufp_index`:
  - Basic semantic matching.
  - Tenant isolation enforcement.
  - Metrics recording via a test `MatchMetrics` implementation.

You can run the tests with:

```bash
cargo test -p ufp_match
```

Since `DefaultMatcher::in_memory_default` uses only the in-memory backend, no
external services or RocksDB toolchain are required to execute the test suite.
