//! Image fingerprinting via [`imgfprint`].
//!
//! Wraps every [`imgfprint`] algorithm into a uniform [`Record`]. The
//! default `fingerprint`/`fingerprint_with` pair compute the multi-hash
//! bundle (PHash + DHash + AHash); per-algorithm helpers expose each of
//! the individual hashes for callers that want to keep storage tight or
//! side-load semantics.
//!
//! # Algorithms
//!
//! | Function                  | Output                  | Feature gate       |
//! | ------------------------- | ----------------------- | ------------------ |
//! | [`fingerprint`]           | MultiHash bundle (default) | `image`         |
//! | [`fingerprint_with`]      | MultiHash + preprocess  | `image`            |
//! | `fingerprint_multi_with`  | MultiHash + multi-config  | `image-perceptual` |
//! | `fingerprint_phash`       | PHash only                | `image-perceptual` |
//! | `fingerprint_dhash`       | DHash only                | `image-perceptual` |
//! | `fingerprint_ahash`       | AHash only                | `image-perceptual` |
//! | `fingerprint_semantic`    | CLIP-style ONNX embedding | `image-semantic`   |
//!
//! `MultiHashConfig` is deliberately threaded only through
//! `fingerprint_multi_with` — the bytes of a multi-hash bundle do not
//! depend on the config (the config is interpreted at compare time), so
//! the simple `fingerprint`/`fingerprint_with` path can ignore it.

use bytes::Bytes;
#[cfg(feature = "image-perceptual")]
use imgfprint::MultiHashConfig;
use imgfprint::{ImageFingerprinter, MultiHashFingerprint, PreprocessConfig};

use crate::core::{Modality, Record};
use crate::error::{Error, Result};

/// Stable algorithm tag for the imgfprint multi-hash bundle.
///
/// Kept as `ALGORITHM` for backwards compatibility with the original
/// helper; new code should prefer [`ALGORITHM_MULTIHASH`] for clarity.
pub const ALGORITHM: &str = "imgfprint-multihash-v1";
/// Stable algorithm tag for the multi-hash bundle (alias of [`ALGORITHM`]).
pub const ALGORITHM_MULTIHASH: &str = "imgfprint-multihash-v1";
/// Stable algorithm tag for PHash (DCT-based perceptual hash).
pub const ALGORITHM_PHASH: &str = "imgfprint-phash-v1";
/// Stable algorithm tag for DHash (gradient-based difference hash).
pub const ALGORITHM_DHASH: &str = "imgfprint-dhash-v1";
/// Stable algorithm tag for AHash (mean-thresholded average hash).
pub const ALGORITHM_AHASH: &str = "imgfprint-ahash-v1";
/// Stable algorithm tag for CLIP-style semantic embeddings via ONNX.
pub const ALGORITHM_SEMANTIC: &str = "imgfprint-semantic-v1";

// ─────────────────────────────────────────────────────────────────────────
// Default MultiHash bundle path (kept for backwards compat).
// ─────────────────────────────────────────────────────────────────────────

/// Fingerprint raw image bytes (PNG / JPEG / WebP / GIF / BMP) with
/// imgfprint's default preprocess and the default MultiHash bundle.
pub fn fingerprint(bytes: &[u8], tenant_id: u32, record_id: u64) -> Result<Record> {
    fingerprint_with(bytes, tenant_id, record_id, &PreprocessConfig::default())
}

/// MultiHash bundle with a tunable [`PreprocessConfig`] (max_input_bytes,
/// max_dimension, min_dimension).
pub fn fingerprint_with(
    bytes: &[u8],
    tenant_id: u32,
    record_id: u64,
    preprocess: &PreprocessConfig,
) -> Result<Record> {
    let fp: MultiHashFingerprint =
        ImageFingerprinter::fingerprint_with_preprocess(bytes, preprocess)
            .map_err(|e| Error::Modality(e.to_string()))?;

    Ok(Record {
        tenant_id,
        record_id,
        modality: Modality::Image,
        format_version: imgfprint::FORMAT_VERSION,
        algorithm: ALGORITHM_MULTIHASH.into(),
        // imgfprint's compare-time config is per-call (MultiHashConfig),
        // not folded into the stored fingerprint, so config_hash is 0
        // for stored records — callers attach it at compare time.
        config_hash: 0,
        fingerprint: Bytes::copy_from_slice(bytemuck::bytes_of(&fp)),
        embedding: None,
        model_id: None,
        metadata: Bytes::new(),
    })
}

/// MultiHash bundle with both preprocess and multi-hash weighting
/// configuration. The `multi_cfg` parameter does not affect the stored
/// bytes (bundle layout is fixed); it is reserved for compare-time use
/// and threaded through here so callers can persist the chosen config
/// alongside the record via metadata.
#[cfg(feature = "image-perceptual")]
pub fn fingerprint_multi_with(
    bytes: &[u8],
    preprocess: &PreprocessConfig,
    _multi_cfg: &MultiHashConfig,
    tenant_id: u32,
    record_id: u64,
) -> Result<Record> {
    fingerprint_with(bytes, tenant_id, record_id, preprocess)
}

// ─────────────────────────────────────────────────────────────────────────
// Per-algorithm hashes — feature `image-perceptual`.
// ─────────────────────────────────────────────────────────────────────────

/// Compute a single PHash (DCT-based perceptual hash) for the image.
#[cfg(feature = "image-perceptual")]
pub fn fingerprint_phash(
    bytes: &[u8],
    preprocess: &PreprocessConfig,
    tenant_id: u32,
    record_id: u64,
) -> Result<Record> {
    fingerprint_single(
        bytes,
        preprocess,
        imgfprint::HashAlgorithm::PHash,
        ALGORITHM_PHASH,
        tenant_id,
        record_id,
    )
}

/// Compute a single DHash (gradient/difference hash) for the image.
#[cfg(feature = "image-perceptual")]
pub fn fingerprint_dhash(
    bytes: &[u8],
    preprocess: &PreprocessConfig,
    tenant_id: u32,
    record_id: u64,
) -> Result<Record> {
    fingerprint_single(
        bytes,
        preprocess,
        imgfprint::HashAlgorithm::DHash,
        ALGORITHM_DHASH,
        tenant_id,
        record_id,
    )
}

/// Compute a single AHash (average hash) for the image.
#[cfg(feature = "image-perceptual")]
pub fn fingerprint_ahash(
    bytes: &[u8],
    preprocess: &PreprocessConfig,
    tenant_id: u32,
    record_id: u64,
) -> Result<Record> {
    fingerprint_single(
        bytes,
        preprocess,
        imgfprint::HashAlgorithm::AHash,
        ALGORITHM_AHASH,
        tenant_id,
        record_id,
    )
}

/// Helper that runs a single-algorithm fingerprint and packs the
/// resulting [`imgfprint::ImageFingerprint`] into a [`Record`].
#[cfg(feature = "image-perceptual")]
fn fingerprint_single(
    bytes: &[u8],
    preprocess: &PreprocessConfig,
    algo: imgfprint::HashAlgorithm,
    tag: &'static str,
    tenant_id: u32,
    record_id: u64,
) -> Result<Record> {
    use imgfprint::FingerprinterContext;
    let mut ctx = FingerprinterContext::new();
    let fp = ctx
        .fingerprint_with_algorithm_and_preprocess(bytes, algo, preprocess)
        .map_err(|e| Error::Modality(e.to_string()))?;

    Ok(Record {
        tenant_id,
        record_id,
        modality: Modality::Image,
        format_version: imgfprint::FORMAT_VERSION,
        algorithm: tag.into(),
        config_hash: 0,
        fingerprint: Bytes::copy_from_slice(bytemuck::bytes_of(&fp)),
        embedding: None,
        model_id: None,
        metadata: Bytes::new(),
    })
}

// ─────────────────────────────────────────────────────────────────────────
// Semantic ONNX embeddings — feature `image-semantic`.
// ─────────────────────────────────────────────────────────────────────────

/// Run a CLIP-style ONNX vision model over the given image bytes and
/// produce a [`Record`] whose `embedding` slot carries the resulting
/// vector for vector-knn matching.
///
/// `model_path` is the on-disk ONNX file; the same path is stored in
/// `model_id` so downstream `IndexBackend` queries can refuse
/// cross-model joins. The `_preprocess` argument is accepted for API
/// parity with the perceptual paths but is ignored — `imgfprint`'s
/// `LocalProvider` runs its own resize/normalise pipeline.
#[cfg(feature = "image-semantic")]
pub fn fingerprint_semantic(
    bytes: &[u8],
    _preprocess: &PreprocessConfig,
    model_path: &str,
    tenant_id: u32,
    record_id: u64,
) -> Result<Record> {
    use imgfprint::{EmbeddingProvider, LocalProvider};

    let provider =
        LocalProvider::from_file(model_path).map_err(|e| Error::Modality(e.to_string()))?;
    let emb = provider
        .embed(bytes)
        .map_err(|e| Error::Modality(e.to_string()))?;

    let vector: Vec<f32> = emb.as_slice().to_vec();
    let bytes_out = Bytes::copy_from_slice(bytemuck::cast_slice(&vector));

    Ok(Record {
        tenant_id,
        record_id,
        modality: Modality::Image,
        format_version: imgfprint::FORMAT_VERSION,
        algorithm: ALGORITHM_SEMANTIC.into(),
        config_hash: 0,
        fingerprint: bytes_out,
        embedding: Some(vector),
        model_id: Some(model_path.to_string()),
        metadata: Bytes::new(),
    })
}
