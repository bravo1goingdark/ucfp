# UCFP Match (`match`)

## Purpose

`match` is the query-time matching layer for the Universal Content
Fingerprinting pipeline. It consumes indexed fingerprints + embeddings from
`index` and exposes a small, opinionated API for finding near-duplicate or
semantically related documents in multi-tenant deployments.

Rather than dealing with raw embeddings or MinHash vectors directly, callers
construct a `MatchRequest` (tenant id + query text + `MatchConfig`).
`match` uses the upstream pipeline crates (`ingest`, `canonical`,
`perceptual`, `semantic`) to canonicalize and fingerprint/embed the query,
then searches the index and returns ranked `MatchHit` results.

## Architecture

`match` sits at the top of the linear dependency chain:

```
ingest → canonical → perceptual/semantic → index → match
```

It directly depends on:
- `ingest` - For ingest configuration and raw record types
- `canonical` - For text canonicalization
- `perceptual` - For perceptual fingerprint generation
- `semantic` - For semantic embedding generation
- `index` - For storage and similarity search

This linear architecture ensures no circular dependencies and clear separation
of concerns.

## Key Types

```rust
pub enum MatchMode {
    Semantic,
    Perceptual,
    Hybrid,
}

pub enum MatchExpr {
    Exact,
    Semantic { metric: MetricId, min_score: f32 },
    Perceptual { metric: MetricId, min_score: f32 },
    Weighted { semantic_weight: f32, min_overall: f32 },
    And { left: Box<MatchExpr>, right: Box<MatchExpr> },
    Or { left: Box<MatchExpr>, right: Box<MatchExpr> },
}

pub enum MetricId {
    Cosine,
    Jaccard,
    Hamming,
}

pub struct MatchConfig {
    pub version: String,
    pub policy_id: String,
    pub policy_version: String,
    pub mode: MatchMode,
    pub strategy: MatchExpr,
    pub max_results: usize,
    pub tenant_enforce: bool,
    pub oversample_factor: f32,
    pub explain: bool,
}

pub struct MatchRequest {
    pub tenant_id: String,
    pub query_text: String,
    pub config: MatchConfig,
    pub attributes: Option<serde_json::Value>,
    pub pipeline_version: Option<String>,
    pub fingerprint_versions: Option<HashMap<String, String>>,
    pub query_canonical_hash: Option<String>,
}

pub struct MatchHit {
    pub canonical_hash: String,
    pub score: f32,
    pub semantic_score: Option<f32>,
    pub perceptual_score: Option<f32>,
    pub exact_score: Option<f32>,
    pub metadata: serde_json::Value,
    pub match_version: String,
    pub policy_id: String,
    pub policy_version: String,
    pub explanation: Option<MatchExplanation>,
}

pub struct MatchExplanation {
    pub semantic_distance: Option<f32>,
    pub perceptual_overlap: Option<f32>,
    pub token_overlap: Option<f32>,
}

pub trait Matcher {
    fn match_document(&self, req: &MatchRequest) -> Result<Vec<MatchHit>, MatchError>;
}

pub struct DefaultMatcher { /* ... */ }
```

`DefaultMatcher` wires the individual pipeline crates (`ingest`, 
`canonical`, `perceptual`, `semantic`) and `index` together
and is suitable for production use in most services. You can implement your 
own `Matcher` on top of a different index or metric strategy if needed.

## Error Types

```rust
pub enum MatchError {
    InvalidConfig(String),
    Ingest(String),
    Canonical(String),
    Perceptual(String),
    Semantic(String),
    Pipeline(String),
    Index(#[from] IndexError),
}
```

The error variants now explicitly track which pipeline stage failed:
- `Ingest` - Error from `ingest`
- `Canonical` - Error from `canonical`
- `Perceptual` - Error from `perceptual`
- `Semantic` - Error from `semantic`
- `Index` - Error from `index`

## Configuration

`MatchConfig` is built directly in Rust code and is serde-friendly for use in
JSON/TOML service configs:

```rust
use match::{MatchConfig, MatchMode, MatchExpr, MetricId};

let cfg = MatchConfig {
    version: "v1".into(),
    policy_id: "default-policy".into(),
    policy_version: "v1".into(),
    mode: MatchMode::Hybrid,
    strategy: MatchExpr::Weighted {
        semantic_weight: 0.7,
        min_overall: 0.3,
    },
    max_results: 20,
    tenant_enforce: true,
    oversample_factor: 2.0,
    explain: true,
};
```

### MatchExpr Strategy

The `MatchExpr` enum provides declarative control over matching logic:

- `Exact` - Exact identity match on canonical hash
- `Semantic { metric, min_score }` - Pure semantic similarity with threshold
- `Perceptual { metric, min_score }` - Pure perceptual similarity with threshold  
- `Weighted { semantic_weight, min_overall }` - Weighted combination of semantic
  and perceptual scores (semantic_weight in [0.0, 1.0])
- `And { left, right }` - Logical conjunction of two sub-strategies
- `Or { left, right }` - Logical disjunction of two sub-strategies

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

- Hybrid mode with `MatchExpr::Weighted { semantic_weight, min_overall }`:

  ```text
  alpha = clamp(semantic_weight, 0.0, 1.0)
  s_eff = s.unwrap_or(0.0)
  p_eff = p.unwrap_or(0.0)
  score = alpha * s_eff + (1.0 - alpha) * p_eff
  ```

- `And` mode: minimum of both sub-strategy scores
- `Or` mode: maximum of both sub-strategy scores

Scores are pure functions of the inputs and configuration: for a fixed index
state, config, and query, the same candidates will always receive the same
scores. Results are then sorted by descending `score` and truncated to
`max_results`.

## Example

```rust
use ingest::{IngestConfig, IngestMetadata, IngestPayload, IngestSource, RawIngestRecord};
use canonical::CanonicalizeConfig;
use perceptual::PerceptualConfig;
use semantic::SemanticConfig;
use index::{BackendConfig, IndexConfig, IndexRecord, UfpIndex, INDEX_SCHEMA_VERSION};
use match::{DefaultMatcher, MatchConfig, MatchMode, MatchRequest, MatchHit, Matcher};
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

// Use the matcher to run the pipeline internally
let index_cfg = IndexConfig::new().with_backend(BackendConfig::in_memory());
let index = UfpIndex::new(index_cfg.clone())?;

// Note: In a real scenario, you'd run the pipeline and upsert the document
// For this example, we assume the index is already populated

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
    pipeline_version: None,
    fingerprint_versions: None,
    query_canonical_hash: None,
};

let hits: Vec<MatchHit> = matcher.match_document(&req)?;
assert!(!hits.is_empty());
```

### Hybrid scoring example

```rust
use match::{MatchConfig, MatchMode, MatchExpr, MetricId};

let hybrid_cfg = MatchConfig {
    version: "v1".into(),
    policy_id: "hybrid-policy".into(),
    policy_version: "v1".into(),
    mode: MatchMode::Hybrid,
    strategy: MatchExpr::Weighted {
        semantic_weight: 0.8,
        min_overall: 0.3,
    },
    max_results: 5,
    tenant_enforce: true,
    oversample_factor: 2.0,
    explain: true,
};

let hybrid_req = MatchRequest { 
    config: hybrid_cfg, 
    tenant_id: "tenant-a".into(),
    query_text: "Rust safety".into(),
    attributes: None,
    pipeline_version: None,
    fingerprint_versions: None,
    query_canonical_hash: None,
};

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
etc.) without coupling `match` to any specific backend.

## Testing

The crate includes unit tests for:

- `MatchConfig` validation and defaults.
- `MatchExpr` strategy validation.
- End-to-end `DefaultMatcher` behavior over an in-memory `index`:
  - Basic semantic matching.
  - Tenant isolation enforcement.
  - Metrics recording via a test `MatchMetrics` implementation.

You can run the tests with:

```bash
cargo test -p match
```

Since `DefaultMatcher::in_memory_default` uses only the in-memory backend, no
external services or RocksDB toolchain are required to execute the test suite.

## Integration with Pipeline

`match` depends on the entire UCFP pipeline. The typical flow is:

1. **Indexing phase**:
   ```
   RawIngestRecord → ingest → CanonicalIngestRecord
   → canonical → CanonicalizedDocument
   → (perceptual + semantic) → (PerceptualFingerprint, SemanticEmbedding)
   → index::IndexRecord → UfpIndex::upsert()
   ```

2. **Query phase** (handled by `match`):
   ```
   MatchRequest → DefaultMatcher
   → runs query through pipeline → index::search()
   → MatchHit results
   ```

The `DefaultMatcher` handles the entire query pipeline internally, using the
individual crate dependencies to process the query text the same way documents
were processed during indexing.

## Migration from ucfp umbrella crate

If you were previously using `ucfp` as a dependency for matching, update your
`Cargo.toml` to use `match` directly:

```toml
[dependencies]
match = { path = "../match" }
index = { path = "../index" }
```

The `DefaultMatcher` now directly orchestrates the individual pipeline crates
instead of going through the `ucfp` umbrella crate, ensuring a clean linear
dependency graph.
