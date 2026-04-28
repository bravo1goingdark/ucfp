//! Request and response DTOs for the HTTP API.
//!
//! Bytes-typed fields (`fingerprint`, `metadata`) ride as JSON arrays of
//! u8 — verbose but no base64 dep, demo-friendly with `curl`.
//!
//! The per-modality `*Algorithm` enums and `*Params` structs are
//! deserialised from query strings (and, where applicable, JSON bodies).
//! Serde renames everything to `kebab-case` so the wire format reads
//! `?algorithm=multi-hash` rather than `MultiHash`.

use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::core::{Modality, Record};

// ── /v1/info ───────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub(super) struct InfoResponse {
    pub format_version: u32,
    pub crate_version: String,
}

// ── /v1/records (POST upsert) ──────────────────────────────────────────

#[derive(Deserialize)]
pub(super) struct UpsertRequest {
    pub records: Vec<RecordIn>,
}

#[derive(Deserialize)]
pub(super) struct RecordIn {
    pub tenant_id: u32,
    pub record_id: u64,
    pub modality: Modality,
    pub format_version: u32,
    pub algorithm: String,
    pub config_hash: u64,
    /// Raw fingerprint bytes — JSON array of u8. Not base64.
    pub fingerprint: Vec<u8>,
    #[serde(default)]
    pub embedding: Option<Vec<f32>>,
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default)]
    pub metadata: Vec<u8>,
}

impl From<RecordIn> for Record {
    fn from(r: RecordIn) -> Self {
        Record {
            tenant_id: r.tenant_id,
            record_id: r.record_id,
            modality: r.modality,
            format_version: r.format_version,
            algorithm: r.algorithm,
            config_hash: r.config_hash,
            fingerprint: Bytes::from(r.fingerprint),
            embedding: r.embedding,
            model_id: r.model_id,
            metadata: Bytes::from(r.metadata),
        }
    }
}

#[derive(Serialize)]
pub(super) struct UpsertResponse {
    pub upserted: usize,
}

// ── /v1/query (POST) ───────────────────────────────────────────────────

#[derive(Deserialize)]
pub(super) struct QueryRequest {
    pub tenant_id: u32,
    pub modality: Modality,
    #[serde(default = "default_k")]
    pub k: usize,
    /// Dense query vector. BM25 path lands once `IndexBackend::bm25`
    /// is implemented; for now this is required.
    pub vector: Vec<f32>,
}

pub(super) fn default_k() -> usize {
    10
}

#[derive(Serialize)]
pub(super) struct QueryResponse {
    pub hits: Vec<HitOut>,
}

#[derive(Serialize)]
pub(super) struct HitOut {
    pub tenant_id: u32,
    pub record_id: u64,
    pub score: f32,
    /// `"vector" | "bm25" | "filter" | "reranker" | "fused"`.
    pub source: &'static str,
}

// ── /v1/ingest/{modality}/{tid}/{rid} (POST) ───────────────────────────

/// Returned by the modality-specific ingest routes after a successful
/// upsert. Confirms what was stored so the client can reconcile.
#[cfg(any(feature = "audio", feature = "image", feature = "text"))]
#[derive(Serialize)]
pub(super) struct IngestResponse {
    pub tenant_id: u32,
    pub record_id: u64,
    pub modality: Modality,
    pub format_version: u32,
    pub algorithm: String,
    pub config_hash: u64,
    pub fingerprint_bytes: usize,
    /// Hex-encoded fingerprint bytes for in-browser visualization.
    pub fingerprint_hex: String,
    pub has_embedding: bool,
}

// ── Algorithm enums ────────────────────────────────────────────────────
//
// Each enum is full-shape regardless of which features are on, so the
// wire format stays stable across builds. Handlers gate the dispatch
// arms behind the matching feature flag and surface a clean
// `Error::Unsupported` for missing-feature requests.

/// Audio fingerprinting algorithm selector.
///
/// Selected via `?algorithm=` on `POST /v1/ingest/audio/{tid}/{rid}`.
/// `Watermark` is exposed only on the dedicated `/watermark` route —
/// requesting it on the main ingest path returns 400.
#[cfg(feature = "audio")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AudioAlgorithm {
    /// Wang landmark hashes (default).
    #[default]
    Wang,
    /// Panako triplet hashes.
    Panako,
    /// Haitsma–Kalker / Philips robust hash.
    Haitsma,
    /// ONNX log-mel neural embeddings; requires `model_id`.
    Neural,
    /// AudioSeal-compatible watermark detection (use the dedicated route).
    Watermark,
}

/// Image fingerprinting algorithm selector.
#[cfg(feature = "image")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ImageAlgorithm {
    /// Multi-hash bundle (PHash + DHash + AHash). Default.
    #[default]
    Multi,
    /// PHash only (DCT-based perceptual hash).
    Phash,
    /// DHash only (gradient/difference hash).
    Dhash,
    /// AHash only (mean-thresholded average hash).
    Ahash,
    /// CLIP-style ONNX semantic embedding; requires `model_id`.
    Semantic,
}

/// Text fingerprinting algorithm selector.
#[cfg(feature = "text")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum TextAlgorithm {
    /// MinHash with the default slot count (default).
    #[default]
    Minhash,
    /// SimHash with term-frequency weighting.
    SimhashTf,
    /// SimHash with TF·IDF weighting.
    SimhashIdf,
    /// LSH-keyed MinHash signature.
    Lsh,
    /// TLSH 128/1.
    Tlsh,
    /// Local ONNX text encoder (BGE / E5 / MiniLM / …).
    SemanticLocal,
    /// OpenAI embeddings API.
    SemanticOpenai,
    /// Voyage embeddings API.
    SemanticVoyage,
    /// Cohere embeddings API.
    SemanticCohere,
}

/// Tokenizer choice for text fingerprinters.
#[cfg(feature = "text")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum TokenizerKind {
    /// UAX #29 word-boundary tokenizer (default).
    #[default]
    Word,
    /// UAX #29 grapheme-cluster tokenizer.
    Grapheme,
    /// CJK morphological segmenter for Japanese.
    CjkJp,
    /// CJK morphological segmenter for Korean.
    CjkKo,
}

/// Optional preprocessing pass applied before canonicalization.
#[cfg(feature = "text")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PreprocessKind {
    /// Strip HTML markup down to plain text.
    Html,
    /// Render Markdown to plain text.
    Markdown,
    /// Extract text from a PDF binary.
    Pdf,
}

// ── *Params request structs ────────────────────────────────────────────

/// Query parameters for `POST /v1/ingest/audio/...` — sample rate is
/// required since the body is raw f32 samples (no header carries it).
#[cfg(feature = "audio")]
#[derive(Deserialize)]
#[allow(dead_code)] // `model_id` only read by feature-gated dispatch arms
pub(super) struct AudioParams {
    /// Sampling rate in Hz of the inbound f32 PCM stream.
    pub sample_rate: u32,
    /// Algorithm selector. Defaults to `wang` when omitted.
    #[serde(default)]
    pub algorithm: AudioAlgorithm,
    /// On-disk path to the ONNX model file; required for `neural`.
    #[serde(default)]
    pub model_id: Option<String>,
}

/// Query parameters for `POST /v1/ingest/image/...`. Body is raw image
/// bytes (PNG/JPEG/WebP/GIF/BMP). Larger configs ride in JSON via
/// alternate routes; this keeps the GET-style query string lean.
#[cfg(feature = "image")]
#[derive(Deserialize)]
#[allow(dead_code)] // `model_id` only read by `image-semantic` arm
pub(super) struct ImageParams {
    /// Algorithm selector. Defaults to `multi` when omitted.
    #[serde(default)]
    pub algorithm: ImageAlgorithm,
    /// On-disk path to the ONNX model file; required for `semantic`.
    #[serde(default)]
    pub model_id: Option<String>,
}

/// Query parameters for `POST /v1/ingest/text/...`. Body is raw UTF-8
/// text. Knobs that need richer shapes (canonicalizer, weighting,
/// preprocess hints) are provided as flat query params with sensible
/// defaults.
#[cfg(feature = "text")]
#[derive(Deserialize)]
#[allow(dead_code)] // `model_id`/`api_key` only read by semantic arms
pub(super) struct TextParams {
    /// Algorithm selector. Defaults to `minhash` when omitted.
    #[serde(default)]
    pub algorithm: TextAlgorithm,
    /// Shingle width override. Defaults to the SDK constant.
    #[serde(default)]
    pub k: Option<usize>,
    /// MinHash slot count override (informational; the public surface
    /// is generic over a const `H`).
    #[serde(default)]
    pub h: Option<usize>,
    /// Tokenizer choice override.
    #[serde(default)]
    pub tokenizer: Option<TokenizerKind>,
    /// Preprocess pass override.
    #[serde(default)]
    pub preprocess: Option<PreprocessKind>,
    /// Embedding model identifier; required for the `semantic-*` arms.
    #[serde(default)]
    pub model_id: Option<String>,
    /// Provider API key for hosted semantic embeddings (OpenAI / Voyage
    /// / Cohere). Self-hosters can also set the corresponding env var.
    #[serde(default)]
    pub api_key: Option<String>,
}

// ── Nested config DTOs ────────────────────────────────────────────────

/// Canonicalizer configuration carried as JSON in the few routes that
/// accept a richer body. Mirrors `txtfp::Canonicalizer`'s public
/// configuration knobs.
#[cfg(feature = "text")]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct CanonicalizerDto {
    /// NFC / NFKC / NFD / NFKD selector. `None` = SDK default (NFKC).
    pub normalization: Option<String>,
    /// Apply Unicode case folding (default true).
    pub case_fold: bool,
    /// Strip Bidi-control characters (default true).
    pub strip_bidi: bool,
    /// Strip format characters (default true).
    pub strip_format: bool,
    /// Apply UTS #39 confusable mapping (default false).
    pub apply_confusable: bool,
}

/// Image preprocess configuration. Maps to `imgfprint::PreprocessConfig`.
#[cfg(feature = "image")]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct PreprocessConfigDto {
    /// Maximum byte length accepted upstream.
    pub max_input_bytes: Option<usize>,
    /// Maximum image dimension in pixels.
    pub max_dimension: Option<u32>,
    /// Minimum image dimension in pixels.
    pub min_dimension: Option<u32>,
}

/// Multi-hash bundle weighting. Compare-time only; bundle bytes don't
/// depend on this config.
#[cfg(feature = "image")]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct MultiHashConfigDto {
    /// PHash component weight in `[0, 1]`.
    pub phash_weight: Option<f32>,
    /// DHash component weight in `[0, 1]`.
    pub dhash_weight: Option<f32>,
    /// AHash component weight in `[0, 1]`.
    pub ahash_weight: Option<f32>,
    /// Global similarity weight in `[0, 1]`.
    pub global_weight: Option<f32>,
    /// Block similarity weight in `[0, 1]`.
    pub block_weight: Option<f32>,
    /// Per-block hamming distance threshold.
    pub block_distance_threshold: Option<u32>,
}

/// SimHash weighting selector.
#[cfg(feature = "text")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum WeightingDto {
    /// Term frequency only.
    Tf,
    /// TF·IDF — `idf_table_ref` names a server-side IDF table.
    Idf {
        /// Caller-provided handle for a pre-loaded IDF table. Servers
        /// that don't have a table store reject this with 400.
        idf_table_ref: Option<String>,
    },
}

// ── /v1/records/{tid}/{rid} (GET describe) ─────────────────────────────

/// Response body for the describe endpoint. Mirrors
/// [`crate::core::FingerprintMeta`] in JSON form.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FingerprintDescription {
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

impl From<crate::core::FingerprintMeta> for FingerprintDescription {
    fn from(m: crate::core::FingerprintMeta) -> Self {
        Self {
            tenant_id: m.tenant_id,
            record_id: m.record_id,
            modality: m.modality,
            algorithm: m.algorithm,
            format_version: m.format_version,
            config_hash: m.config_hash,
            fingerprint_bytes: m.fingerprint_bytes,
            has_embedding: m.has_embedding,
            embedding_dim: m.embedding_dim,
            model_id: m.model_id,
            metadata_bytes: m.metadata_bytes,
        }
    }
}

// ── Watermark detection response ───────────────────────────────────────

/// Response body for `POST /v1/ingest/audio/{tid}/{rid}/watermark`.
///
/// Mirrors [`crate::modality::audio::WatermarkReport`] when the
/// `audio-watermark` feature is on; defined here unconditionally so the
/// public DTO surface stays stable across feature combinations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WatermarkReport {
    /// `true` when the mean detection score exceeds the configured
    /// threshold.
    pub detected: bool,
    /// Decoded message bytes when detected; `None` otherwise.
    pub payload: Option<Vec<u8>>,
    /// Mean detection confidence in `[0.0, 1.0]`.
    pub confidence: f32,
}

#[cfg(feature = "audio-watermark")]
impl From<crate::modality::audio::WatermarkReport> for WatermarkReport {
    fn from(r: crate::modality::audio::WatermarkReport) -> Self {
        Self {
            detected: r.detected,
            payload: r.payload,
            confidence: r.confidence,
        }
    }
}
