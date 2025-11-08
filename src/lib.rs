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

use chrono::{DateTime, NaiveDate, SecondsFormat, Utc};
use std::error::Error;
use std::fmt;
use std::io::{self, Write};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
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

/// Processing stage captured in observability events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineStage {
    Ingest,
    Canonical,
    Perceptual,
}

impl fmt::Display for PipelineStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            PipelineStage::Ingest => "ingest",
            PipelineStage::Canonical => "canonical",
            PipelineStage::Perceptual => "perceptual",
        };
        f.write_str(name)
    }
}

/// Outcome of a pipeline stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineEventStatus {
    Success,
    Failure,
}

impl fmt::Display for PipelineEventStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            PipelineEventStatus::Success => "success",
            PipelineEventStatus::Failure => "failure",
        };
        f.write_str(label)
    }
}

/// Structured observation describing the outcome of a pipeline stage.
#[derive(Debug, Clone)]
pub struct PipelineEvent {
    pub stage: PipelineStage,
    pub status: PipelineEventStatus,
    pub latency: Duration,
    pub record_id: String,
    pub doc_id: Option<String>,
    pub tenant_id: Option<String>,
    pub error: Option<String>,
}

impl PipelineEvent {
    fn from_outcome(
        stage: PipelineStage,
        context: &StageContext,
        latency: Duration,
        error: Option<String>,
    ) -> Self {
        let status = if error.is_some() {
            PipelineEventStatus::Failure
        } else {
            PipelineEventStatus::Success
        };
        Self {
            stage,
            status,
            latency,
            record_id: context.record_id.clone(),
            doc_id: context.doc_id.clone(),
            tenant_id: context.tenant_id.clone(),
            error,
        }
    }

    fn format_key_values(&self, include_timestamp: bool) -> String {
        let mut parts = Vec::new();
        if include_timestamp {
            let ts = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
            parts.push(format!("timestamp=\"{ts}\""));
        }
        let stage = self.stage;
        parts.push(format!("stage={stage}"));
        let status = self.status;
        parts.push(format!("status={status}"));
        let latency_us = self.latency.as_micros();
        parts.push(format!("latency_us={latency_us}"));
        let record_id = escape_kv(&self.record_id);
        parts.push(format!("record_id=\"{record_id}\""));
        if let Some(doc_id) = &self.doc_id {
            let doc_id = escape_kv(doc_id);
            parts.push(format!("doc_id=\"{doc_id}\""));
        }
        if let Some(tenant_id) = &self.tenant_id {
            let tenant_id = escape_kv(tenant_id);
            parts.push(format!("tenant_id=\"{tenant_id}\""));
        }
        if let Some(error) = &self.error {
            let error = escape_kv(error);
            parts.push(format!("error=\"{error}\""));
        }
        parts.join(" ")
    }
}

fn escape_kv(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

/// Hook for emitting structured events per pipeline stage.
pub trait PipelineEventLogger: Send + Sync {
    fn log(&self, event: &PipelineEvent);
}

/// Simple key-value logger that writes structured events to any writer.
pub struct KeyValueLogger {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    include_timestamp: bool,
}

impl KeyValueLogger {
    /// Create a logger that writes to stdout.
    pub fn stdout() -> Self {
        Self::new(Box::new(io::stdout()))
    }

    /// Create a logger backed by the provided writer.
    pub fn new(writer: Box<dyn Write + Send>) -> Self {
        Self {
            writer: Arc::new(Mutex::new(writer)),
            include_timestamp: true,
        }
    }

    /// Toggle timestamp emission for the structured log line.
    pub fn with_timestamps(mut self, include_timestamp: bool) -> Self {
        self.include_timestamp = include_timestamp;
        self
    }
}

impl PipelineEventLogger for KeyValueLogger {
    fn log(&self, event: &PipelineEvent) {
        if let Ok(mut writer) = self.writer.lock() {
            let line = event.format_key_values(self.include_timestamp);
            let _ = writeln!(writer, "{line}");
        }
    }
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

/// Install or clear the structured pipeline event logger.
pub fn set_pipeline_logger(logger: Option<Arc<dyn PipelineEventLogger>>) {
    let lock = logger_lock();
    let mut guard = lock.write().expect("pipeline logger lock poisoned");
    *guard = logger;
}

fn logger_lock() -> &'static RwLock<Option<Arc<dyn PipelineEventLogger>>> {
    static LOGGER: OnceLock<RwLock<Option<Arc<dyn PipelineEventLogger>>>> = OnceLock::new();
    LOGGER.get_or_init(|| RwLock::new(None))
}

fn pipeline_logger() -> Option<Arc<dyn PipelineEventLogger>> {
    let guard = logger_lock()
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.clone()
}

#[derive(Debug, Clone)]
struct StageContext {
    record_id: String,
    doc_id: Option<String>,
    tenant_id: Option<String>,
}

impl StageContext {
    fn from_raw(record: &RawIngestRecord) -> Self {
        Self {
            record_id: record.id.clone(),
            doc_id: record.metadata.doc_id.clone(),
            tenant_id: record.metadata.tenant_id.clone(),
        }
    }

    fn update_with_ingest(&mut self, record: &CanonicalIngestRecord) {
        self.record_id = record.id.clone();
        self.doc_id = Some(record.doc_id.clone());
        self.tenant_id = Some(record.tenant_id.clone());
    }

    fn from_ingest_record(record: &CanonicalIngestRecord) -> Self {
        Self {
            record_id: record.id.clone(),
            doc_id: Some(record.doc_id.clone()),
            tenant_id: Some(record.tenant_id.clone()),
        }
    }

    fn from_document(doc: &CanonicalizedDocument) -> Self {
        Self {
            record_id: doc.doc_id.clone(),
            doc_id: Some(doc.doc_id.clone()),
            tenant_id: None,
        }
    }
}

struct MetricsSpan {
    recorder: Option<Arc<dyn PipelineMetrics>>,
    logger: Option<Arc<dyn PipelineEventLogger>>,
    stage: PipelineStage,
    context: StageContext,
    start: Instant,
}

impl MetricsSpan {
    fn start(stage: PipelineStage, context: StageContext) -> Option<Self> {
        let recorder = metrics_recorder();
        let logger = pipeline_logger();
        if recorder.is_none() && logger.is_none() {
            return None;
        }
        Some(Self {
            recorder,
            logger,
            stage,
            context,
            start: Instant::now(),
        })
    }

    fn update_context<F>(&mut self, update: F)
    where
        F: FnOnce(&mut StageContext),
    {
        update(&mut self.context);
    }

    fn record_ingest(self, result: Result<(), IngestError>) {
        let latency = self.start.elapsed();
        self.emit_event(latency, result.as_ref().err().map(|e| e.to_string()));
        if let Some(recorder) = self.recorder {
            recorder.record_ingest(latency, result);
        }
    }

    fn record_canonical(self, result: Result<(), PipelineError>) {
        let latency = self.start.elapsed();
        self.emit_event(latency, result.as_ref().err().map(|e| e.to_string()));
        if let Some(recorder) = self.recorder {
            recorder.record_canonical(latency, result);
        }
    }

    fn record_perceptual(self, result: Result<(), PerceptualError>) {
        let latency = self.start.elapsed();
        self.emit_event(latency, result.as_ref().err().map(|e| e.to_string()));
        if let Some(recorder) = self.recorder {
            recorder.record_perceptual(latency, result);
        }
    }

    fn emit_event(&self, latency: Duration, error: Option<String>) {
        if let Some(logger) = self.logger.as_ref() {
            let event = PipelineEvent::from_outcome(self.stage, &self.context, latency, error);
            logger.log(&event);
        }
    }
}

/// Process a raw ingest record end-to-end with explicit configuration.
/// Binary payloads produce a `PipelineError::NonTextPayload`.
pub fn process_record_with_configs(
    raw: RawIngestRecord,
    ingest_cfg: &IngestConfig,
    canonical_cfg: &CanonicalizeConfig,
) -> Result<CanonicalizedDocument, PipelineError> {
    let mut ingest_metrics =
        MetricsSpan::start(PipelineStage::Ingest, StageContext::from_raw(&raw));
    let canonical_record = match ingest(raw, ingest_cfg) {
        Ok(record) => record,
        Err(err) => {
            if let Some(span) = ingest_metrics.take() {
                span.record_ingest(Err(err.clone()));
            }
            return Err(PipelineError::Ingest(err));
        }
    };

    if let Some(span) = ingest_metrics.as_mut() {
        span.update_context(|ctx| ctx.update_with_ingest(&canonical_record));
    }
    if let Some(span) = ingest_metrics.take() {
        span.record_ingest(Ok(()));
    }

    let mut canonical_metrics = MetricsSpan::start(
        PipelineStage::Canonical,
        StageContext::from_ingest_record(&canonical_record),
    );
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
    let mut perceptual_metrics =
        MetricsSpan::start(PipelineStage::Perceptual, StageContext::from_document(&doc));
    let token_refs: Vec<&str> = doc.tokens.iter().map(|t| t.text.as_str()).collect();
    match perceptualize_tokens(token_refs.as_slice(), perceptual_cfg) {
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

    #[derive(Default)]
    struct RecordingLogger {
        stages: Arc<RwLock<Vec<PipelineStage>>>,
    }

    impl RecordingLogger {
        fn snapshot(&self) -> Vec<PipelineStage> {
            self.stages.read().unwrap().clone()
        }
    }

    impl PipelineEventLogger for RecordingLogger {
        fn log(&self, event: &PipelineEvent) {
            self.stages.write().unwrap().push(event.stage);
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

    #[test]
    fn structured_logger_receives_stage_events() {
        let logger = Arc::new(RecordingLogger::default());
        set_pipeline_logger(Some(logger.clone()));

        let canonical_cfg = CanonicalizeConfig::default();
        let perceptual_cfg = PerceptualConfig {
            k: 2,
            ..Default::default()
        };
        let record = base_record(IngestPayload::Text("Structured logging validation".into()));

        let result = process_record_with_perceptual(record, &canonical_cfg, &perceptual_cfg);
        assert!(result.is_ok());

        let stages = logger.snapshot();
        let expected = [
            PipelineStage::Ingest,
            PipelineStage::Canonical,
            PipelineStage::Perceptual,
        ];
        assert!(
            stages.ends_with(&expected),
            "structured events missing or out of order: {stages:?}"
        );

        set_pipeline_logger(None);
    }
}
