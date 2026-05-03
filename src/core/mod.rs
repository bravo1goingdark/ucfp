//! Core data shapes — the contract between ingest, index, matcher, and rerank.
//!
//! These types are modality-agnostic: they describe *fingerprinted records*,
//! not *audio* or *images* or *text*. Per-modality SDKs build instances via
//! the [`crate::modality`] adapters; the index and matcher only ever see
//! these shapes.
//!
//! Layout note: `tenant_id` lives on every shape. Per-tenant key prefixing
//! is a day-one schema decision (see `docs/ARCHITECTURE.md` §8.1).

use bytes::Bytes;
use serde::{Deserialize, Serialize};

/// What kind of content produced this record.
///
/// The matcher uses this to refuse cross-modality compares: an audio
/// landmark hash and an image PHash are never directly comparable.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Modality {
    /// Audio fingerprint (Wang/Panako/Haitsma) from `audiofp`.
    Audio,
    /// Image fingerprint (AHash/PHash/DHash) from `imgfprint`.
    Image,
    /// Text fingerprint (MinHash/SimHash/LSH) from `txtfp`.
    Text,
}

/// A unit of work flowing into the index.
///
/// The fingerprint blob is whatever the producing SDK emits via
/// `bytemuck::cast_slice` — UCFP treats it as an opaque byte run keyed
/// by `(modality, algorithm, format_version, config_hash)`.
#[derive(Clone, Debug)]
pub struct Record {
    /// Tenant partition key. Used as the leading component of every
    /// storage key; see ARCHITECTURE §8.1.
    pub tenant_id: u32,
    /// Monotonic per-tenant identifier. Producers should use UUIDv5 of
    /// the canonical input or a content hash for idempotency.
    pub record_id: u64,
    /// Which SDK produced this record.
    pub modality: Modality,
    /// The producing SDK's `FORMAT_VERSION` constant at ingest time.
    /// Cross-version compares are refused by [`crate::Error::Incompatible`].
    pub format_version: u32,
    /// Stable algorithm tag, e.g. `"wang"`, `"imgfprint-multihash-v1"`,
    /// `"minhash-h128"`. Frozen for the lifetime of `format_version`.
    /// Owned `String` (rather than `&'static str`) so HTTP-supplied tags
    /// from runtime DTOs can flow through unchanged; modality adapters
    /// build with `ALGORITHM.into()`.
    pub algorithm: String,
    /// SDK-specific config hash (e.g. `txtfp::config_hash`). Two records
    /// with the same `algorithm` but different `config_hash` are not
    /// directly comparable.
    pub config_hash: u64,
    /// Raw `bytemuck`-cast fingerprint bytes. Layout is owned by the SDK.
    pub fingerprint: Bytes,
    /// Optional dense vector for semantic similarity (cosine).
    pub embedding: Option<Vec<f32>>,
    /// Embedding model identifier — must match across compared records.
    pub model_id: Option<String>,
    /// Variable-length application metadata (rkyv-archived in storage).
    pub metadata: Bytes,
    /// Original text content for BM25 inverted-index ingestion.
    ///
    /// Set by the text modality builders (MinHash / SimHash / LSH) so the
    /// embedded backend can update the per-tenant FST + roaring postings
    /// inside the same redb transaction that stores the fingerprint
    /// itself. `None` for non-text modalities — the BM25 path is then a
    /// no-op for that record. See ARCHITECTURE §4.
    pub text: Option<String>,
}

/// Metadata view of a stored fingerprint without materialising its bytes.
///
/// Returned by [`crate::IndexBackend::get_record_metadata`]; powers the
/// `GET /v1/records/{tid}/{rid}` describe endpoint. Only sizes and
/// identifiers are exposed — the raw fingerprint blob stays in the
/// backend so the read cost is constant per record.
#[derive(Clone, Debug, PartialEq)]
pub struct FingerprintMeta {
    /// Tenant the record is scoped to.
    pub tenant_id: u32,
    /// Record identifier within the tenant.
    pub record_id: u64,
    /// Modality the record was produced from.
    pub modality: Modality,
    /// SDK algorithm tag captured at ingest time.
    pub algorithm: String,
    /// SDK FORMAT_VERSION captured at ingest time.
    pub format_version: u32,
    /// SDK config hash captured at ingest time.
    pub config_hash: u64,
    /// Length of the fingerprint blob in bytes.
    pub fingerprint_bytes: usize,
    /// `true` if the record carries a dense embedding vector.
    pub has_embedding: bool,
    /// Dimension of the dense embedding vector when present.
    pub embedding_dim: Option<usize>,
    /// Embedding model identifier when present.
    pub model_id: Option<String>,
    /// Length of the application metadata blob in bytes.
    pub metadata_bytes: usize,
}

/// A single search result.
#[derive(Clone, Debug)]
pub struct Hit {
    /// Tenant the hit belongs to.
    pub tenant_id: u32,
    /// Record identifier within the tenant.
    pub record_id: u64,
    /// Higher is better. Cosine similarity for vector hits, BM25 for text,
    /// fused score for hybrid results.
    pub score: f32,
    /// Which retrieval path produced this hit. Useful for explainability.
    pub source: HitSource,
}

/// Which ranker produced a [`Hit`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum HitSource {
    /// Dense vector k-NN.
    Vector,
    /// Sparse BM25 over text fields.
    Bm25,
    /// Pure metadata-filter result (no relevance signal).
    Filter,
    /// Reranked by a cross-encoder or LTR model.
    Reranker,
    /// Output of [`crate::matcher::rrf`] fusion.
    Fused,
}

/// Query envelope passed to [`crate::Matcher::search`].
///
/// Each ranker reads only the fields it needs: vector knn ignores `terms`,
/// BM25 ignores `vector`, both honour `filter` and `tenant_id`.
#[derive(Clone, Debug)]
pub struct Query {
    /// Tenant scope. The matcher pushes this down to every backend call.
    pub tenant_id: u32,
    /// Modality scope. Cross-modality queries are refused unless
    /// embeddings explicitly share an aligned `model_id`.
    pub modality: Modality,
    /// Top-k cap returned by the matcher.
    pub k: usize,
    /// Optional dense query vector. `None` → BM25/filter-only.
    pub vector: Option<Vec<f32>>,
    /// Optional tokenized query terms. Empty → vector/filter-only.
    pub terms: Vec<String>,
    /// Optional metadata pre-filter expression (RoaringBitmap-encoded
    /// in the embedded backend).
    pub filter: Option<Bytes>,
    /// RRF fusion constant. Default 60 per ARCHITECTURE §4.
    pub rrf_k: u32,
}

impl Default for Query {
    fn default() -> Self {
        Self {
            tenant_id: 0,
            modality: Modality::Text,
            k: 10,
            vector: None,
            terms: Vec::new(),
            filter: None,
            rrf_k: 60,
        }
    }
}
