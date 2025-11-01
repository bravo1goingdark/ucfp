# UCFP Ingest Layer

## Overview

`ufp_ingest` is the front door to the Universal Content Fingerprinting pipeline. It validates
incoming metadata, normalizes payloads, and emits deterministic `CanonicalIngestRecord` values that
downstream stages can rely on.

Key responsibilities:

- enforce required metadata (`tenant_id`, `doc_id`, timestamps, ingest id)
- ensure payload presence when required (raw text and file sources)
- collapse whitespace for text payloads via `normalize_payload`
- preserve binary payloads without mutation for later perceptual processing

## Core Data Structures

```rust
pub enum IngestSource { RawText, Url(String), File { filename: String, content_type: Option<String> }, Api }

pub struct IngestMetadata {
    pub tenant_id: String,
    pub doc_id: String,
    pub received_at: DateTime<Utc>,
    pub original_source: Option<String>,
    pub attributes: Option<serde_json::Value>,
}

pub enum IngestPayload { Text(String), Binary(Vec<u8>) }

pub struct RawIngestRecord {
    pub id: String,
    pub source: IngestSource,
    pub metadata: IngestMetadata,
    pub payload: Option<IngestPayload>,
}

pub struct CanonicalIngestRecord {
    pub id: String,
    pub tenant_id: String,
    pub doc_id: String,
    pub received_at: DateTime<Utc>,
    pub original_source: Option<String>,
    pub source: IngestSource,
    pub normalized_payload: Option<CanonicalPayload>,
    pub attributes: Option<serde_json::Value>,
}
```

The ingest id and metadata are provided by callers; no UUIDs or timestamps are generated internally,
which keeps the pipeline deterministic.

## Public API

```rust
pub fn ingest(record: RawIngestRecord) -> Result<CanonicalIngestRecord, IngestError>;
pub fn normalize_payload(text: &str) -> String;
```

`ingest` returns `IngestError::MissingPayload` when text or file sources are empty and
`IngestError::InvalidMetadata` when required metadata is missing. Table-driven tests in
`src/lib.rs` cover whitespace normalization, metadata preservation, and empty payload rejection.

## Example

```rust
use chrono::{NaiveDate, Utc};
use ufp_ingest::{ingest, IngestMetadata, IngestPayload, IngestSource, RawIngestRecord};

let date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap();
let timestamp = chrono::DateTime::<Utc>::from_naive_utc_and_offset(date, Utc);

let record = RawIngestRecord {
    id: "ingest-1".into(),
    source: IngestSource::RawText,
    metadata: IngestMetadata {
        tenant_id: "tenant".into(),
        doc_id: "doc".into(),
        received_at: timestamp,
        original_source: None,
        attributes: None,
    },
    payload: Some(IngestPayload::Text("  Hello   world  ".into())),
};

let canonical = ingest(record)?;
assert_eq!(
    canonical.normalized_payload,
    Some(ufp_ingest::CanonicalPayload::Text("Hello world".into()))
);
```

### Examples

Run the binaries shipped with the crate for hands-on usage:

- `cargo run --package ufp_ingest --example ingest_demo` - ingests a single text payload with deterministic metadata.
- `cargo run --package ufp_ingest --example batch_ingest` - processes a mix of raw-text, URL, and binary fixtures to illustrate whitespace collapsing and binary passthrough.

## Testing

Run unit tests with:

```bash
cargo test -p ufp_ingest
```

## Downstream Integration

`CanonicalIngestRecord` instances feed directly into `ufp_canonical::process_record`, keeping the
full ingest pipeline deterministic and side-effect free.
