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

// ─────────────────────────────────────────────────────────────────────────
// Pipeline inspect — surfaces the intermediate image-pipeline stages so
// the playground's PipelineInspector UI can render each step.
// ─────────────────────────────────────────────────────────────────────────

/// One stage payload for the image pipeline inspector.
///
/// Each `*_png_b64` field is a complete PNG file encoded as base64
/// (no `data:` URI prefix — the UI prepends it). Sizes are tuned so the
/// total payload stays under ~50 KiB for a typical photo: original
/// thumbnail at 256 px max edge, 32×32 / 8×8 stages at native size.
#[cfg(feature = "inspect")]
#[derive(Clone, Debug, serde::Serialize)]
pub struct InspectImageResult {
    /// Stable algorithm identifier (currently always `imgfprint-multihash-v1`).
    pub algorithm: &'static str,
    /// Original image width in pixels (pre-thumbnail).
    pub width: u32,
    /// Original image height in pixels (pre-thumbnail).
    pub height: u32,
    /// Decoded original, downscaled to ≤256 px max edge for display.
    pub original_png_b64: String,
    /// 32×32 grayscale — the input PHash applies its DCT to.
    pub gray32_png_b64: String,
    /// 8×8 grayscale — the input AHash mean-thresholds.
    pub gray8_png_b64: String,
    /// AHash mean — the threshold used to derive the 64-bit AHash.
    pub ahash_mean: u8,
    /// Final multi-hash bundle bytes as hex.
    pub fingerprint_hex: String,
    /// Length of the underlying multi-hash bundle in bytes.
    pub fingerprint_bytes: usize,
    /// imgfprint config hash captured for this run.
    pub config_hash: u64,
}

/// Run the image pipeline and surface each intermediate stage.
///
/// Always uses the multi-hash bundle (PHash + DHash + AHash). PHash's
/// 32×32 DCT input and AHash's 8×8 grayscale input are exposed as
/// PNG thumbnails so the UI can paint them as-is.
///
/// Image-decode + resize go through the `image` crate's nearest-
/// neighbour path — these are visualisation aids, not the bytes that
/// feed imgfprint's actual fingerprint computation. The fingerprint
/// itself comes from imgfprint's default pipeline so it round-trips
/// the regular `fingerprint_with` call.
#[cfg(feature = "inspect")]
pub fn inspect_image(bytes: &[u8], pre: &PreprocessConfig) -> Result<InspectImageResult> {
    use image::{GenericImageView, imageops::FilterType};

    let img = image::load_from_memory(bytes)
        .map_err(|e| Error::Modality(format!("image decode: {e}")))?;
    let (w, h) = img.dimensions();

    // Stage 1: original thumbnail — at most 256 px max edge.
    let max_edge = 256u32;
    let original_thumb = if w.max(h) > max_edge {
        let scale = max_edge as f32 / w.max(h) as f32;
        let nw = (w as f32 * scale).round().max(1.0) as u32;
        let nh = (h as f32 * scale).round().max(1.0) as u32;
        img.resize(nw, nh, FilterType::Triangle)
    } else {
        img.clone()
    };
    let original_png_b64 = encode_png_b64(&original_thumb)?;

    // Stage 2: 32×32 grayscale (PHash DCT input).
    let gray32 = img.resize_exact(32, 32, FilterType::Triangle).grayscale();
    let gray32_png_b64 = encode_png_b64(&gray32)?;

    // Stage 3: 8×8 grayscale (AHash input) + the mean threshold.
    let gray8 = img.resize_exact(8, 8, FilterType::Triangle).grayscale();
    let gray8_l8 = gray8.to_luma8();
    let mean_u32: u32 = gray8_l8.as_raw().iter().map(|&v| v as u32).sum();
    let ahash_mean = (mean_u32 / 64) as u8;
    let gray8_png_b64 = encode_png_b64(&gray8)?;

    // Stage 4: final fingerprint — reuse the regular pipeline.
    let rec = fingerprint_with(bytes, 0, 0, pre)?;
    let fingerprint_hex = hex_lower(&rec.fingerprint);
    let fingerprint_bytes = rec.fingerprint.len();
    let config_hash = rec.config_hash;

    Ok(InspectImageResult {
        algorithm: ALGORITHM_MULTIHASH,
        width: w,
        height: h,
        original_png_b64,
        gray32_png_b64,
        gray8_png_b64,
        ahash_mean,
        fingerprint_hex,
        fingerprint_bytes,
        config_hash,
    })
}

#[cfg(feature = "inspect")]
fn encode_png_b64(img: &image::DynamicImage) -> Result<String> {
    use base64::Engine;
    let mut buf: Vec<u8> = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .map_err(|e| Error::Modality(format!("png encode: {e}")))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&buf))
}

#[cfg(feature = "inspect")]
fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}
