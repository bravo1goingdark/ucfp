use ingest::{ingest, IngestConfig, IngestMetadata, IngestPayload, IngestSource, RawIngestRecord};

fn main() {
    println!("--- Demonstrating Payload Size Limit Policies ---");

    let cfg = IngestConfig {
        max_payload_bytes: Some(32),
        max_normalized_bytes: Some(20),
        ..Default::default()
    };

    // --- Case 1: Payload within all limits ---
    println!("\n1. Ingesting a payload that is within all size limits...");
    let record1 = RawIngestRecord {
        id: "demo-1-success".into(),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: None,
            doc_id: None,
            received_at: None,
            original_source: None,
            attributes: None,
        },
        payload: Some(IngestPayload::Text("  valid payload data  ".into())), // Raw: 22 bytes, Norm: 18
    };

    match ingest(record1, &cfg) {
        Ok(rec) => println!(
            " -> Success! Normalized payload: {:?}",
            rec.normalized_payload.unwrap()
        ),
        Err(err) => eprintln!(" -> Unexpected Error: {err}"),
    }

    // --- Case 2: Raw payload exceeds max_payload_bytes ---
    println!("\n2. Ingesting a payload that exceeds the raw size limit...");
    let record2 = RawIngestRecord {
        id: "demo-2-raw-limit".into(),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: None,
            doc_id: None,
            received_at: None,
            original_source: None,
            attributes: None,
        },
        payload: Some(IngestPayload::Text(
            "this raw payload is definitely way too long".into(),
        )), // 43 bytes
    };

    match ingest(record2, &cfg) {
        Ok(_) => eprintln!(" -> Unexpected Success!"),
        Err(err) => println!(" -> Success! Caught expected error: {err}"),
    }

    // --- Case 3: Normalized payload exceeds max_normalized_bytes ---
    println!("\n3. Ingesting a payload that exceeds the normalized size limit...");
    let record3 = RawIngestRecord {
        id: "demo-3-norm-limit".into(),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: None,
            doc_id: None,
            received_at: None,
            original_source: None,
            attributes: None,
        },
        payload: Some(IngestPayload::Text("short raw, but long normalized".into())), // Raw: 30 bytes, Norm: 29
    };

    match ingest(record3, &cfg) {
        Ok(_) => eprintln!(" -> Unexpected Success!"),
        Err(err) => println!(" -> Success! Caught expected error: {err}"),
    }
}
