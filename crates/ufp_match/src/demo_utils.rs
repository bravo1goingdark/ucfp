use chrono::{NaiveDate, Utc};
use ufp_ingest::{IngestMetadata, IngestPayload, IngestSource, RawIngestRecord};
use ufp_semantic::SemanticEmbedding;

/// Fixed wall-clock timestamp so demos and tests are deterministic.
pub fn demo_timestamp() -> chrono::DateTime<Utc> {
    let Some(date) = NaiveDate::from_ymd_opt(2025, 1, 1) else {
        panic!("invalid demo date components");
    };
    let Some(date_time) = date.and_hms_opt(0, 0, 0) else {
        panic!("invalid demo time components");
    };
    chrono::DateTime::<Utc>::from_naive_utc_and_offset(date_time, Utc)
}

/// Build a basic RawIngestRecord for a given tenant and document id.
pub fn base_record_with_tenant(tenant: &str, doc_id: &str, text: &str) -> RawIngestRecord {
    RawIngestRecord {
        id: format!("ingest-{doc_id}"),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: Some(tenant.to_string()),
            doc_id: Some(doc_id.to_string()),
            received_at: Some(demo_timestamp()),
            original_source: None,
            attributes: None,
        },
        payload: Some(IngestPayload::Text(text.to_string())),
    }
}

/// Build a demo RawIngestRecord for examples with a fixed tenant id and source.
pub fn demo_base_record(doc_id: &str, text: &str, original: &str) -> RawIngestRecord {
    RawIngestRecord {
        id: format!("demo-{doc_id}"),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: Some("tenant-a".into()),
            doc_id: Some(doc_id.into()),
            received_at: Some(demo_timestamp()),
            original_source: Some(original.into()),
            attributes: None,
        },
        payload: Some(IngestPayload::Text(text.into())),
    }
}

/// Quantize a semantic embedding using a caller-provided scale.
pub fn quantize_with_scale(embedding: &SemanticEmbedding, scale: f32) -> Vec<i8> {
    embedding
        .vector
        .iter()
        .map(|v| (v * scale).clamp(-128.0, 127.0) as i8)
        .collect()
}
