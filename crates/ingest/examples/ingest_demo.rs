use chrono::{DateTime, NaiveDate, Utc};
use ingest::{
    ingest, CanonicalIngestRecord, IngestConfig, IngestMetadata, IngestPayload, IngestSource,
    RawIngestRecord,
};

fn fixed_timestamp() -> DateTime<Utc> {
    let Some(date) = NaiveDate::from_ymd_opt(2024, 1, 1) else {
        panic!("invalid date components");
    };
    let Some(date_time) = date.and_hms_opt(12, 0, 0) else {
        panic!("invalid time components");
    };
    DateTime::<Utc>::from_naive_utc_and_offset(date_time, Utc)
}

fn main() {
    let record = RawIngestRecord {
        id: "ingest-demo".into(),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: Some("tenant1".into()),
            doc_id: Some("doc1".into()),
            received_at: Some(fixed_timestamp()),
            original_source: None,
            attributes: None,
        },
        payload: Some(IngestPayload::Text(
            "  Hello   world\nThis  is\tUC-FP  ".into(),
        )),
    };

    match ingest(record, &IngestConfig::default()) {
        Ok(rec) => {
            let CanonicalIngestRecord { .. } = rec;
            println!("{rec:#?}");
        }
        Err(err) => eprintln!("ingest failed: {err}"),
    }
}
