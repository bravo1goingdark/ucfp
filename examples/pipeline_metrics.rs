use std::fmt::Display;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use ucfp::{
    process_record_with_perceptual,
    set_pipeline_metrics,
    CanonicalizeConfig,
    IngestError,
    IngestMetadata,
    IngestPayload,
    IngestSource,
    PerceptualConfig,
    PerceptualError,
    PipelineError,
    PipelineMetrics,
    RawIngestRecord,
};

fn main() -> Result<(), PipelineError> {
    let metrics = Arc::new(RecordingMetrics::new());
    set_pipeline_metrics(Some(metrics.clone()));

    let canonical_cfg = CanonicalizeConfig::default();
    let perceptual_cfg = PerceptualConfig {
        k: 3,
        ..PerceptualConfig::default()
    };

    let (_doc, _fingerprint) = process_record_with_perceptual(
        build_demo_record(),
        &canonical_cfg,
        &perceptual_cfg,
    )?;

    println!("Recorded metrics events:");
    for event in metrics.snapshot() {
        println!(" - {event}");
    }

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
}

fn format_stage<E: Display>(stage: &str, latency: Duration, result: Result<(), E>) -> String {
    let latency_us = latency.as_micros();
    match result {
        Ok(()) => format!("{stage}: ok ({latency_us} us)"),
        Err(err) => format!("{stage}: err ({latency_us} us) - {err}"),
    }
}
