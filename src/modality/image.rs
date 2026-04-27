//! Image fingerprinting via [`imgfprint`].
//!
//! Wraps [`imgfprint::ImageFingerprinter::fingerprint`] (or the path /
//! preprocess-config variants) into a uniform [`Record`].

use bytes::Bytes;
use imgfprint::{ImageFingerprinter, MultiHashFingerprint, PreprocessConfig};

use crate::core::{Modality, Record};
use crate::error::{Error, Result};

/// Stable algorithm tag for the imgfprint multi-hash bundle.
pub const ALGORITHM: &str = "imgfprint-multihash-v1";

/// Fingerprint raw image bytes (PNG / JPEG / WebP / GIF / BMP) with
/// imgfprint's default preprocess.
pub fn fingerprint(bytes: &[u8], tenant_id: u32, record_id: u64) -> Result<Record> {
    fingerprint_with(bytes, tenant_id, record_id, &PreprocessConfig::default())
}

/// Fingerprint with a tunable [`PreprocessConfig`] (max_input_bytes,
/// max_dimension, min_dimension). See `imgfprint::PreprocessConfig`.
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
        algorithm: ALGORITHM,
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
