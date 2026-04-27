//! Audio fingerprinting via [`audiofp`].
//!
//! Wraps the Wang landmark fingerprinter (Shazam-style) by default; the
//! caller decodes upstream (e.g. via `audiofp::io::decode_to_mono_at`)
//! and passes mono f32 samples + sample rate.

use bytes::Bytes;

use crate::core::{Modality, Record};
use crate::error::{Error, Result};

/// Stable algorithm tag for Wang landmark hashes.
pub const ALGORITHM_WANG: &str = "audiofp-wang-v1";

/// Fingerprint a mono f32 sample buffer at `sample_rate` Hz with the
/// default Wang landmark fingerprinter.
pub fn fingerprint_wang(
    samples: &[f32],
    sample_rate: u32,
    tenant_id: u32,
    record_id: u64,
) -> Result<Record> {
    use audiofp::classical::Wang;
    use audiofp::{AudioBuffer, Fingerprinter, SampleRate};

    let rate = SampleRate::new(sample_rate)
        .ok_or_else(|| Error::Modality(format!("invalid sample rate {sample_rate}")))?;

    let mut wang = Wang::default();
    let out = wang
        .extract(AudioBuffer { samples, rate })
        .map_err(|e| Error::Modality(e.to_string()))?;

    // Wang hashes are bytemuck::Pod; cast the slice to bytes.
    let bytes = Bytes::copy_from_slice(bytemuck::cast_slice(&out.hashes));

    Ok(Record {
        tenant_id,
        record_id,
        modality: Modality::Audio,
        format_version: 1, // audiofp 0.2.x; replace with audiofp::FORMAT_VERSION when added.
        algorithm: ALGORITHM_WANG.into(),
        config_hash: 0,
        fingerprint: bytes,
        embedding: None,
        model_id: None,
        metadata: Bytes::new(),
    })
}
