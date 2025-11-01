use chrono::{DateTime, NaiveDate, Utc};
use ufp_ingest::{
    CanonicalIngestRecord, CanonicalPayload, IngestMetadata, IngestPayload, IngestSource,
    RawIngestRecord, ingest,
};

fn timestamp(hour: u32) -> DateTime<Utc> {
    let Some(date) = NaiveDate::from_ymd_opt(2024, 1, 1) else {
        panic!("invalid date components");
    };
    let Some(dt) = date.and_hms_opt(hour, 0, 0) else {
        panic!("invalid time components");
    };
    DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)
}

fn main() {
    let fixtures: [RawIngestRecord; 3] = [
        RawIngestRecord {
            id: "text-001".into(),
            source: IngestSource::RawText,
            metadata: IngestMetadata {
                tenant_id: "tenant-a".into(),
                doc_id: "doc-1".into(),
                received_at: timestamp(0),
                original_source: Some("https://example.com/a".into()),
                attributes: None,
            },
            payload: Some(IngestPayload::Text("  First   text\npayload ".into())),
        },
        RawIngestRecord {
            id: "text-002".into(),
            source: IngestSource::Url("https://example.com/b".into()),
            metadata: IngestMetadata {
                tenant_id: "tenant-a".into(),
                doc_id: "doc-2".into(),
                received_at: timestamp(1),
                original_source: None,
                attributes: Some(serde_json::json!({"topic": "news"})),
            },
            payload: Some(IngestPayload::Text("Second payload\twith spacing".into())),
        },
        RawIngestRecord {
            id: "bin-001".into(),
            source: IngestSource::File {
                filename: "image.png".into(),
                content_type: Some("image/png".into()),
            },
            metadata: IngestMetadata {
                tenant_id: "tenant-b".into(),
                doc_id: "image-1".into(),
                received_at: timestamp(2),
                original_source: None,
                attributes: Some(serde_json::json!({"kind": "thumbnail"})),
            },
            payload: Some(IngestPayload::Binary(vec![0, 1, 2, 3])),
        },
    ];

    for record in fixtures {
        match ingest(record) {
            Ok(CanonicalIngestRecord {
                normalized_payload, ..
            }) => match normalized_payload {
                Some(CanonicalPayload::Text(text)) => {
                    println!("text payload -> \"{text}\"");
                }
                Some(CanonicalPayload::Binary(bytes)) => {
                    println!("binary payload -> {} bytes", bytes.len());
                }
                None => println!("no payload provided"),
            },
            Err(err) => eprintln!("ingest failed: {err}"),
        }
    }
}
