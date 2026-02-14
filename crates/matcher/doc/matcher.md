# UCFP Matcher (`matcher`)

> **Query-time matching layer for the Universal Content Fingerprinting pipeline**

[![API Docs](https://img.shields.io/badge/docs-api-blue)](https://docs.rs/matcher)
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
- [Scoring Model](#scoring-model)
- [Observability](#observability)
- [Testing](#testing)
- [Integration Guide](#integration-guide)

---

## Overview

The `matcher` crate is the **query-time matching layer** for the Universal Content Fingerprinting (UCFP) pipeline. It consumes indexed fingerprints and embeddings from the `index` crate and exposes a small, opinionated API for finding near-duplicate or semantically related documents in multi-tenant deployments.

### What This Crate Does

| Function | Description |
|----------|-------------|
| **Query Processing** | Canonicalize and fingerprint query text using the upstream pipeline |
| **Semantic Search** | Find similar documents using cosine similarity over embeddings |
| **Perceptual Search** | Find similar documents using Jaccard similarity over MinHash |
| **Hybrid Matching** | Combine semantic and perceptual signals with weighted scoring |
| **Multi-tenant Isolation** | Filter results by tenant ID for SaaS-style deployments |
| **Policy Enforcement** | Apply thresholds, result limits, and oversampling strategies |

### Pipeline Position

```
┌─────────┐     ┌──────────┐     ┌──────────────────┐     ┌───────┐     ┌───────┐
│  Ingest │────▶│Canonical │────▶│Perceptual/Semantic│────▶│ Index │────▶│ Match │
│         │     │          │     │                  │     │       │     │(this) │
└─────────┘     └──────────┘     └──────────────────┘     └───────┘     └───────┘
```

---

## Quick Start

### Basic Matching

```rust
use ingest::IngestConfig;
use canonical::CanonicalizeConfig;
use perceptual::PerceptualConfig;
use semantic::SemanticConfig;
use index::{BackendConfig, IndexConfig, UfpIndex};
use matcher::{Matcher, MatchConfig, MatchMode, MatchRequest, MatchExpr};

let index_cfg = IndexConfig::new().with_backend(BackendConfig::in_memory());
let index = UfpIndex::new(index_cfg).expect("index init");

let matcher = Matcher::new(
    index,
    IngestConfig::default(),
    CanonicalizeConfig::default(),
    PerceptualConfig::default(),
    SemanticConfig::default(),
);

let req = MatchRequest {
    tenant_id: "tenant-a".into(),
    query_text: "Rust memory safety".into(),
    config: MatchConfig {
        mode: MatchMode::Hybrid,
        max_results: 10,
        tenant_enforce: true,
        oversample_factor: 2.0,
        explain: true,
        strategy: MatchExpr::Weighted {
            semantic_weight: 0.7,
            min_overall: 0.3,
        },
        ..Default::default()
    },
    attributes: None,
    pipeline_version: None,
    fingerprint_versions: None,
    query_canonical_hash: None,
};

let hits = matcher.match_document(&req).expect("match");
for hit in hits {
    println!("{} score={}", hit.canonical_hash, hit.score);
}
```

---

## Architecture

### Data Flow

```
MatchRequest
      │
      ▼
┌─────────────────────────────────────────┐
│           Matcher Pipeline               │
├─────────────────────────────────────────┤
│  1. Build Query Record                 │
│     - Run ingest → canonical pipeline   │
│     - Generate perceptual fingerprint  │
│     - Generate semantic embedding       │
├─────────────────────────────────────────┤
│  2. Query Index                        │
│     - Determine search strategy         │
│     - Apply oversampling                │
│     - Fetch candidate records           │
├─────────────────────────────────────────┤
│  3. Score Candidates                   │
│     - Calculate semantic similarity     │
│     - Calculate perceptual similarity  │
│     - Apply MatchExpr strategy         │
├─────────────────────────────────────────┤
│  4. Filter & Rank                      │
│     - Enforce tenant isolation          │
│     - Apply thresholds                  │
│     - Sort by score                     │
│     - Truncate to max_results          │
└─────────────────────────────────────────┘
      │
      ▼
Vec<MatchHit>
```

### Dependencies

`matcher` directly depends on:
- `ingest` - For ingest configuration and raw record types
- `canonical` - For text canonicalization
- `perceptual` - For perceptual fingerprint generation
- `semantic` - For semantic embedding generation
- `index` - For storage and similarity search

This linear architecture ensures no circular dependencies and clear separation of concerns.

---

## Core Concepts

### MatchMode

Coarse match mode for metrics and backwards compatibility:

```rust
pub enum MatchMode {
    /// Semantic-only matching using cosine similarity over embeddings
    Semantic,
    /// Perceptual-only matching using Jaccard similarity over MinHash
    Perceptual,
    /// Combine semantic and perceptual scores using a weighted policy
    Hybrid,
}
```

### MatchExpr

Declarative matching strategy for combining signals:

```rust
pub enum MatchExpr {
    /// Exact identity match on canonical hash
    Exact,
    /// Pure semantic similarity with explicit metric + threshold
    Semantic { metric: MetricId, min_score: f32 },
    /// Pure perceptual similarity with explicit metric + threshold
    Perceptual { metric: MetricId, min_score: f32 },
    /// Weighted combination of semantic and perceptual scores
    Weighted { semantic_weight: f32, min_overall: f32 },
    /// Logical conjunction of two sub-strategies
    And { left: Box<MatchExpr>, right: Box<MatchExpr> },
    /// Logical disjunction of two sub-strategies
    Or { left: Box<MatchExpr>, right: Box<MatchExpr> },
}
```

### MetricId

Similarity metric identifiers:

```rust
pub enum MetricId {
    Cosine,
    Jaccard,
    Hamming,
}
```

---

## Configuration

### MatchConfig

Central configuration struct controlling match behavior:

```rust
pub struct MatchConfig {
    /// Configuration schema version
    pub version: String,
    /// Logical policy identifier for audits
    pub policy_id: String,
    /// Policy version identifier
    pub policy_version: String,
    /// High-level mode for observability
    pub mode: MatchMode,
    /// Declarative strategy for combining signals
    pub strategy: MatchExpr,
    /// Maximum results to return
    pub max_results: usize,
    /// Enforce tenant isolation
    pub tenant_enforce: bool,
    /// Oversampling factor (internal_top_k = ceil(max_results * oversample_factor))
    pub oversample_factor: f32,
    /// Populate explanation data in hits
    pub explain: bool,
}
```

### MatchRequest

Query request structure:

```rust
pub struct MatchRequest {
    /// Tenant identifier for multi-tenant isolation
    pub tenant_id: String,
    /// Free-text query text
    pub query_text: String,
    /// Per-request configuration
    pub config: MatchConfig,
    /// Optional opaque attributes
    pub attributes: Option<serde_json::Value>,
    /// Optional pipeline version
    pub pipeline_version: Option<String>,
    /// Optional fingerprint version map
    pub fingerprint_versions: Option<HashMap<String, String>>,
    /// Pre-computed canonical hash for exact-match
    pub query_canonical_hash: Option<String>,
}
```

---

## API Reference

### Matcher

Production-ready implementation that wires the pipeline crates together:

```rust
pub struct Matcher { /* ... */ }

impl Matcher {
    /// Construct a matcher from an existing index and explicit configs
    pub fn new(
        index: UfpIndex,
        ingest_cfg: IngestConfig,
        canonical_cfg: CanonicalizeConfig,
        perceptual_cfg: PerceptualConfig,
        semantic_cfg: SemanticConfig,
    ) -> Self;

    /// Construct a matcher from a shared index handle
    pub fn with_index_arc(
        index: Arc<UfpIndex>,
        ingest_cfg: IngestConfig,
        canonical_cfg: CanonicalizeConfig,
        perceptual_cfg: PerceptualConfig,
        semantic_cfg: SemanticConfig,
    ) -> Self;

    /// Convenience helper for tests
    pub fn in_memory_default(
        ingest_cfg: IngestConfig,
        canonical_cfg: CanonicalizeConfig,
        perceptual_cfg: PerceptualConfig,
        semantic_cfg: SemanticConfig,
    ) -> Result<Self, MatchError>;

    /// Execute a match request
    pub fn match_document(&self, req: &MatchRequest) -> Result<Vec<MatchHit>, MatchError>;
}
```

### MatchHit

Single match result:

```rust
pub struct MatchHit {
    /// Canonical hash of matched document
    pub canonical_hash: String,
    /// Final score after strategy application
    pub score: f32,
    /// Underlying semantic score
    pub semantic_score: Option<f32>,
    /// Underlying perceptual score
    pub perceptual_score: Option<f32>,
    /// Exact-match score
    pub exact_score: Option<f32>,
    /// Stored metadata from index
    pub metadata: serde_json::Value,
    /// Match engine version
    pub match_version: String,
    /// Policy identifier
    pub policy_id: String,
    pub policy_version: String,
    /// Optional explanation data
    pub explanation: Option<MatchExplanation>,
}
```

### MatchExplanation

Explanation artifacts:

```rust
pub struct MatchExplanation {
    pub semantic_distance: Option<f32>,
    pub perceptual_overlap: Option<f32>,
    pub token_overlap: Option<f32>,
}
```

---

## Error Handling

### MatchError Variants

All errors are typed and cloneable:

| Error | Trigger |
|-------|---------|
| `InvalidConfig(String)` | Invalid match configuration |
| `Ingest(String)` | Ingest stage failed |
| `Canonical(String)` | Canonical stage failed |
| `Perceptual(String)` | Perceptual fingerprinting failed |
| `Semantic(String)` | Semantic embedding failed |
| `Pipeline(String)` | General pipeline error |
| `Index(IndexError)` | Index read/search failed |

### Error Handling Patterns

```rust
use matcher::MatchError;

fn handle_match_error(err: MatchError) {
    match err {
        MatchError::InvalidConfig(msg) => {
            tracing::warn!(error = %msg, "invalid_match_config");
        }
        MatchError::Index(e) => {
            tracing::error!(error = %e, "index_error");
        }
        _ => {
            tracing::error!(error = %err, "match_failure");
        }
    }
}
```

---

## Examples

### Hybrid Scoring

```rust
use matcher::{MatchConfig, MatchMode, MatchExpr, MetricId};

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

let req = MatchRequest {
    tenant_id: "tenant-a".into(),
    query_text: "Rust safety".into(),
    config: hybrid_cfg,
    attributes: None,
    pipeline_version: None,
    fingerprint_versions: None,
    query_canonical_hash: None,
};

let hits = matcher.match_document(&req)?;
for hit in hits {
    println!(
        "hash={} score={} semantic={:?} perceptual={:?}",
        hit.canonical_hash, hit.score, hit.semantic_score, hit.perceptual_score,
    );
}
```

### Exact Match

```rust
use matcher::MatchExpr;

let req = MatchRequest {
    tenant_id: "tenant-a".into(),
    query_text: "".into(), // Ignored for exact match
    config: MatchConfig {
        strategy: MatchExpr::Exact,
        ..Default::default()
    },
    query_canonical_hash: Some("abc123".into()),
    ..Default::default()
};

let hits = matcher.match_document(&req)?;
```

### Semantic-Only with Threshold

```rust
use matcher::{MatchExpr, MetricId};

let req = MatchRequest {
    tenant_id: "tenant-a".into(),
    query_text: "memory safety".into(),
    config: MatchConfig {
        strategy: MatchExpr::Semantic {
            metric: MetricId::Cosine,
            min_score: 0.75,
        },
        ..Default::default()
    },
    ..Default::default()
};

let hits = matcher.match_document(&req)?;
// Only returns results with cosine similarity >= 0.75
```

---

## Scoring Model

The matcher combines per-mode scores into a single `score` field used for ranking:

### Semantic Mode

```
score = s  (semantic score, defaults to 0.0 if unavailable)
```

### Perceptual Mode

```
score = p  (perceptual score, defaults to 0.0 if unavailable)
```

### Hybrid with Weighted Strategy

```
alpha = clamp(semantic_weight, 0.0, 1.0)
s_eff = s.unwrap_or(0.0)
p_eff = p.unwrap_or(0.0)
score = alpha * s_eff + (1.0 - alpha) * p_eff
```

### Logical Strategies

- **And**: `score = min(left_score, right_score)`
- **Or**: `score = max(left_score, right_score)`

Results are sorted by descending `score` and truncated to `max_results`.

---

## Observability

Install a `MatchMetrics` implementation to capture per-request metrics:

```rust
use matcher::{set_match_metrics, MatchMetrics};

struct MyMetrics { /* ... */ }

impl MatchMetrics for MyMetrics {
    fn record_latency(&self, mode: MatchMode, latency: std::time::Duration) {
        // Record to Prometheus, OpenTelemetry, etc.
    }
    fn record_hits(&self, mode: MatchMode, hit_count: usize) {
        // Record hit count
    }
}

set_match_metrics(Some(Box::new(MyMetrics { /* ... */ })));
```

---

## Testing

```bash
# Run all tests
cargo test -p matcher

# Run with output
cargo test -p matcher -- --nocapture

# Run specific test
cargo test -p matcher test_hybrid_matching

# Run benchmarks
cargo bench -p matcher
```

The crate includes unit tests for:
- `MatchConfig` validation and defaults
- `MatchExpr` strategy validation
- End-to-end `Matcher` behavior over in-memory index
- Tenant isolation enforcement
- Metrics recording

---

## Integration Guide

### With the Full Pipeline

```rust
// 1. Indexing phase (your existing pipeline)
use ingest::{ingest, IngestConfig, RawIngestRecord, IngestSource, IngestMetadata, IngestPayload};
use canonical::canonicalize;
use perceptual::perceptualize_tokens;
use semantic::semanticize;
use index::UfpIndex;

let record = RawIngestRecord { /* ... */ };
let canonical = ingest(record, &ingest_cfg)?;
let canonical_doc = canonicalize(&canonical, &canonical_cfg)?;
let perceptual = perceptualize_tokens(&canonical_doc, &perceptual_cfg)?;
let embedding = semanticize(&canonical_doc, &semantic_cfg).await?;

let index_record = IndexRecord {
    schema_version: INDEX_SCHEMA_VERSION,
    canonical_hash: /* ... */,
    perceptual: Some(perceptual),
    embedding: Some(embedding),
    metadata: serde_json::json!({ "tenant": tenant_id }),
};

index.upsert(index_record)?;

// 2. Query phase (matcher)
let matcher = Matcher::new(index, ingest_cfg, canonical_cfg, perceptual_cfg, semantic_cfg);
let hits = matcher.match_document(&req)?;
```

---

## Migration from ucfp Umbrella Crate

If you were previously using `ucfp` as a dependency for matching:

```toml
[dependencies]
matcher = { path = "../matcher" }
index = { path = "../index" }
```

The `Matcher` now directly orchestrates the individual pipeline crates instead of going through the `ucfp` umbrella crate, ensuring a clean linear dependency graph.

---

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](../../LICENSE) for details.

---

## Contributing

Contributions are welcome! Please ensure:
- All tests pass: `cargo test -p matcher`
- Documentation is updated
- Examples are provided for new features
