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
        text: None,
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
        text: None,
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
        text: None,
    })
}

// ─────────────────────────────────────────────────────────────────────────
// Neural ONNX log-mel embedder — feature `audio-neural`.
// ─────────────────────────────────────────────────────────────────────────

/// Optional per-call overrides for [`fingerprint_neural_with`].
#[cfg(feature = "audio-neural")]
#[derive(Clone, Debug, Default)]
pub struct NeuralOpts {
    /// Override the upper edge of the mel filterbank in Hz. `None`
    /// keeps the SDK default (`sample_rate / 2`).
    pub fmax: Option<f32>,
}

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
    fingerprint_neural_with(
        samples,
        sample_rate,
        model_path,
        &NeuralOpts::default(),
        tenant_id,
        record_id,
    )
}

/// Configurable variant of [`fingerprint_neural`]. Honors
/// [`NeuralOpts::fmax`] when supplied; falls back to `sample_rate / 2`.
#[cfg(feature = "audio-neural")]
pub fn fingerprint_neural_with(
    samples: &[f32],
    sample_rate: u32,
    model_path: &str,
    opts: &NeuralOpts,
    tenant_id: u32,
    record_id: u64,
) -> Result<Record> {
    use audiofp::neural::{NeuralEmbedder, NeuralEmbedderConfig};
    use audiofp::{AudioBuffer, Fingerprinter, SampleRate};

    let mut cfg = NeuralEmbedderConfig::new(model_path.to_string());
    cfg.sample_rate = sample_rate;
    cfg.fmax = opts.fmax.unwrap_or(sample_rate as f32 / 2.0);

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
        text: None,
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

/// Optional per-call overrides for [`detect_watermark_with`].
#[cfg(feature = "audio-watermark")]
#[derive(Clone, Debug, Default)]
pub struct WatermarkOpts {
    /// Override the detection threshold in `[0, 1]`. SDK default is 0.5.
    pub threshold: Option<f32>,
}

/// Run the AudioSeal-compatible watermark detector loaded from
/// `model_path` over the given samples.
#[cfg(feature = "audio-watermark")]
pub fn detect_watermark(
    samples: &[f32],
    sample_rate: u32,
    model_path: &str,
) -> Result<WatermarkReport> {
    detect_watermark_with(samples, sample_rate, model_path, &WatermarkOpts::default())
}

/// Configurable variant of [`detect_watermark`]. Honors
/// [`WatermarkOpts::threshold`] when supplied.
#[cfg(feature = "audio-watermark")]
pub fn detect_watermark_with(
    samples: &[f32],
    sample_rate: u32,
    model_path: &str,
    opts: &WatermarkOpts,
) -> Result<WatermarkReport> {
    use audiofp::watermark::{WatermarkConfig, WatermarkDetector};
    use audiofp::{AudioBuffer, SampleRate};

    let mut cfg = WatermarkConfig::new(model_path.to_string());
    cfg.sample_rate = sample_rate;
    if let Some(t) = opts.threshold {
        cfg.threshold = t;
    }

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
            text: None,
        }]
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Pipeline inspect — surfaces the intermediate audio-pipeline stages so
// the playgrounds PipelineInspector UI can render each step.
// ─────────────────────────────────────────────────────────────────────────

/// One stage payload for the audio pipeline inspector.
#[cfg(feature = "inspect")]
#[derive(Clone, Debug, serde::Serialize)]
pub struct InspectAudioResult {
    /// Stable algorithm identifier (always Wang for now).
    pub algorithm: &'static str,
    /// Sample rate the pipeline ran at (Hz).
    pub sample_rate: u32,
    /// Total duration of the input in seconds.
    pub duration_secs: f32,
    /// Downsampled amplitude envelope: 256 buckets of max-abs sample
    /// magnitude per bucket. Renders as a tiny waveform strip.
    pub envelope: Vec<f32>,
    /// Linear-frequency log-magnitude spectrogram as a base64 PNG
    /// (viridis colormap, low end = dark purple, high end = yellow).
    /// Width = downsampled time frames, height = downsampled freq bins.
    pub spectrogram_png_b64: String,
    /// Width of the linear spectrogram PNG (= number of time frames).
    pub spec_width: u32,
    /// Height of the linear spectrogram PNG (= number of freq bins).
    pub spec_height: u32,
    /// Mel-scaled log-power spectrogram as a viridis-coloured PNG.
    /// Mel is closer to perceptual loudness — low frequencies get more
    /// detail, high frequencies are compressed. Same time axis as the
    /// linear spectrogram so the UI can stack them.
    pub mel_spec_png_b64: String,
    /// Width of the mel spectrogram PNG.
    pub mel_spec_width: u32,
    /// Height of the mel spectrogram PNG (= mel band count).
    pub mel_spec_height: u32,
    /// Lower frequency edge of the mel filterbank (Hz).
    pub mel_fmin_hz: f32,
    /// Upper frequency edge of the mel filterbank (Hz).
    pub mel_fmax_hz: f32,
    /// First N peaks the picker emitted, as `(t_ms, freq_hz, db)`
    /// triples. Capped to keep payloads small.
    pub peaks: Vec<InspectAudioPeak>,
    /// Total number of peaks the picker emitted.
    pub total_peaks: usize,
    /// Wang anchor → target landmark pairs (the actual fingerprint
    /// inputs). Each pair is the line the UI draws between two peaks
    /// to visualise WHY a hash was emitted.
    pub landmark_pairs: Vec<InspectAudioLandmark>,
    /// Total number of landmark pairs emitted (uncapped count).
    pub total_landmarks: usize,
    /// Final Wang fingerprint as hex.
    pub fingerprint_hex: String,
    /// Length of the underlying fingerprint in bytes.
    pub fingerprint_bytes: usize,
}

/// One Wang landmark pair — anchor → target peak coordinates in real
/// (ms, Hz) units. The UI draws these as faint lines on top of the
/// spectrogram to make the pairing rule visible.
#[cfg(feature = "inspect")]
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct InspectAudioLandmark {
    /// Anchor peak time (ms from start).
    pub t1_ms: f32,
    /// Anchor peak frequency (Hz).
    pub f1_hz: f32,
    /// Target peak time (ms from start).
    pub t2_ms: f32,
    /// Target peak frequency (Hz).
    pub f2_hz: f32,
}

/// One peak surfaced by the audio inspector — picked landmarks from
/// the STFT magnitude grid. Coordinates are in real units (ms / Hz).
#[cfg(feature = "inspect")]
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct InspectAudioPeak {
    /// Peak position in milliseconds from the start of the input.
    pub t_ms: f32,
    /// Peak frequency in Hz.
    pub freq_hz: f32,
    /// Peak magnitude in dB (relative to peak-magnitude floor).
    pub db: f32,
}

/// Run the audio pipeline and surface every intermediate stage. Always
/// uses the Wang landmark fingerprinter for now (Panako/Haitsma can be
/// added when their UIs land).
///
/// `samples` must be mono f32 PCM at `sample_rate` Hz. The function
/// downsamples internally for visualisation; the final fingerprint
/// goes through the regular `fingerprint_wang` path so config_hash and
/// hex round-trip a normal ingest.
#[cfg(feature = "inspect")]
pub fn inspect_audio(samples: &[f32], sample_rate: u32) -> Result<InspectAudioResult> {
    use audiofp::classical::WangConfig;
    use audiofp::dsp::mel::{MelFilterBank, MelScale};
    use audiofp::dsp::peaks::{PeakPicker, PeakPickerConfig};
    use audiofp::dsp::stft::{ShortTimeFFT, StftConfig};
    use audiofp::dsp::windows::WindowKind;

    if samples.is_empty() {
        return Err(Error::Modality("audio inspect: empty sample buffer".into()));
    }
    let duration_secs = samples.len() as f32 / sample_rate as f32;

    // Stage 1 — amplitude envelope (256 buckets).
    let envelope = downsample_envelope(samples, 256);

    // Stage 2a — STFT magnitudes. n_fft=1024 / hop=256 gives ~31.25 ms /
    // 8 ms cadence at 8 kHz.
    let n_fft = 1024usize;
    let hop = 256usize;
    let mut stft = ShortTimeFFT::new(StftConfig {
        n_fft,
        hop,
        window: WindowKind::Hann,
        center: true,
    });
    let (mag_flat, n_frames, n_bins) = stft.magnitude_flat(samples);

    // Linear-frequency spectrogram (viridis). 96 bins is enough vertical
    // resolution to pick out peaks without making the PNG huge.
    let (lin_grid, lin_w, lin_h) = downsample_spec(&mag_flat, n_frames, n_bins, 256, 96);
    let lin_spec_png = encode_spec_png_b64_viridis(&lin_grid, lin_w, lin_h)?;

    // Stage 2b — Mel-scaled spectrogram. Mel-spaced bins compress the
    // upper frequencies and expand the low end where most spectral
    // structure (formants, harmonics, drums) actually lives.
    let mel_n = 64usize;
    let mel_fmin = 0.0f32;
    let mel_fmax = (sample_rate as f32) / 2.0;
    let bank = MelFilterBank::new(
        mel_n,
        n_fft,
        sample_rate,
        mel_fmin,
        mel_fmax,
        MelScale::Slaney,
    );
    let mel_grid_full = compute_mel_grid(&bank, &mag_flat, n_frames, n_bins, mel_n);
    // Already mel-scaled vertically; just downsample the time axis to
    // keep the PNG narrow. Skip the bin axis (mel_n is already 64).
    let target_w_mel = 256u32.min(n_frames as u32).max(1);
    let mel_grid = downsample_time_only(&mel_grid_full, n_frames, mel_n, target_w_mel);
    let mel_spec_png = encode_spec_png_b64_viridis(&mel_grid, target_w_mel, mel_n as u32)?;

    // Stage 3 — pick peaks on the *full-resolution* linear magnitude
    // grid so the (t_ms, freq_hz) coordinates match what Wang sees.
    let frames_per_sec = sample_rate as f32 / hop as f32;
    let mut picker = PeakPicker::new(PeakPickerConfig::default());
    let raw_peaks = picker.pick(&mag_flat, n_frames, n_bins, frames_per_sec);
    let total_peaks = raw_peaks.len();

    let max_mag = mag_flat.iter().copied().fold(0.0f32, f32::max).max(1e-9);
    let bin_hz = sample_rate as f32 / n_fft as f32;
    let frame_ms = 1000.0 * hop as f32 / sample_rate as f32;

    const MAX_PEAKS_RETURNED: usize = 256;
    let peaks: Vec<InspectAudioPeak> = raw_peaks
        .iter()
        .take(MAX_PEAKS_RETURNED)
        .map(|p| InspectAudioPeak {
            t_ms: p.t_frame as f32 * frame_ms,
            freq_hz: p.f_bin as f32 * bin_hz,
            db: 20.0 * (p.mag.max(1e-9) / max_mag).log10(),
        })
        .collect();

    // Stage 3b — Wang landmark pairs. Re-runs Wang's pairing rule on
    // the picked peaks so the UI can draw anchor → target lines on
    // the spectrogram, making the "why was this hashed?" visible.
    let cfg = WangConfig::default();
    let landmark_pairs = compute_landmark_pairs(&raw_peaks, &cfg, frame_ms, bin_hz, 256);
    let total_landmarks = count_total_landmark_pairs(&raw_peaks, &cfg);

    // Stage 4 — final Wang fingerprint via the regular pipeline. Soft
    // fail so short clips that don't satisfy Wang's `min_samples` still
    // get the envelope / spectrogram / peaks / landmarks panes.
    let (fingerprint_hex, fingerprint_bytes) = match fingerprint_wang(samples, sample_rate, 0, 0) {
        Ok(rec) => (hex_lower_audio(&rec.fingerprint), rec.fingerprint.len()),
        Err(_) => (String::new(), 0),
    };

    Ok(InspectAudioResult {
        algorithm: ALGORITHM_WANG,
        sample_rate,
        duration_secs,
        envelope,
        spectrogram_png_b64: lin_spec_png,
        spec_width: lin_w,
        spec_height: lin_h,
        mel_spec_png_b64: mel_spec_png,
        mel_spec_width: target_w_mel,
        mel_spec_height: mel_n as u32,
        mel_fmin_hz: mel_fmin,
        mel_fmax_hz: mel_fmax,
        peaks,
        total_peaks,
        landmark_pairs,
        total_landmarks,
        fingerprint_hex,
        fingerprint_bytes,
    })
}

/// Downsample a sample buffer to `buckets` cells of max-abs magnitude.
/// Used for the amplitude envelope strip in the inspector UI.
#[cfg(feature = "inspect")]
fn downsample_envelope(samples: &[f32], buckets: usize) -> Vec<f32> {
    let buckets = buckets.max(1);
    if samples.len() <= buckets {
        return samples.iter().map(|s| s.abs()).collect();
    }
    let step = samples.len() as f64 / buckets as f64;
    (0..buckets)
        .map(|i| {
            let lo = (i as f64 * step).floor() as usize;
            let hi = (((i + 1) as f64) * step).ceil() as usize;
            let hi = hi.min(samples.len());
            samples[lo..hi]
                .iter()
                .copied()
                .fold(0.0f32, |acc, s| acc.max(s.abs()))
        })
        .collect()
}

/// Downsample a (frames × bins) magnitude grid to (target_w × target_h)
/// via simple max-pooling. Avoids pulling fast_image_resize for what is
/// already a visualisation aid.
#[cfg(feature = "inspect")]
fn downsample_spec(
    mag_flat: &[f32],
    n_frames: usize,
    n_bins: usize,
    target_w: u32,
    target_h: u32,
) -> (Vec<f32>, u32, u32) {
    let w = target_w.min(n_frames as u32).max(1);
    let h = target_h.min(n_bins as u32).max(1);
    let mut out = vec![0.0f32; (w * h) as usize];
    let xs = n_frames as f64 / w as f64;
    let ys = n_bins as f64 / h as f64;
    for x in 0..w {
        let f0 = (x as f64 * xs).floor() as usize;
        let f1 = (((x + 1) as f64) * xs).ceil() as usize;
        let f1 = f1.min(n_frames);
        for y in 0..h {
            let b0 = (y as f64 * ys).floor() as usize;
            let b1 = (((y + 1) as f64) * ys).ceil() as usize;
            let b1 = b1.min(n_bins);
            let mut peak = 0.0f32;
            for f in f0..f1 {
                let row = &mag_flat[f * n_bins + b0..f * n_bins + b1];
                for v in row.iter().copied() {
                    if v > peak {
                        peak = v;
                    }
                }
            }
            // Flip y so low frequencies sit at the bottom of the image.
            let y_img = h - 1 - y;
            out[(y_img * w + x) as usize] = peak;
        }
    }
    (out, w, h)
}

/// Encode a magnitude grid as a viridis-coloured base64 PNG.
///
/// Magnitudes are log-scaled (dB) against the grid's own peak and
/// clamped to a -60 dB floor so background noise doesn't wash the
/// foreground out. Each cell maps through the [`viridis`] palette so
/// quiet bins land on dark purple, loud bins on bright yellow — a
/// far more legible spectrogram than the previous grayscale.
#[cfg(feature = "inspect")]
fn encode_spec_png_b64_viridis(grid: &[f32], w: u32, h: u32) -> Result<String> {
    use base64::Engine;
    use image::{ImageFormat, Rgb, RgbImage};

    let max_mag = grid.iter().copied().fold(0.0f32, f32::max).max(1e-9);
    let mut img = RgbImage::new(w, h);
    const DB_FLOOR: f32 = -60.0;
    for (i, &m) in grid.iter().enumerate() {
        let db = 20.0 * (m.max(1e-9) / max_mag).log10();
        let t = ((db - DB_FLOOR) / -DB_FLOOR).clamp(0.0, 1.0);
        let [r, g, b] = viridis(t);
        let x = (i as u32) % w;
        let y = (i as u32) / w;
        img.put_pixel(x, y, Rgb([r, g, b]));
    }
    let mut buf: Vec<u8> = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), ImageFormat::Png)
        .map_err(|e| Error::Modality(format!("spectrogram png encode: {e}")))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&buf))
}

/// 9-stop viridis approximation. Linearly interpolated; at this
/// resolution the result is visually indistinguishable from matplotlib's
/// 256-stop table for spectrogram rendering and ships in ~80 bytes
/// of static data instead of 768.
#[cfg(feature = "inspect")]
fn viridis(t: f32) -> [u8; 3] {
    const STOPS: [[u8; 3]; 9] = [
        [68, 1, 84],    // 0.000  dark purple
        [72, 35, 116],  // 0.125
        [64, 67, 135],  // 0.250
        [52, 94, 141],  // 0.375
        [41, 121, 142], // 0.500
        [32, 144, 140], // 0.625
        [34, 167, 132], // 0.750
        [121, 209, 81], // 0.875
        [253, 231, 37], // 1.000  yellow
    ];
    let t = t.clamp(0.0, 1.0);
    let scaled = t * (STOPS.len() - 1) as f32;
    let lo = scaled.floor() as usize;
    let hi = (lo + 1).min(STOPS.len() - 1);
    let f = scaled - lo as f32;
    let lerp = |a: u8, b: u8| -> u8 {
        let av = a as f32;
        let bv = b as f32;
        ((av + (bv - av) * f).round() as u32).min(255) as u8
    };
    [
        lerp(STOPS[lo][0], STOPS[hi][0]),
        lerp(STOPS[lo][1], STOPS[hi][1]),
        lerp(STOPS[lo][2], STOPS[hi][2]),
    ]
}

/// Apply the mel filterbank to every STFT magnitude frame and return a
/// row-major `(n_frames × n_mels)` log-mel grid, with low frequencies
/// at the bottom of the image (y is flipped from the natural index
/// order so the rendering matches the linear spectrogram convention).
#[cfg(feature = "inspect")]
fn compute_mel_grid(
    bank: &audiofp::dsp::mel::MelFilterBank,
    mag_flat: &[f32],
    n_frames: usize,
    n_bins: usize,
    n_mels: usize,
) -> Vec<f32> {
    let mut out = vec![0.0_f32; n_frames * n_mels];
    let mut frame_out = vec![0.0_f32; n_mels];
    for f in 0..n_frames {
        let mag = &mag_flat[f * n_bins..(f + 1) * n_bins];
        bank.log_mel(mag, &mut frame_out);
        // The grid is laid out time-first (one frame per row) — flip y
        // so the encode loop renders low-frequency at the bottom.
        for (k, v) in frame_out.iter().enumerate() {
            let y = n_mels - 1 - k;
            // log_mel returns log10(power); pull back to a magnitude-like
            // scale so the encode_spec_png_b64_viridis dB math (which
            // expects raw magnitudes) sees comparable numbers.
            out[f * n_mels + y] = 10.0_f32.powf(*v / 2.0);
        }
    }
    // Transpose to (height, width) layout the encoder expects:
    // it indexes as `out[y * w + x]`, so we need rows = mel band,
    // cols = time frame.
    let mut transposed = vec![0.0_f32; n_frames * n_mels];
    for f in 0..n_frames {
        for k in 0..n_mels {
            transposed[k * n_frames + f] = out[f * n_mels + k];
        }
    }
    transposed
}

/// Downsample only the time axis of a `(height × n_frames)` grid to
/// `(height × target_w)` via max-pool. Used when the height is already
/// the desired resolution (e.g. mel bands are picked up-front).
#[cfg(feature = "inspect")]
fn downsample_time_only(grid: &[f32], n_frames: usize, height: usize, target_w: u32) -> Vec<f32> {
    let w = (target_w as usize).min(n_frames).max(1);
    let mut out = vec![0.0_f32; w * height];
    let xs = n_frames as f64 / w as f64;
    for x in 0..w {
        let f0 = (x as f64 * xs).floor() as usize;
        let f1 = (((x + 1) as f64) * xs).ceil() as usize;
        let f1 = f1.min(n_frames);
        for y in 0..height {
            let mut peak = 0.0f32;
            for f in f0..f1 {
                let v = grid[y * n_frames + f];
                if v > peak {
                    peak = v;
                }
            }
            out[y * w + x] = peak;
        }
    }
    out
}

/// Re-run Wang's anchor → target pairing rule on the picked peaks so
/// the UI can draw the actual hash inputs (not just standalone dots).
///
/// For each anchor peak: look forward in time within the configured
/// target zone (`Δt ≤ target_zone_t`, `|Δf| ≤ target_zone_f`) and
/// take up to `fan_out` neighbouring peaks. Each (anchor, target)
/// becomes one Wang hash — and one line on the inspector image.
#[cfg(feature = "inspect")]
fn compute_landmark_pairs(
    peaks: &[audiofp::dsp::peaks::Peak],
    cfg: &audiofp::classical::WangConfig,
    frame_ms: f32,
    bin_hz: f32,
    cap: usize,
) -> Vec<InspectAudioLandmark> {
    let mut out: Vec<InspectAudioLandmark> = Vec::with_capacity(cap);
    for (i, anchor) in peaks.iter().enumerate() {
        let mut taken = 0usize;
        for target in &peaks[i + 1..] {
            if out.len() >= cap {
                return out;
            }
            let dt = target.t_frame as i32 - anchor.t_frame as i32;
            if dt <= 0 {
                continue;
            }
            if dt > cfg.target_zone_t as i32 {
                break; // peaks are time-sorted; no later peak fits
            }
            let df = (target.f_bin as i32 - anchor.f_bin as i32).abs();
            if df > cfg.target_zone_f as i32 {
                continue;
            }
            out.push(InspectAudioLandmark {
                t1_ms: anchor.t_frame as f32 * frame_ms,
                f1_hz: anchor.f_bin as f32 * bin_hz,
                t2_ms: target.t_frame as f32 * frame_ms,
                f2_hz: target.f_bin as f32 * bin_hz,
            });
            taken += 1;
            if taken >= cfg.fan_out as usize {
                break;
            }
        }
    }
    out
}

/// Total number of landmark pairs Wang would emit for this peak set —
/// surfaced separately so the UI can show "showing 256 of N" when
/// `compute_landmark_pairs` capped the result.
#[cfg(feature = "inspect")]
fn count_total_landmark_pairs(
    peaks: &[audiofp::dsp::peaks::Peak],
    cfg: &audiofp::classical::WangConfig,
) -> usize {
    let mut total = 0usize;
    for (i, anchor) in peaks.iter().enumerate() {
        let mut taken = 0usize;
        for target in &peaks[i + 1..] {
            let dt = target.t_frame as i32 - anchor.t_frame as i32;
            if dt <= 0 {
                continue;
            }
            if dt > cfg.target_zone_t as i32 {
                break;
            }
            let df = (target.f_bin as i32 - anchor.f_bin as i32).abs();
            if df > cfg.target_zone_f as i32 {
                continue;
            }
            taken += 1;
            total += 1;
            if taken >= cfg.fan_out as usize {
                break;
            }
        }
    }
    total
}

#[cfg(feature = "inspect")]
fn hex_lower_audio(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}
