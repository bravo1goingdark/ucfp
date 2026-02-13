//! Workspace umbrella crate for Universal Content Fingerprinting (UCFP).
//!
//! The `ucfp` crate re-exports the ingest, canonical, perceptual, and semantic
//! layers so applications can drive the full pipeline through a single
//! dependency. The unified [`process_pipeline`] function with [`PipelineStageConfig`]
//! orchestrates all stages end-to-end, providing a single entry point for
//! canonicalization, perceptual fingerprinting, and semantic embedding.
//!
//! ## Quick start
//!
//! The [`process_pipeline`] helper accepts a raw ingest record, a stage configuration,
//! and the configuration bundles that describe how each stage should behave.
//! Use [`PipelineStageConfig::Canonical`] for ingest + canonicalization only,
//! [`PipelineStageConfig::Perceptual`] to include perceptual fingerprinting,
//! or [`PipelineStageConfig::Semantic`] for semantic embeddings.
//!
//! ```ignore
//! use chrono::Utc;
//! use ucfp::{
//!     process_pipeline, PipelineStageConfig, CanonicalizeConfig, IngestConfig,
//!     IngestMetadata, IngestPayload, IngestSource, PerceptualConfig, RawIngestRecord,
//!     SemanticConfig,
//! };
//!
//! # fn demo() -> Result<(), ucfp::PipelineError> {
//! let ingest_config = IngestConfig::default();
//! let canonical_config = CanonicalizeConfig::default();
//! let perceptual_config = PerceptualConfig::default();
//! let semantic_config = SemanticConfig::default();
//!
//! let record = RawIngestRecord {
//!     id: "doc-123".into(),
//!     source: IngestSource::RawText,
//!     metadata: IngestMetadata {
//!         tenant_id: Some("tenant-a".into()),
//!         doc_id: None,
//!         received_at: Some(Utc::now()),
//!         original_source: None,
//!         attributes: None,
//!     },
//!     payload: Some(IngestPayload::Text("Hello, world!".into())),
//! };
//!
//! // Canonical only
//! let (canonical, _, _) = process_pipeline(
//!     record.clone(),
//!     PipelineStageConfig::Canonical,
//!     &ingest_config,
//!     &canonical_config,
//!     None,
//!     None,
//! )?;
//!
//! // With perceptual fingerprint
//! let (_, fingerprint, _) = process_pipeline(
//!     record.clone(),
//!     PipelineStageConfig::Perceptual,
//!     &ingest_config,
//!     &canonical_config,
//!     Some(&perceptual_config),
//!     None,
//! )?;
//!
//! // With semantic embedding
//! let (_, _, embedding) = process_pipeline(
//!     record,
//!     PipelineStageConfig::Semantic,
//!     &ingest_config,
//!     &canonical_config,
//!     None,
//!     Some(&semantic_config),
//! )?;
//! # Ok(())
//! # }
//! ```
//!
//! For workloads that need both perceptual and semantic outputs, use
//! [`PipelineStageConfig::Perceptual`] with both config options provided.
//!
//! ## Observability
//!
//! Metrics and structured logs can be captured by installing a
//! [`PipelineMetrics`] recorder via [`set_pipeline_metrics`] and/or a
//! [`PipelineEventLogger`] with [`set_pipeline_logger`]. Both hooks receive the
//! `record_id`, optional document/tenant identifiers, and the concrete stage
//! outcome so deployments can correlate with upstream systems. The ingest and
//! canonical configs typically inject those identifiers (for example from HTTP
//! headers) while perceptual and semantic configs fine-tune downstream
//! processing; the observability hooks therefore expose the same context that
//! operators configure in those stages. `PipelineMetrics` is best suited for
//! emitting latency/histogram telemetry, whereas `PipelineEventLogger` provides
//! structured events for centralized logging.
//!
//! In typical services these hooks are registered once during startup alongside
//! construction of the ingest/canonical/perceptual/semantic configs, ensuring
//! that every call to [`process_pipeline`] shares a consistent view of pipeline
//! behavior and instrumentation.
//!
//! ## Indexing and downstream integration
//!
//! The canonical document, perceptual fingerprint, and optional semantic
//! embedding produced by [`process_pipeline`] map directly into the index types
//! exposed by the companion [`index`](https://docs.rs/index) crate. The
//! [`IndexRecord`](https://docs.rs/index/latest/index/struct.IndexRecord.html)
//! struct mirrors the combined outputs returned by [`process_pipeline`] so
//! search or deduplication services can ingest them without translation.
//! When semantic embeddings are disabled the struct fields simply remain
//! `None`, allowing downstream systems to handle mixed-mode deployments.
//!
//! ## Errors
//!
//! Failures produced by any layer converge on [`PipelineError`], which maps the
//! source error and preserves metadata for downstream handling. Callers can
//! distinguish between ingest, canonical, perceptual, semantic, or
//! non-text-payload failures without needing to depend on the individual
//! workspace crates.

pub use canonical::{
    canonicalize, collapse_whitespace, hash_text, tokenize, CanonicalError, CanonicalizeConfig,
    CanonicalizedDocument, Token,
};
pub use index::IndexError;
pub use ingest::{
    ingest, normalize_payload, CanonicalIngestRecord, CanonicalPayload, IngestConfig, IngestError,
    IngestMetadata, IngestPayload, IngestSource, RawIngestRecord,
};
pub use matcher::{MatchError, Matcher};
pub use perceptual::{
    perceptualize_tokens, PerceptualConfig, PerceptualError, PerceptualFingerprint,
};
pub use semantic::{semanticize, SemanticConfig, SemanticEmbedding, SemanticError};

pub mod config;

use chrono::{DateTime, NaiveDate, SecondsFormat, Utc};
use std::error::Error;
use std::fmt;
use std::io::{self, Write};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant};

/// Errors that can occur while processing an ingest record through the pipeline.
#[derive(Debug, Clone)]
pub enum PipelineError {
    Ingest(IngestError),
    Canonical(CanonicalError),
    NonTextPayload,
    MissingCanonicalPayload,
    Perceptual(PerceptualError),
    Semantic(SemanticError),
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
            PipelineError::Perceptual(err) => {
                write!(f, "perceptual fingerprinting failed: {err}")
            }
            PipelineError::Semantic(err) => {
                write!(f, "semantic embedding failed: {err}")
            }
        }
    }
}

impl Error for PipelineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            PipelineError::Ingest(err) => Some(err),
            PipelineError::Canonical(err) => Some(err),
            PipelineError::Perceptual(err) => Some(err),
            PipelineError::Semantic(err) => Some(err),
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

impl From<SemanticError> for PipelineError {
    fn from(value: SemanticError) -> Self {
        PipelineError::Semantic(value)
    }
}

/// Metrics observer for pipeline stages.
pub trait PipelineMetrics: Send + Sync {
    fn record_ingest(&self, latency: Duration, result: Result<(), IngestError>);
    fn record_canonical(&self, latency: Duration, result: Result<(), PipelineError>);
    fn record_perceptual(&self, latency: Duration, result: Result<(), PerceptualError>);
    fn record_semantic(&self, latency: Duration, result: Result<(), SemanticError>);
    fn record_index(&self, latency: Duration, result: Result<(), IndexError>);
    fn record_match(&self, latency: Duration, result: Result<(), MatchError>);
}

/// Processing stage captured in observability events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineStage {
    Ingest,
    Canonical,
    Perceptual,
    Semantic,
    Index,
    Match,
}

impl fmt::Display for PipelineStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            PipelineStage::Ingest => "ingest",
            PipelineStage::Canonical => "canonical",
            PipelineStage::Perceptual => "perceptual",
            PipelineStage::Semantic => "semantic",
            PipelineStage::Index => "index",
            PipelineStage::Match => "match",
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
        // Pre-allocate: base fields (stage, status, latency_us, record_id) + optional fields
        let capacity = 4
            + (include_timestamp as usize)
            + self.doc_id.as_ref().map_or(0, |_| 1)
            + self.tenant_id.as_ref().map_or(0, |_| 1)
            + self.error.as_ref().map_or(0, |_| 1);
        let mut parts = Vec::with_capacity(capacity);
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
    fn new(record_id: String) -> Self {
        Self {
            record_id,
            doc_id: None,
            tenant_id: None,
        }
    }

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

    fn record_semantic(self, result: Result<(), SemanticError>) {
        let latency = self.start.elapsed();
        self.emit_event(latency, result.as_ref().err().map(|e| e.to_string()));
        if let Some(recorder) = self.recorder {
            recorder.record_semantic(latency, result);
        }
    }

    fn record_index(self, result: Result<(), IndexError>) {
        let latency = self.start.elapsed();
        self.emit_event(latency, result.as_ref().err().map(|e| e.to_string()));
        if let Some(recorder) = self.recorder {
            recorder.record_index(latency, result);
        }
    }

    fn record_match(self, result: Result<(), MatchError>) {
        let latency = self.start.elapsed();
        self.emit_event(latency, result.as_ref().err().map(|e| e.to_string()));
        if let Some(recorder) = self.recorder {
            recorder.record_match(latency, result);
        }
    }

    fn emit_event(&self, latency: Duration, error: Option<String>) {
        if let Some(logger) = self.logger.as_ref() {
            let event = PipelineEvent::from_outcome(self.stage, &self.context, latency, error);
            logger.log(&event);
        }
    }
}

/// Pipeline stages to execute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineStageConfig {
    /// Run ingest and canonicalization only.
    Canonical,
    /// Run full pipeline through perceptual fingerprinting.
    Perceptual,
    /// Run full pipeline through semantic embedding.
    Semantic,
}

/// Process a raw ingest record through configured pipeline stages.
///
/// This unified function replaces the previous 10+ variant functions.
/// Use `PipelineStageConfig` to control which stages execute.
///
/// # Examples
///
/// ```ignore
/// // Canonical only (replaces process_record)
/// let doc = process_pipeline(
///     raw,
///     PipelineStageConfig::Canonical,
///     &IngestConfig::default(),
///     &CanonicalizeConfig::default(),
///     None,
///     None,
/// )?;
///
/// // With perceptual fingerprint (replaces process_record_with_perceptual)
/// let (doc, fp) = process_pipeline(
///     raw,
///     PipelineStageConfig::Perceptual,
///     &IngestConfig::default(),
///     &CanonicalizeConfig::default(),
///     Some(&PerceptualConfig::default()),
///     None,
/// )?;
/// ```
pub fn process_pipeline(
    raw: RawIngestRecord,
    stage: PipelineStageConfig,
    ingest_cfg: &IngestConfig,
    canonical_cfg: &CanonicalizeConfig,
    perceptual_cfg: Option<&PerceptualConfig>,
    semantic_cfg: Option<&SemanticConfig>,
) -> Result<
    (
        CanonicalizedDocument,
        Option<PerceptualFingerprint>,
        Option<SemanticEmbedding>,
    ),
    PipelineError,
> {
    // --- Ingest Stage ---
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

    // --- Canonicalization Stage ---
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

    let doc = match payload {
        CanonicalPayload::Text(text) => {
            match canonicalize(canonical_record.doc_id.as_str(), text, canonical_cfg) {
                Ok(doc) => {
                    if let Some(span) = canonical_metrics.take() {
                        span.record_canonical(Ok(()));
                    }
                    doc
                }
                Err(err) => {
                    let pipeline_err = PipelineError::Canonical(err);
                    if let Some(span) = canonical_metrics.take() {
                        span.record_canonical(Err(pipeline_err.clone()));
                    }
                    return Err(pipeline_err);
                }
            }
        }
        _ => {
            let err = PipelineError::NonTextPayload;
            if let Some(span) = canonical_metrics.take() {
                span.record_canonical(Err(err.clone()));
            }
            return Err(err);
        }
    };

    if stage == PipelineStageConfig::Canonical {
        return Ok((doc, None, None));
    }

    // --- Perceptual Stage ---
    let fingerprint = if stage == PipelineStageConfig::Perceptual || perceptual_cfg.is_some() {
        let cfg = perceptual_cfg.ok_or(PipelineError::Perceptual(
            perceptual::PerceptualError::InvalidConfigVersion { version: 0 },
        ))?;
        let mut perceptual_metrics =
            MetricsSpan::start(PipelineStage::Perceptual, StageContext::from_document(&doc));
        let token_refs: Vec<&str> = doc.tokens.iter().map(|t| t.text.as_str()).collect();
        match perceptualize_tokens(token_refs.as_slice(), cfg) {
            Ok(fp) => {
                if let Some(span) = perceptual_metrics.take() {
                    span.record_perceptual(Ok(()));
                }
                Some(fp)
            }
            Err(err) => {
                if let Some(span) = perceptual_metrics.take() {
                    span.record_perceptual(Err(err.clone()));
                }
                return Err(PipelineError::Perceptual(err));
            }
        }
    } else {
        None
    };

    if stage == PipelineStageConfig::Perceptual {
        return Ok((doc, fingerprint, None));
    }

    // --- Semantic Stage ---
    let embedding = if stage == PipelineStageConfig::Semantic || semantic_cfg.is_some() {
        let cfg = semantic_cfg.ok_or(PipelineError::Semantic(
            semantic::SemanticError::Inference("missing semantic config".into()),
        ))?;
        let span = MetricsSpan::start(PipelineStage::Semantic, StageContext::from_document(&doc));

        let result = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| {
                handle.block_on(async {
                    semanticize(doc.doc_id.as_str(), doc.canonical_text.as_str(), cfg).await
                })
            })
        } else {
            tokio::runtime::Runtime::new().unwrap().block_on(async {
                semanticize(doc.doc_id.as_str(), doc.canonical_text.as_str(), cfg).await
            })
        };

        match result {
            Ok(emb) => {
                if let Some(s) = span {
                    s.record_semantic(Ok(()));
                }
                Some(emb)
            }
            Err(err) => {
                if let Some(s) = span {
                    s.record_semantic(Err(err.clone()));
                }
                return Err(PipelineError::Semantic(err));
            }
        }
    } else {
        None
    };

    Ok((doc, fingerprint, embedding))
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
    const BIG_TEXT: &str = include_str!("big_text.txt");

    let raw = RawIngestRecord {
        id: "demo-big-text".into(),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: Some("ucfp-demo".into()),
            doc_id: Some("big-text".into()),
            received_at: Some(demo_timestamp()),
            original_source: Some("crates/canonical/examples/big_text.txt".into()),
            attributes: None,
        },
        payload: Some(IngestPayload::Text(BIG_TEXT.to_string())),
    };

    process_pipeline(
        raw,
        PipelineStageConfig::Perceptual,
        &IngestConfig::default(),
        &CanonicalizeConfig::default(),
        Some(perceptual_cfg),
        None,
    )
    .map(|(doc, fp, _)| (doc, fp.unwrap()))
}

/// Execute an index upsert operation with metrics tracking.
/// Wraps `index.upsert()` and records latency to the metrics pipeline.
pub fn index_upsert_with_metrics(
    index: &index::UfpIndex,
    record: &index::IndexRecord,
) -> Result<(), IndexError> {
    let span = MetricsSpan::start(
        PipelineStage::Index,
        StageContext::new(record.canonical_hash.clone()),
    );
    let result = index.upsert(record);
    if let Some(s) = span {
        s.record_index(result.as_ref().map_err(|e| e.clone()).map(|_| ()));
    }
    result
}

/// Execute a match query with metrics tracking.
/// Wraps `matcher.match_document()` and records latency to the metrics pipeline.
pub fn match_document_with_metrics(
    matcher: &matcher::Matcher,
    request: &matcher::MatchRequest,
) -> Result<Vec<matcher::MatchHit>, MatchError> {
    let span = MetricsSpan::start(
        PipelineStage::Match,
        StageContext::new(request.query_text.clone()),
    );
    let result = matcher.match_document(request);
    if let Some(s) = span {
        s.record_match(result.as_ref().map_err(|e| e.clone()).map(|_| ()));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn logger_test_mutex() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn process_record_canonicalizes_text() {
        let canonical_cfg = CanonicalizeConfig::default();
        let ingest_cfg = IngestConfig::default();
        let record = base_record(IngestPayload::Text(" Hello   Rust ".into()));

        let (doc, _, _) = process_pipeline(
            record,
            PipelineStageConfig::Canonical,
            &ingest_cfg,
            &canonical_cfg,
            None,
            None,
        )
        .expect("canonicalization should succeed");
        assert_eq!(doc.canonical_text, "hello rust");
        assert_eq!(doc.tokens.len(), 2);
        assert_eq!(doc.tokens[0].text, "hello");
        assert_eq!(doc.tokens[1].text, "rust");
        assert_eq!(doc.doc_id, "doc");
    }

    #[test]
    fn process_record_rejects_binary_payload() {
        let canonical_cfg = CanonicalizeConfig::default();
        let ingest_cfg = IngestConfig::default();
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

        let result = process_pipeline(
            record,
            PipelineStageConfig::Canonical,
            &ingest_cfg,
            &canonical_cfg,
            None,
            None,
        );
        assert!(matches!(result, Err(PipelineError::NonTextPayload)));
    }

    #[test]
    fn process_record_requires_payload() {
        let canonical_cfg = CanonicalizeConfig::default();
        let ingest_cfg = IngestConfig::default();
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

        let result = process_pipeline(
            record,
            PipelineStageConfig::Canonical,
            &ingest_cfg,
            &canonical_cfg,
            None,
            None,
        );
        assert!(matches!(
            result,
            Err(PipelineError::Ingest(IngestError::EmptyNormalizedText))
        ));
    }

    #[test]
    fn process_record_deterministic_output() {
        let canonical_cfg = CanonicalizeConfig::default();
        let ingest_cfg = IngestConfig::default();
        let record_a = base_record(IngestPayload::Text(" Caf\u{00E9}\nRust ".into()));
        let record_b = base_record(IngestPayload::Text("Cafe\u{0301} RUST".into()));

        let (doc_a, _, _) = process_pipeline(
            record_a,
            PipelineStageConfig::Canonical,
            &ingest_cfg,
            &canonical_cfg,
            None,
            None,
        )
        .expect("first canonicalization");
        let (doc_b, _, _) = process_pipeline(
            record_b,
            PipelineStageConfig::Canonical,
            &ingest_cfg,
            &canonical_cfg,
            None,
            None,
        )
        .expect("second canonicalization");

        assert_eq!(doc_a.canonical_text, doc_b.canonical_text);
        assert_eq!(doc_a.sha256_hex, doc_b.sha256_hex);
    }

    #[test]
    fn process_record_with_perceptual_produces_fingerprint() {
        let canonical_cfg = CanonicalizeConfig::default();
        let ingest_cfg = IngestConfig::default();
        let perceptual_cfg = PerceptualConfig {
            k: 3, // ensure tokens >= k for the short input
            ..Default::default()
        };
        let record = base_record(IngestPayload::Text(
            "The quick brown fox jumps over the lazy dog".into(),
        ));

        let (doc, fp, _) = process_pipeline(
            record,
            PipelineStageConfig::Perceptual,
            &ingest_cfg,
            &canonical_cfg,
            Some(&perceptual_cfg),
            None,
        )
        .expect("pipeline should succeed");
        let fp = fp.unwrap();

        assert!(!doc.canonical_text.is_empty());
        assert!(!fp.shingles.is_empty());
        assert_eq!(fp.meta.k, 3);
    }

    #[test]
    fn process_record_with_semantic_produces_embedding() {
        let canonical_cfg = CanonicalizeConfig::default();
        let ingest_cfg = IngestConfig::default();
        let semantic_cfg = SemanticConfig {
            tier: "fast".into(),
            mode: "fast".into(),
            ..Default::default()
        };
        let record = base_record(IngestPayload::Text("Embeddings make search easier".into()));

        let (doc, _, embedding) = process_pipeline(
            record,
            PipelineStageConfig::Semantic,
            &ingest_cfg,
            &canonical_cfg,
            None,
            Some(&semantic_cfg),
        )
        .expect("semantic pipeline should succeed");
        let embedding = embedding.unwrap();

        assert_eq!(embedding.doc_id, doc.doc_id);
        assert!(!embedding.vector.is_empty());
        assert!(embedding.embedding_dim > 0);
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

        fn record_semantic(&self, _latency: Duration, result: Result<(), SemanticError>) {
            let label = if result.is_ok() {
                "semantic_ok"
            } else {
                "semantic_err"
            };
            self.events.write().unwrap().push(label);
        }

        fn record_index(&self, _latency: Duration, result: Result<(), IndexError>) {
            let label = if result.is_ok() {
                "index_ok"
            } else {
                "index_err"
            };
            self.events.write().unwrap().push(label);
        }

        fn record_match(&self, _latency: Duration, result: Result<(), MatchError>) {
            let label = if result.is_ok() {
                "match_ok"
            } else {
                "match_err"
            };
            self.events.write().unwrap().push(label);
        }
    }

    #[derive(Default)]
    struct RecordingLogger {
        events: Arc<RwLock<Vec<PipelineEvent>>>,
    }

    impl RecordingLogger {
        fn snapshot(&self) -> Vec<PipelineEvent> {
            self.events.read().unwrap().clone()
        }
    }

    impl PipelineEventLogger for RecordingLogger {
        fn log(&self, event: &PipelineEvent) {
            self.events.write().unwrap().push(event.clone());
        }
    }

    #[test]
    fn metrics_recorder_tracks_pipeline_outcome() {
        let metrics = Arc::new(CountingMetrics::new());
        set_pipeline_metrics(Some(metrics.clone()));

        let canonical_cfg = CanonicalizeConfig::default();
        let ingest_cfg = IngestConfig::default();
        let perceptual_cfg = PerceptualConfig {
            k: 2,
            ..Default::default()
        };
        let record = base_record(IngestPayload::Text(
            "This is a metrics validation payload".into(),
        ));

        let result = process_pipeline(
            record,
            PipelineStageConfig::Perceptual,
            &ingest_cfg,
            &canonical_cfg,
            Some(&perceptual_cfg),
            None,
        );

        assert!(result.is_ok());

        let semantic_cfg = SemanticConfig {
            mode: "fast".into(),
            tier: "fast".into(),
            ..Default::default()
        };
        let semantic_record = base_record(IngestPayload::Text(
            "Semantic metrics validation payload".into(),
        ));

        let semantic_result = process_pipeline(
            semantic_record,
            PipelineStageConfig::Semantic,
            &ingest_cfg,
            &canonical_cfg,
            Some(&perceptual_cfg),
            Some(&semantic_cfg),
        );
        assert!(semantic_result.is_ok());

        let events = metrics.snapshot();
        assert!(events.contains(&"ingest_ok"));
        assert!(events.contains(&"canonical_ok"));
        assert!(events.contains(&"perceptual_ok"));
        assert!(events.contains(&"semantic_ok"));

        set_pipeline_metrics(None);
    }

    #[test]
    fn structured_logger_receives_stage_events() {
        let _guard = logger_test_mutex()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let logger = Arc::new(RecordingLogger::default());
        set_pipeline_logger(Some(logger.clone()));

        let canonical_cfg = CanonicalizeConfig::default();
        let ingest_cfg = IngestConfig::default();
        let perceptual_cfg = PerceptualConfig {
            k: 2,
            ..Default::default()
        };
        let record = RawIngestRecord {
            id: "logger-perceptual".into(),
            source: IngestSource::RawText,
            metadata: IngestMetadata {
                tenant_id: Some("logger".into()),
                doc_id: Some("logger-doc-perceptual".into()),
                received_at: Some(demo_timestamp()),
                original_source: None,
                attributes: None,
            },
            payload: Some(IngestPayload::Text("Structured logging validation".into())),
        };

        let result = process_pipeline(
            record,
            PipelineStageConfig::Perceptual,
            &ingest_cfg,
            &canonical_cfg,
            Some(&perceptual_cfg),
            None,
        );
        assert!(result.is_ok());

        let stages: Vec<_> = logger
            .snapshot()
            .into_iter()
            .filter(|event| event.doc_id.as_deref() == Some("logger-doc-perceptual"))
            .map(|event| event.stage)
            .collect();
        let expected = [
            PipelineStage::Ingest,
            PipelineStage::Canonical,
            PipelineStage::Perceptual,
        ];
        assert!(
            stages == expected,
            "structured events missing or out of order for logger-doc-perceptual: {stages:?}"
        );

        set_pipeline_logger(None);
    }

    #[test]
    fn structured_logger_records_semantic_stage() {
        let _guard = logger_test_mutex()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let logger = Arc::new(RecordingLogger::default());
        set_pipeline_logger(Some(logger.clone()));

        let canonical_cfg = CanonicalizeConfig::default();
        let ingest_cfg = IngestConfig::default();
        let semantic_cfg = SemanticConfig {
            mode: "fast".into(),
            tier: "fast".into(),
            ..Default::default()
        };
        let record = RawIngestRecord {
            id: "logger-semantic".into(),
            source: IngestSource::RawText,
            metadata: IngestMetadata {
                tenant_id: Some("logger".into()),
                doc_id: Some("logger-doc-semantic".into()),
                received_at: Some(demo_timestamp()),
                original_source: None,
                attributes: None,
            },
            payload: Some(IngestPayload::Text(
                "Structured semantic logging validation".into(),
            )),
        };

        let result = process_pipeline(
            record,
            PipelineStageConfig::Semantic,
            &ingest_cfg,
            &canonical_cfg,
            None,
            Some(&semantic_cfg),
        );
        assert!(result.is_ok());

        let stages: Vec<_> = logger
            .snapshot()
            .into_iter()
            .filter(|event| event.doc_id.as_deref() == Some("logger-doc-semantic"))
            .map(|event| event.stage)
            .collect();
        let expected = [
            PipelineStage::Ingest,
            PipelineStage::Canonical,
            PipelineStage::Semantic,
        ];
        assert_eq!(
            stages, expected,
            "structured semantic events missing or out of order for logger-doc-semantic: {stages:?}"
        );

        set_pipeline_logger(None);
    }
}
