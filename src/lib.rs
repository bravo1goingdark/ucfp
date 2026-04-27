//! `ucfp` — Universal Content Fingerprinting integrator.
//!
//! UCFP does **not** compute fingerprints itself. The per-modality SDKs
//! ([`audiofp`], [`imgfprint`], [`txtfp`]) each own their canonicalization,
//! hashing, and (optional) ML embedding. UCFP provides what they don't:
//!
//! - **ingest** — accept records over HTTP, validate, route to the right SDK
//! - **storage** — persist fingerprint bytes + metadata + posting lists in
//!   one [`redb`] file (default) or graduate to a managed backend
//! - **indexing** — vector ANN ([`hnsw_rs`] or brute-force) + metadata
//!   pre-filter ([`roaring`])
//! - **matching** — hybrid retrieval (vector + BM25 + filter), Reciprocal
//!   Rank Fusion, optional ONNX rerank
//! - **server** — minimal axum HTTP API binding the above
//!
//! See [`docs/ARCHITECTURE.md`] in the repository for the full design and
//! scale-up triggers.
//!
//! [`docs/ARCHITECTURE.md`]: https://github.com/bravo1goingdark/ucfp/blob/main/docs/ARCHITECTURE.md

#![deny(missing_docs)]
#![warn(rust_2018_idioms)]

mod core;
mod error;
mod index;
mod ingest;
mod matcher;
mod modality;
mod rerank;

pub use crate::core::{HitSource, Modality, Query, Record};
pub use crate::error::{Error, Result};
pub use crate::index::IndexBackend;
pub use crate::ingest::IngestSource;
pub use crate::matcher::{Matcher, rrf};
pub use crate::rerank::{NoopReranker, Reranker};

#[cfg(feature = "audio")]
pub use crate::modality::audio;

#[cfg(feature = "image")]
pub use crate::modality::image;

#[cfg(feature = "text")]
pub use crate::modality::text;

#[cfg(feature = "embedded")]
pub use crate::index::embedded::EmbeddedBackend;

/// On-disk format version of a UCFP database.
///
/// Independent of the per-modality SDKs' own `FORMAT_VERSION` constants —
/// each [`Record`] carries the SDK version at its origin so cross-SDK-
/// version compares can be refused without touching this constant.
pub const FORMAT_VERSION: u32 = 1;

/// Hit returned from [`IndexBackend::knn`] / [`IndexBackend::bm25`] /
/// [`Matcher::search`].
pub use crate::core::Hit;
