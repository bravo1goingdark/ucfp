//! Audio fingerprinting via [`audiofp`].
//!
//! This module wraps every algorithm exposed by `audiofp` into a uniform
//! [`Record`] envelope so the rest of UCFP can ingest them through a
//! single code path. Per-algorithm functions are gated behind the
//! corresponding `audio-*` features in `Cargo.toml`.
//!
//! # Algorithms
//!
//! | Function                   | Output                    | Feature gate        |
//! | -------------------------- | ------------------------- | ------------------- |
//! | [`fingerprint_wang`]       | Wang landmark hashes      | `audio` (default)   |
//! | `fingerprint_panako`       | Panako triplet hashes     | `audio-panako`      |
//! | `fingerprint_haitsma`      | Philips robust hash       | `audio-haitsma`     |
//! | `fingerprint_neural`       | ONNX log-mel embeddings   | `audio-neural`      |
//! | `detect_watermark`         | AudioSeal-style detection | `audio-watermark`   |
//! | `StreamingWangSession`     | Push/finalize streamer    | `audio-streaming`   |
//!
//! Watermark detection does **not** produce a [`Record`] — it returns a
//! [`WatermarkReport`] because the result is descriptive ("is this audio
//! watermarked?"), not something that should be persisted as a
//! comparable fingerprint.

use bytes::Bytes;

use crate::core::{Modality, Record};
use crate::error::{Error, Result};

/// Stable algorithm tag for Wang landmark hashes.
pub const ALGORITHM_WANG: &str = "audiofp-wang-v1";
/// Stable algorithm tag for Panako triplet hashes.
pub const ALGORITHM_PANAKO: &str = "audiofp-panako-v1";
/// Stable algorithm tag for Haitsma–Kalker / Philips robust hashes.
pub const ALGORITHM_HAITSMA: &str = "audiofp-haitsma-v1";
/// Stable algorithm tag for ONNX log-mel neural embeddings.
pub const ALGORITHM_NEURAL: &str = "audiofp-neural-v1";
/// Stable algorithm tag for AudioSeal-compatible watermark detection.
pub const ALGORITHM_WATERMARK: &str = "audiofp-watermark-v1";

// ─────────────────────────────────────────────────────────────────────────
// Wang landmark fingerprinter (default; no per-algorithm feature flag).
// ─────────────────────────────────────────────────────────────────────────

/// Fingerprint a mono f32 sample buffer at `sample_rate` Hz with the
/// default Wang landmark fingerprinter.
pub fn fingerprint_wang(
    samples: &[f32],
    sample_rate: u32,
    tenant_id: u32,
    record_id: u64,
) -> Result<Record> {
    fingerprint_wang_with(
        samples,
        sample_rate,
        &audiofp::classical::WangConfig::default(),
        tenant_id,
        record_id,
    )
}

/// Fingerprint a mono f32 sample buffer with a tunable
/// [`audiofp::classical::WangConfig`] (fan_out, target_zone_t/f,
/// peaks_per_sec, min_anchor_mag_db).
pub fn fingerprint_wang_with(
    samples: &[f32],
    sample_rate: u32,
    cfg: &audiofp::classical::WangConfig,
    tenant_id: u32,
    record_id: u64,
) -> Result<Record> {
    use audiofp::classical::Wang;
    use audiofp::{AudioBuffer, Fingerprinter, SampleRate};

    let rate = SampleRate::new(sample_rate)
        .ok_or_else(|| Error::Modality(format!("invalid sample rate {sample_rate}")))?;

    let mut wang = Wang::new(cfg.clone());
    let out = wang
        .extract(AudioBuffer { samples, rate })
        .map_err(|e| Error::Modality(e.to_string()))?;

    // WangHash is bytemuck::Pod; cast the slice to bytes.
    let bytes = Bytes::copy_from_slice(bytemuck::cast_slice(&out.hashes));

    Ok(Record {
        tenant_id,
        record_id,
        modality: Modality::Audio,
        format_version: 1, // audiofp 0.3 has no FORMAT_VERSION constant yet.
        algorithm: ALGORITHM_WANG.into(),
        config_hash: 0,
        fingerprint: bytes,
        embedding: None,
        model_id: None,
        metadata: Bytes::new(),
    })
}

// ─────────────────────────────────────────────────────────────────────────
// Panako triplet fingerprinter — feature `audio-panako`.
// ─────────────────────────────────────────────────────────────────────────

/// Fingerprint with the default [`audiofp::classical::Panako`] config.
#[cfg(feature = "audio-panako")]
pub fn fingerprint_panako(
    samples: &[f32],
    sample_rate: u32,
    tenant_id: u32,
    record_id: u64,
) -> Result<Record> {
    fingerprint_panako_with(
        samples,
        sample_rate,
        &audiofp::classical::PanakoConfig::default(),
        tenant_id,
        record_id,
    )
}

/// Fingerprint with a tunable [`audiofp::classical::PanakoConfig`].
#[cfg(feature = "audio-panako")]
pub fn fingerprint_panako_with(
    samples: &[f32],
    sample_rate: u32,
    cfg: &audiofp::classical::PanakoConfig,
    tenant_id: u32,
    record_id: u64,
) -> Result<Record> {
    use audiofp::classical::Panako;
    use audiofp::{AudioBuffer, Fingerprinter, SampleRate};

    let rate = SampleRate::new(sample_rate)
        .ok_or_else(|| Error::Modality(format!("invalid sample rate {sample_rate}")))?;

    let mut p = Panako::new(cfg.clone());
    let out = p
        .extract(AudioBuffer { samples, rate })
        .map_err(|e| Error::Modality(e.to_string()))?;

    let bytes = Bytes::copy_from_slice(bytemuck::cast_slice(&out.hashes));

    Ok(Record {
        tenant_id,
        record_id,
        modality: Modality::Audio,
        format_version: 1,
        algorithm: ALGORITHM_PANAKO.into(),
        config_hash: 0,
        fingerprint: bytes,
        embedding: None,
        model_id: None,
        metadata: Bytes::new(),
    })
}

// ─────────────────────────────────────────────────────────────────────────
// Haitsma robust hash — feature `audio-haitsma`.
// ─────────────────────────────────────────────────────────────────────────

/// Fingerprint with the default [`audiofp::classical::Haitsma`] config.
#[cfg(feature = "audio-haitsma")]
pub fn fingerprint_haitsma(
    samples: &[f32],
    sample_rate: u32,
    tenant_id: u32,
    record_id: u64,
) -> Result<Record> {
    fingerprint_haitsma_with(
        samples,
        sample_rate,
        &audiofp::classical::HaitsmaConfig::default(),
        tenant_id,
        record_id,
    )
}

/// Fingerprint with a tunable [`audiofp::classical::HaitsmaConfig`].
#[cfg(feature = "audio-haitsma")]
pub fn fingerprint_haitsma_with(
    samples: &[f32],
    sample_rate: u32,
    cfg: &audiofp::classical::HaitsmaConfig,
    tenant_id: u32,
    record_id: u64,
) -> Result<Record> {
    use audiofp::classical::Haitsma;
    use audiofp::dsp::resample;
    use audiofp::{AudioBuffer, Fingerprinter, SampleRate};

    // Haitsma–Kalker requires 5 kHz mono input; resample if the caller
    // supplies a different (standard) rate.
    const HAITSMA_SR: u32 = 5_000;
    let resampled: Vec<f32>;
    let (buf, rate) = if sample_rate == HAITSMA_SR {
        (samples, SampleRate::new(HAITSMA_SR).unwrap())
    } else {
        resampled = resample::linear(samples, sample_rate, HAITSMA_SR);
        (resampled.as_slice(), SampleRate::new(HAITSMA_SR).unwrap())
    };

    let mut h = Haitsma::new(cfg.clone());
    let out = h
        .extract(AudioBuffer { samples: buf, rate })
        .map_err(|e| Error::Modality(e.to_string()))?;

    // HaitsmaFingerprint::frames is `Vec<u32>` — cast to bytes.
    let bytes = Bytes::copy_from_slice(bytemuck::cast_slice(&out.frames));

    Ok(Record {
        tenant_id,
        record_id,
        modality: Modality::Audio,
        format_version: 1,
        algorithm: ALGORITHM_HAITSMA.into(),
        config_hash: 0,
        fingerprint: bytes,
        embedding: None,
        model_id: None,
        metadata: Bytes::new(),
    })
}

// ─────────────────────────────────────────────────────────────────────────
// Neural ONNX log-mel embedder — feature `audio-neural`.
// ─────────────────────────────────────────────────────────────────────────

/// Compute log-mel ONNX embeddings using a model loaded from
/// `model_path`.
///
/// Each analysis window emits one embedding; the resulting [`Record`]
/// stores all window embeddings packed contiguously in `fingerprint`
/// (as f32 little-endian bytes via `bytemuck::cast_slice`) and lifts the
/// **first** window into the optional `embedding` slot so the matcher's
/// vector-knn path works out of the box.
#[cfg(feature = "audio-neural")]
pub fn fingerprint_neural(
    samples: &[f32],
    sample_rate: u32,
    model_path: &str,
    tenant_id: u32,
    record_id: u64,
) -> Result<Record> {
    use audiofp::neural::{NeuralEmbedder, NeuralEmbedderConfig};
    use audiofp::{AudioBuffer, Fingerprinter, SampleRate};

    let mut cfg = NeuralEmbedderConfig::new(model_path.to_string());
    cfg.sample_rate = sample_rate;
    cfg.fmax = sample_rate as f32 / 2.0;

    let rate = SampleRate::new(sample_rate)
        .ok_or_else(|| Error::Modality(format!("invalid sample rate {sample_rate}")))?;

    let mut emb = NeuralEmbedder::new(cfg).map_err(|e| Error::Modality(e.to_string()))?;
    let out = emb
        .extract(AudioBuffer { samples, rate })
        .map_err(|e| Error::Modality(e.to_string()))?;

    if out.embeddings.is_empty() {
        return Err(Error::Modality(
            "neural embedder produced no embeddings".into(),
        ));
    }

    // Pack every window's vector contiguously as f32 little-endian bytes.
    let total = out.embeddings.iter().map(|e| e.vector.len()).sum::<usize>();
    let mut flat = Vec::with_capacity(total);
    for emb in &out.embeddings {
        flat.extend_from_slice(&emb.vector);
    }
    let bytes = Bytes::copy_from_slice(bytemuck::cast_slice(&flat));

    // Lift the first window's embedding for cosine-knn fast path.
    let first_embedding = out.embeddings[0].vector.clone();

    Ok(Record {
        tenant_id,
        record_id,
        modality: Modality::Audio,
        format_version: 1,
        algorithm: ALGORITHM_NEURAL.into(),
        config_hash: 0,
        fingerprint: bytes,
        embedding: Some(first_embedding),
        model_id: Some(model_path.to_string()),
        metadata: Bytes::new(),
    })
}

// ─────────────────────────────────────────────────────────────────────────
// Watermark detection — feature `audio-watermark`.
// ─────────────────────────────────────────────────────────────────────────

/// Result of running the AudioSeal-style watermark detector.
///
/// Unlike the fingerprinters, this does not produce a comparable
/// `Record` — the detector emits a per-call decision plus an optional
/// payload (the bit-packed message recovered from the carrier).
#[derive(Clone, Debug)]
pub struct WatermarkReport {
    /// `true` when the mean detection score exceeds the configured
    /// threshold.
    pub detected: bool,
    /// Decoded message bytes (little-endian packing of the 32-bit
    /// message word). `None` when `detected == false`.
    pub payload: Option<Vec<u8>>,
    /// Mean detection confidence in `[0.0, 1.0]`.
    pub confidence: f32,
}

/// Run the AudioSeal-compatible watermark detector loaded from
/// `model_path` over the given samples.
#[cfg(feature = "audio-watermark")]
pub fn detect_watermark(
    samples: &[f32],
    sample_rate: u32,
    model_path: &str,
) -> Result<WatermarkReport> {
    use audiofp::watermark::{WatermarkConfig, WatermarkDetector};
    use audiofp::{AudioBuffer, SampleRate};

    let mut cfg = WatermarkConfig::new(model_path.to_string());
    cfg.sample_rate = sample_rate;

    let rate = SampleRate::new(sample_rate)
        .ok_or_else(|| Error::Modality(format!("invalid sample rate {sample_rate}")))?;

    let mut det = WatermarkDetector::new(cfg).map_err(|e| Error::Modality(e.to_string()))?;
    let out = det
        .detect(AudioBuffer { samples, rate })
        .map_err(|e| Error::Modality(e.to_string()))?;

    let payload = if out.detected {
        Some(out.message.to_le_bytes().to_vec())
    } else {
        None
    };

    Ok(WatermarkReport {
        detected: out.detected,
        payload,
        confidence: out.confidence,
    })
}

// ─────────────────────────────────────────────────────────────────────────
// Streaming Wang wrapper — feature `audio-streaming`.
// ─────────────────────────────────────────────────────────────────────────

/// Push-based streaming Wang fingerprint session.
///
/// R3 owns the multipart HTTP plumbing; this struct exposes the minimum
/// surface needed by it: `push` for inbound chunks, `finalize` to drain
/// any pending material at end-of-stream. Each `push` / `finalize` call
/// returns a (possibly empty) batch of `Record`s — one per emitted
/// landmark group, all sharing the supplied `(tenant_id, record_id)`.
#[cfg(feature = "audio-streaming")]
pub struct StreamingWangSession {
    inner: audiofp::classical::StreamingWang,
    tenant_id: u32,
    record_id: u64,
}

#[cfg(feature = "audio-streaming")]
impl StreamingWangSession {
    /// Build a session at the canonical Wang sample rate (8 kHz).
    /// `sample_rate` is accepted for parity with the offline API and
    /// validated; mismatched rates return [`Error::Modality`].
    pub fn new(sample_rate: u32, tenant_id: u32, record_id: u64) -> Result<Self> {
        if sample_rate != 8_000 {
            return Err(Error::Modality(format!(
                "Wang requires 8 kHz mono input (got {sample_rate} Hz); resample upstream"
            )));
        }
        Ok(Self {
            inner: audiofp::classical::StreamingWang::default(),
            tenant_id,
            record_id,
        })
    }

    /// Feed a chunk of mono PCM samples; returns whatever `Record`s
    /// became available during this push (typically zero or one).
    pub fn push(&mut self, samples: &[f32]) -> Result<Vec<Record>> {
        use audiofp::StreamingFingerprinter;
        let frames = self.inner.push(samples);
        Ok(self.frames_to_records(frames))
    }

    /// Drain any pending fingerprint material at end-of-stream.
    pub fn finalize(&mut self) -> Result<Vec<Record>> {
        use audiofp::StreamingFingerprinter;
        let frames = self.inner.flush();
        Ok(self.frames_to_records(frames))
    }

    fn frames_to_records(
        &self,
        frames: Vec<(audiofp::TimestampMs, audiofp::classical::WangHash)>,
    ) -> Vec<Record> {
        if frames.is_empty() {
            return Vec::new();
        }
        // Pack every emitted hash into a single Record: streaming consumers
        // assemble timeline by accumulating multiple Records under the
        // same (tenant_id, record_id) per their own retention policy.
        let hashes: Vec<audiofp::classical::WangHash> =
            frames.into_iter().map(|(_, h)| h).collect();
        let bytes = Bytes::copy_from_slice(bytemuck::cast_slice(&hashes));
        vec![Record {
            tenant_id: self.tenant_id,
            record_id: self.record_id,
            modality: Modality::Audio,
            format_version: 1,
            algorithm: ALGORITHM_WANG.into(),
            config_hash: 0,
            fingerprint: bytes,
            embedding: None,
            model_id: None,
            metadata: Bytes::new(),
        }]
    }
}
