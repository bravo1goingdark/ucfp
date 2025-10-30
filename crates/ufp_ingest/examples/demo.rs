use chrono::Utc;
use ufp_ingest::{CanonicalIngestRecord, IngestMetadata, IngestRequest, IngestSource, ingest};

fn main() {
    let req = IngestRequest {
        source: IngestSource::RawText,
        metadata: Some(IngestMetadata {
            tenant_id: "tenant1".into(),
            doc_id: "doc1".into(),
            received_at: Utc::now(),
            original_source: None,
            attributes: None,
        }),
        payload: Some("  Hello   world\nThis  is\tUC-FP  ".into()),
    };

    let rec: CanonicalIngestRecord = ingest(req).unwrap();
    println!("{:#?}", rec);
}
