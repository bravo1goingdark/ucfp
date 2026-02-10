//! # UCFP Matcher (`matcher`)
//!
//! ## Purpose
//!
//! `matcher` sits on top of the core UCFP pipeline (`ucfp`) and the index
//! layer (`index`). It is responsible for turning free-text queries into
//! canonicalized representations, running perceptual and/or semantic search,
//! enforcing multi-tenant isolation, and applying match policies such as
//! thresholds and result limits.
//!
//! In a typical deployment you will:
//! - Use `ucfp` to ingest and canonicalize documents, then write `IndexRecord`
//!   values into `index`.
//! - Use `matcher` to service query-time lookups over that index, selecting
//!   between semantic, perceptual, or hybrid strategies.
//!
//! ## Core Types
//!
//! - [`MatchMode`]: selects the matching strategy:
//!   - `Semantic` — cosine similarity over quantized embeddings.
//!   - `Perceptual` — Jaccard similarity over MinHash fingerprints.
//!   - `Hybrid` — weighted combination of both scores.
//! - [`MatchConfig`]: per-request tuning knobs such as `max_results`,
//!   `oversample_factor`, and `tenant_enforce`.
//! - [`MatchExpr`]: declarative expression of the matching strategy.
//! - [`MatchRequest`]: tenant id + query text + configuration.
//! - [`MatchHit`]: canonical hash, final score, optional per-mode scores,
//!   and stored metadata.
//! - [`Matcher`]: production-ready implementation that wires `ucfp` and `index` together.
//!
//! ## Example Usage
//!
//! ```no_run
//! use std::sync::Arc;
//! use ingest::IngestConfig;
//! use canonical::CanonicalizeConfig;
//! use perceptual::PerceptualConfig;
//! use semantic::SemanticConfig;
//! use index::{BackendConfig, IndexConfig, UfpIndex};
//! use matcher::{
//!     Matcher, MatchConfig, MatchExpr, MatchMode, MatchRequest,
//! };
//!
//! // Build or open the index
//! let index_cfg = IndexConfig::new().with_backend(BackendConfig::in_memory());
//! let index = UfpIndex::new(index_cfg).expect("index init");
//!
//! let ingest_cfg = IngestConfig::default();
//! let canonical_cfg = CanonicalizeConfig::default();
//! let perceptual_cfg = PerceptualConfig::default();
//! let semantic_cfg = SemanticConfig::default();
//!
//! let matcher = Matcher::new(index, ingest_cfg, canonical_cfg, perceptual_cfg, semantic_cfg);
//!
//! let req = MatchRequest {
//!     tenant_id: "tenant-a".into(),
//!     query_text: "Rust memory safety".into(),
//!     config: MatchConfig {
//!         mode: MatchMode::Hybrid,
//!         max_results: 10,
//!         tenant_enforce: true,
//!         oversample_factor: 2.0,
//!         explain: true,
//!         strategy: MatchExpr::Weighted {
//!             semantic_weight: 0.7,
//!             min_overall: 0.3,
//!         },
//!         ..Default::default()
//!     },
//!     attributes: None,
//!     pipeline_version: None,
//!     fingerprint_versions: None,
//!     query_canonical_hash: None,
//! };
//!
//! let hits = matcher.match_document(&req).expect("match");
//! for hit in hits {
//!     println!("{} score={} metadata={}", hit.canonical_hash, hit.score, hit.metadata);
//! }
//! ```
//!
//! ## Observability
//!
//! Install a [`MatchMetrics`] implementation via [`set_match_metrics`] to record
//! per-request latency and hit counts. This is typically done once during
//! service startup so all calls through [`Matcher`] share the same
//! metrics backend.

pub mod engine;
pub mod metrics;
pub mod types;

#[doc(hidden)]
pub mod demo_utils;

pub use crate::engine::Matcher;
pub use crate::metrics::{set_match_metrics, MatchMetrics};
pub use crate::types::{MatchConfig, MatchError, MatchExpr, MatchHit, MatchMode, MatchRequest};
