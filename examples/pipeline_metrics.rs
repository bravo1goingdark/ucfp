use std::fmt::Display;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use ucfp::{
    process_pipeline, set_pipeline_logger, set_pipeline_metrics, CanonicalizeConfig, IndexError,
    IngestConfig, IngestError, IngestMetadata, IngestPayload, IngestSource, KeyValueLogger,
    MatchError, PerceptualConfig, PerceptualError, PipelineError, PipelineEventLogger,
    PipelineMetrics, PipelineStageConfig, RawIngestRecord, SemanticConfig, SemanticError,
};

fn main() -> Result<(), PipelineError> {
    let metrics = Arc::new(RecordingMetrics::new());
    set_pipeline_metrics(Some(metrics.clone()));
    let logger: Arc<dyn PipelineEventLogger> = Arc::new(KeyValueLogger::stdout());
    set_pipeline_logger(Some(logger));

    let canonical_cfg = CanonicalizeConfig::default();
    let ingest_cfg = IngestConfig::default();
    let perceptual_cfg = PerceptualConfig {
        k: 3,
        ..PerceptualConfig::default()
    };

    let (_doc, _fingerprint, _) = process_pipeline(
        build_demo_record(),
        PipelineStageConfig::Perceptual,
        &ingest_cfg,
        &canonical_cfg,
        Some(&perceptual_cfg),
        None,
    )?;

    let semantic_cfg = SemanticConfig {
        mode: "fast".into(),
        tier: "fast".into(),
        ..Default::default()
    };
    let (_, _, embedding) = process_pipeline(
        build_demo_record(),
        PipelineStageConfig::Semantic,
        &ingest_cfg,
        &canonical_cfg,
        Some(&perceptual_cfg),
        Some(&semantic_cfg),
    )?;
    let _embedding = embedding.unwrap();

    println!("Recorded metrics events:");
    for event in metrics.snapshot() {
        println!(" - {event}");
    }

    set_pipeline_logger(None);
    set_pipeline_metrics(None);
    Ok(())
}

fn build_demo_record() -> RawIngestRecord {
    RawIngestRecord {
        id: "metrics-demo".into(),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: Some("metrics-tenant".into()),
            doc_id: Some("metrics-doc".into()),
            received_at: None,
            original_source: Some("examples/pipeline_metrics.rs".into()),
            attributes: None,
        },
        payload: Some(IngestPayload::Text(
            "The quick brown fox jumps over the lazy dog".into(),
        )),
    }
}

#[derive(Clone, Default)]
struct RecordingMetrics {
    events: Arc<RwLock<Vec<String>>>,
}

impl RecordingMetrics {
    fn new() -> Self {
        Self::default()
    }

    fn snapshot(&self) -> Vec<String> {
        self.events.read().unwrap().clone()
    }

    fn push(&self, entry: String) {
        self.events.write().unwrap().push(entry);
    }
}

impl PipelineMetrics for RecordingMetrics {
    fn record_ingest(&self, latency: Duration, result: Result<(), IngestError>) {
        self.push(format_stage("ingest", latency, result));
    }

    fn record_canonical(&self, latency: Duration, result: Result<(), PipelineError>) {
        self.push(format_stage("canonical", latency, result));
    }

    fn record_perceptual(&self, latency: Duration, result: Result<(), PerceptualError>) {
        self.push(format_stage("perceptual", latency, result));
    }

    fn record_semantic(&self, latency: Duration, result: Result<(), SemanticError>) {
        let result = result.map_err(|err| err.to_string());
        self.push(format_stage("semantic", latency, result));
    }

    fn record_index(&self, latency: Duration, result: Result<(), IndexError>) {
        self.push(format_stage("index", latency, result));
    }

    fn record_match(&self, latency: Duration, result: Result<(), MatchError>) {
        self.push(format_stage("match", latency, result));
    }
}

fn format_stage<E: Display>(stage: &str, latency: Duration, result: Result<(), E>) -> String {
    let latency_us = latency.as_micros();
    match result {
        Ok(()) => format!("{stage}: ok ({latency_us} us)"),
        Err(err) => format!("{stage}: err ({latency_us} us) - {err}"),
    }
}
