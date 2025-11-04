# UCFP Ingest Layer

## Overview

`ufp_ingest` is the front door to the Universal Content Fingerprinting pipeline. It validates
incoming metadata, normalizes payloads, and emits deterministic `CanonicalIngestRecord` values that
downstream stages can rely on.

Key responsibilities:

- enforce required metadata (`tenant_id`, `doc_id`, timestamps, ingest id), defaulting when absent
- ensure payload presence when required (raw text and file sources) and reject empty normalized text
- collapse whitespace for text payloads via `normalize_payload`
- sanitize control characters from metadata to keep logs and downstream systems safe
- decode UTF-8 from byte payloads and surface invalid sequences as `IngestError::InvalidUtf8`
- preserve binary payloads without mutation for later perceptual processing

## Core Data Structures

```rust
pub struct IngestConfig {
    pub version: u32,
    pub default_tenant_id: String,
    pub doc_id_namespace: Uuid,
    pub strip_control_chars: bool,
}

pub enum IngestSource {
    RawText,
    Url(String),
    File { filename: String, content_type: Option<String> },
    Api,
}

pub struct IngestMetadata {
    pub tenant_id: Option<String>,
    pub doc_id: Option<String>,
    pub received_at: Option<DateTime<Utc>>,
    pub original_source: Option<String>,
    pub attributes: Option<serde_json::Value>,
}

pub enum IngestPayload {
    Text(String),
    TextBytes(Vec<u8>),
    Binary(Vec<u8>),
}

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

The ingest id is supplied by callers. Optional metadata is automatically normalized:
tenant ids fall back to the configuration default, document ids are deterministically derived
from the ingest id and tenant when omitted, and missing timestamps are replaced with the current
UTC time. Control characters in metadata are stripped to prevent log or storage injection.

## Public API

```rust
pub fn ingest(record: RawIngestRecord, cfg: &IngestConfig)
    -> Result<CanonicalIngestRecord, IngestError>;
pub fn normalize_payload(text: &str) -> String;
```

`ingest` records structured tracing spans for every attempt, including normalized payload length,
tenant/doc identifiers, and outcome. The function rejects:

- payloads that decode to empty text (`IngestError::EmptyNormalizedText`)
- missing or invalid metadata (`IngestError::InvalidMetadata`)
- malformed UTF-8 when using `IngestPayload::TextBytes` (`IngestError::InvalidUtf8`)

Successful calls emit `CanonicalIngestRecord` values with sanitized metadata and whitespace
normalized text payloads. Table-driven tests in `src/lib.rs` cover defaulting behavior, control
character stripping, deterministic doc id derivation, and binary passthrough.

## Example

```rust
use chrono::{NaiveDate, Utc};
use ufp_ingest::{
    ingest, CanonicalPayload, IngestConfig, IngestMetadata, IngestPayload, IngestSource,
    RawIngestRecord,
};

let cfg = IngestConfig::default();
let date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap();
let timestamp = chrono::DateTime::<Utc>::from_naive_utc_and_offset(date, Utc);

let record = RawIngestRecord {
    id: "ingest-1".into(),
    source: IngestSource::RawText,
    metadata: IngestMetadata {
        tenant_id: Some("tenant".into()),
        doc_id: Some("doc".into()),
        received_at: Some(timestamp),
        original_source: None,
        attributes: None,
    },
    payload: Some(IngestPayload::Text("  Hello   world  ".into())),
};

let canonical = ingest(record, &cfg)?;
assert_eq!(
    canonical.normalized_payload,
    Some(CanonicalPayload::Text("Hello world".into()))
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
