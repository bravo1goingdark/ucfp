//! Workspace umbrella crate for Universal Content Fingerprinting (UCFP).
//!
//! This crate stitches together ingest normalization and canonicalization so
//! callers can operate over text payloads with a single API entry point.

pub use ufp_canonical::{
    CanonicalError, CanonicalizeConfig, CanonicalizedDocument, Token, canonicalize,
    collapse_whitespace, hash_text, tokenize,
};
pub use ufp_ingest::{
    CanonicalIngestRecord, CanonicalPayload, IngestConfig, IngestError, IngestMetadata,
    IngestPayload, IngestSource, RawIngestRecord, ingest, normalize_payload,
};
pub use ufp_perceptual::{
    PerceptualConfig, PerceptualError, PerceptualFingerprint, perceptualize_tokens,
};

use chrono::{DateTime, NaiveDate, Utc};
use std::error::Error;
use std::fmt;
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{Duration, Instant};

/// Errors that can occur while processing an ingest record through the pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipelineError {
    Ingest(IngestError),
    Canonical(CanonicalError),
    NonTextPayload,
    MissingCanonicalPayload,
    Perceptual(PerceptualError),
}

impl fmt::Display for PipelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PipelineError::Ingest(err) => write!(f, "ingest failure: {err}"),
            PipelineError::Canonical(err) => write!(f, "canonicalization failure: {err}"),
            PipelineError::NonTextPayload => write!(f, "payload is not text; cannot canonicalize"),
            PipelineError::MissingCanonicalPayload => {
                write!(f, "ingest succeeded without canonical payload")
            }
            PipelineError::Perceptual(err) => write!(f, "perceptual fingerprinting failed: {err}"),
        }
    }
}

impl Error for PipelineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            PipelineError::Ingest(err) => Some(err),
            PipelineError::Canonical(err) => Some(err),
            PipelineError::Perceptual(err) => Some(err),
            PipelineError::NonTextPayload | PipelineError::MissingCanonicalPayload => None,
        }
    }
}

impl From<IngestError> for PipelineError {
    fn from(value: IngestError) -> Self {
        PipelineError::Ingest(value)
    }
}

impl From<CanonicalError> for PipelineError {
    fn from(value: CanonicalError) -> Self {
        PipelineError::Canonical(value)
    }
}

impl From<PerceptualError> for PipelineError {
    fn from(value: PerceptualError) -> Self {
        PipelineError::Perceptual(value)
    }
}

/// Metrics observer for pipeline stages.
pub trait PipelineMetrics: Send + Sync {
    fn record_ingest(&self, latency: Duration, result: Result<(), IngestError>);
    fn record_canonical(&self, latency: Duration, result: Result<(), PipelineError>);
    fn record_perceptual(&self, latency: Duration, result: Result<(), PerceptualError>);
}

/// Install or clear the global pipeline metrics recorder.
pub fn set_pipeline_metrics(recorder: Option<Arc<dyn PipelineMetrics>>) {
    let lock = metrics_lock();
    let mut guard = lock.write().expect("pipeline metrics lock poisoned");
    *guard = recorder;
}

fn metrics_lock() -> &'static RwLock<Option<Arc<dyn PipelineMetrics>>> {
    static METRICS: OnceLock<RwLock<Option<Arc<dyn PipelineMetrics>>>> = OnceLock::new();
    METRICS.get_or_init(|| RwLock::new(None))
}

fn metrics_recorder() -> Option<Arc<dyn PipelineMetrics>> {
    let guard = metrics_lock()
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.clone()
}

struct MetricsSpan {
    recorder: Arc<dyn PipelineMetrics>,
    start: Instant,
}

impl MetricsSpan {
    fn start() -> Option<Self> {
        metrics_recorder().map(|recorder| Self {
            recorder,
            start: Instant::now(),
        })
    }

    fn record_ingest(self, result: Result<(), IngestError>) {
        self.recorder.record_ingest(self.start.elapsed(), result);
    }

    fn record_canonical(self, result: Result<(), PipelineError>) {
        self.recorder.record_canonical(self.start.elapsed(), result);
    }

    fn record_perceptual(self, result: Result<(), PerceptualError>) {
        self.recorder
            .record_perceptual(self.start.elapsed(), result);
    }
}

/// Process a raw ingest record end-to-end with explicit configuration.
/// Binary payloads produce a `PipelineError::NonTextPayload`.
pub fn process_record_with_configs(
    raw: RawIngestRecord,
    ingest_cfg: &IngestConfig,
    canonical_cfg: &CanonicalizeConfig,
) -> Result<CanonicalizedDocument, PipelineError> {
    let mut ingest_metrics = MetricsSpan::start();
    let canonical_record = match ingest(raw, ingest_cfg) {
        Ok(record) => {
            if let Some(span) = ingest_metrics.take() {
                span.record_ingest(Ok(()));
            }
            record
        }
        Err(err) => {
            if let Some(span) = ingest_metrics.take() {
                span.record_ingest(Err(err.clone()));
            }
            return Err(PipelineError::Ingest(err));
        }
    };

    let mut canonical_metrics = MetricsSpan::start();
    let payload = match canonical_record.normalized_payload.as_ref() {
        Some(payload) => payload,
        None => {
            let err = PipelineError::MissingCanonicalPayload;
            if let Some(span) = canonical_metrics.take() {
                span.record_canonical(Err(err.clone()));
            }
            return Err(err);
        }
    };

    match payload {
        CanonicalPayload::Text(text) => {
            match canonicalize(canonical_record.doc_id.as_str(), text, canonical_cfg) {
                Ok(doc) => {
                    if let Some(span) = canonical_metrics.take() {
                        span.record_canonical(Ok(()));
                    }
                    Ok(doc)
                }
                Err(err) => {
                    let pipeline_err = PipelineError::Canonical(err);
                    if let Some(span) = canonical_metrics.take() {
                        span.record_canonical(Err(pipeline_err.clone()));
                    }
                    Err(pipeline_err)
                }
            }
        }
        CanonicalPayload::Binary(_) => {
            let err = PipelineError::NonTextPayload;
            if let Some(span) = canonical_metrics.take() {
                span.record_canonical(Err(err.clone()));
            }
            Err(err)
        }
    }
}

/// Process a raw ingest record end-to-end using default ingest configuration.
pub fn process_record(
    raw: RawIngestRecord,
    cfg: &CanonicalizeConfig,
) -> Result<CanonicalizedDocument, PipelineError> {
    process_record_with_configs(raw, &IngestConfig::default(), cfg)
}

/// Run ingest, canonicalization, and perceptual fingerprinting in order.
/// Returns both the canonical document and the resulting perceptual fingerprint.
pub fn process_record_with_perceptual(
    raw: RawIngestRecord,
    canonical_cfg: &CanonicalizeConfig,
    perceptual_cfg: &PerceptualConfig,
) -> Result<(CanonicalizedDocument, PerceptualFingerprint), PipelineError> {
    process_record_with_perceptual_configs(
        raw,
        &IngestConfig::default(),
        canonical_cfg,
        perceptual_cfg,
    )
}

/// Pipeline helper that accepts explicit configuration for all stages.
pub fn process_record_with_perceptual_configs(
    raw: RawIngestRecord,
    ingest_cfg: &IngestConfig,
    canonical_cfg: &CanonicalizeConfig,
    perceptual_cfg: &PerceptualConfig,
) -> Result<(CanonicalizedDocument, PerceptualFingerprint), PipelineError> {
    let doc = process_record_with_configs(raw, ingest_cfg, canonical_cfg)?;
    let mut perceptual_metrics = MetricsSpan::start();
    match perceptualize_tokens(doc.tokens.as_slice(), perceptual_cfg) {
        Ok(fp) => {
            if let Some(span) = perceptual_metrics.take() {
                span.record_perceptual(Ok(()));
            }
            Ok((doc, fp))
        }
        Err(err) => {
            if let Some(span) = perceptual_metrics.take() {
                span.record_perceptual(Err(err.clone()));
            }
            Err(PipelineError::Perceptual(err))
        }
    }
}

/// Convenience helper that returns only the fingerprint using default ingest/canonical configs.
pub fn process_document(
    raw: RawIngestRecord,
    perceptual_cfg: &PerceptualConfig,
) -> Result<PerceptualFingerprint, PipelineError> {
    let canonical_cfg = CanonicalizeConfig::default();
    let (_, fp) = process_record_with_perceptual_configs(
        raw,
        &IngestConfig::default(),
        &canonical_cfg,
        perceptual_cfg,
    )?;
    Ok(fp)
}

/// Fingerprint helper that accepts configuration for all pipeline stages.
pub fn process_document_with_configs(
    raw: RawIngestRecord,
    ingest_cfg: &IngestConfig,
    canonical_cfg: &CanonicalizeConfig,
    perceptual_cfg: &PerceptualConfig,
) -> Result<PerceptualFingerprint, PipelineError> {
    let (_, fp) =
        process_record_with_perceptual_configs(raw, ingest_cfg, canonical_cfg, perceptual_cfg)?;
    Ok(fp)
}

fn demo_timestamp() -> DateTime<Utc> {
    let Some(date) = NaiveDate::from_ymd_opt(2025, 1, 1) else {
        panic!("invalid demo date components");
    };
    let Some(date_time) = date.and_hms_opt(0, 0, 0) else {
        panic!("invalid demo time components");
    };
    DateTime::<Utc>::from_naive_utc_and_offset(date_time, Utc)
}

/// Convenience helper that feeds the bundled `big_text.txt` sample through the full pipeline.
/// Useful for demos and integration smoke tests.
pub fn big_text_demo(
    perceptual_cfg: &PerceptualConfig,
) -> Result<(CanonicalizedDocument, PerceptualFingerprint), PipelineError> {
    const BIG_TEXT: &str = include_str!("../crates/ufp_canonical/examples/big_text.txt");

    let raw = RawIngestRecord {
        id: "demo-big-text".into(),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: Some("ucfp-demo".into()),
            doc_id: Some("big-text".into()),
            received_at: Some(demo_timestamp()),
            original_source: Some("crates/ufp_canonical/examples/big_text.txt".into()),
            attributes: None,
        },
        payload: Some(IngestPayload::Text(BIG_TEXT.to_string())),
    };

    process_record_with_perceptual(raw, &CanonicalizeConfig::default(), perceptual_cfg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, RwLock};
    use std::time::Duration;

    fn base_record(payload: IngestPayload) -> RawIngestRecord {
        RawIngestRecord {
            id: "ingest-1".into(),
            source: IngestSource::RawText,
            metadata: IngestMetadata {
                tenant_id: Some("tenant".into()),
                doc_id: Some("doc".into()),
                received_at: Some(demo_timestamp()),
                original_source: Some("origin".into()),
                attributes: None,
            },
            payload: Some(payload),
        }
    }

    #[test]
    fn process_record_canonicalizes_text() {
        let cfg = CanonicalizeConfig::default();
        let record = base_record(IngestPayload::Text(" Hello   Rust ".into()));

        let doc = process_record(record, &cfg).expect("canonicalization should succeed");
        assert_eq!(doc.canonical_text, "hello rust");
        assert_eq!(doc.tokens.len(), 2);
        assert_eq!(doc.tokens[0].text, "hello");
        assert_eq!(doc.tokens[1].text, "rust");
        assert_eq!(doc.doc_id, "doc");
    }

    #[test]
    fn process_record_rejects_binary_payload() {
        let cfg = CanonicalizeConfig::default();
        let record = RawIngestRecord {
            id: "ingest-binary".into(),
            source: IngestSource::File {
                filename: "data.bin".into(),
                content_type: None,
            },
            metadata: IngestMetadata {
                tenant_id: Some("tenant".into()),
                doc_id: Some("doc".into()),
                received_at: Some(demo_timestamp()),
                original_source: None,
                attributes: None,
            },
            payload: Some(IngestPayload::Binary(vec![0, 1, 2])),
        };

        let result = process_record(record, &cfg);
        assert!(matches!(result, Err(PipelineError::NonTextPayload)));
    }

    #[test]
    fn process_record_requires_payload() {
        let cfg = CanonicalizeConfig::default();
        let record = RawIngestRecord {
            id: "ingest-empty".into(),
            source: IngestSource::RawText,
            metadata: IngestMetadata {
                tenant_id: Some("tenant".into()),
                doc_id: Some("doc".into()),
                received_at: Some(demo_timestamp()),
                original_source: None,
                attributes: None,
            },
            payload: Some(IngestPayload::Text("   ".into())),
        };

        let result = process_record(record, &cfg);
        assert!(matches!(
            result,
            Err(PipelineError::Ingest(IngestError::EmptyNormalizedText))
        ));
    }

    #[test]
    fn process_record_deterministic_output() {
        let cfg = CanonicalizeConfig::default();
        let record_a = base_record(IngestPayload::Text(" Caf\u{00E9}\nRust ".into()));
        let record_b = base_record(IngestPayload::Text("Cafe\u{0301} RUST".into()));

        let doc_a = process_record(record_a, &cfg).expect("first canonicalization");
        let doc_b = process_record(record_b, &cfg).expect("second canonicalization");

        assert_eq!(doc_a.canonical_text, doc_b.canonical_text);
        assert_eq!(doc_a.sha256_hex, doc_b.sha256_hex);
    }

    #[test]
    fn process_record_with_perceptual_produces_fingerprint() {
        let canonical_cfg = CanonicalizeConfig::default();
        let perceptual_cfg = PerceptualConfig {
            k: 3, // ensure tokens >= k for the short input
            ..Default::default()
        };
        let record = base_record(IngestPayload::Text(
            "The quick brown fox jumps over the lazy dog".into(),
        ));

        let (doc, fp) = process_record_with_perceptual(record, &canonical_cfg, &perceptual_cfg)
            .expect("pipeline should succeed");

        assert!(!doc.canonical_text.is_empty());
        assert!(!fp.shingles.is_empty());
        assert_eq!(fp.meta.k, 3);
    }

    #[derive(Default)]
    struct CountingMetrics {
        events: Arc<RwLock<Vec<&'static str>>>,
    }

    impl CountingMetrics {
        fn new() -> Self {
            Self {
                events: Arc::new(RwLock::new(Vec::new())),
            }
        }

        fn snapshot(&self) -> Vec<&'static str> {
            self.events.read().unwrap().clone()
        }
    }

    impl PipelineMetrics for CountingMetrics {
        fn record_ingest(&self, _latency: Duration, result: Result<(), IngestError>) {
            let label = if result.is_ok() {
                "ingest_ok"
            } else {
                "ingest_err"
            };
            self.events.write().unwrap().push(label);
        }

        fn record_canonical(&self, _latency: Duration, result: Result<(), PipelineError>) {
            let label = if result.is_ok() {
                "canonical_ok"
            } else {
                "canonical_err"
            };
            self.events.write().unwrap().push(label);
        }

        fn record_perceptual(&self, _latency: Duration, result: Result<(), PerceptualError>) {
            let label = if result.is_ok() {
                "perceptual_ok"
            } else {
                "perceptual_err"
            };
            self.events.write().unwrap().push(label);
        }
    }

    #[test]
    fn metrics_recorder_tracks_pipeline_outcome() {
        let metrics = Arc::new(CountingMetrics::new());
        set_pipeline_metrics(Some(metrics.clone()));

        let canonical_cfg = CanonicalizeConfig::default();
        let perceptual_cfg = PerceptualConfig {
            k: 2,
            ..Default::default()
        };
        let record = base_record(IngestPayload::Text(
            "This is a metrics validation payload".into(),
        ));

        let result = process_record_with_perceptual(record, &canonical_cfg, &perceptual_cfg);

        assert!(result.is_ok());

        let events = metrics.snapshot();
        assert!(events.contains(&"ingest_ok"));
        assert!(events.contains(&"canonical_ok"));
        assert!(events.contains(&"perceptual_ok"));

        set_pipeline_metrics(None);
    }
}
