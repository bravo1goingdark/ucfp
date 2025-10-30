# UCFP Ingest Module — Documentation

## Overview
The `ufp_ingest` crate is the **entry point** for ingesting content into the Universal Content Fingerprint (UCFP) pipeline. It standardizes and validates incoming text, files, or URLs into a **canonical ingest record**, ready for downstream canonicalization and fingerprint extraction.

This ensures all input—no matter how it arrives—has consistent metadata, normalized content, and a unique identifier before processing.

---

## Purpose
The ingest layer serves to:
1. Accept text, file, or URL inputs from users or systems.
2. Validate and normalize metadata (tenant ID, document ID, timestamps, etc.).
3. Clean and normalize the payload (collapse whitespace, remove anomalies).
4. Produce a deterministic `CanonicalIngestRecord` structure for use by the canonicalizer.

---

## Key Components

### 1. **Data Types**

#### `IngestSource`
Defines how the content is being provided:
```rust
enum IngestSource {
    RawText,
    Url(String),
    File { filename: String, content_type: Option<String> },
    Api,
}
```

#### `IngestMetadata`
Carries metadata about the input:
```rust
struct IngestMetadata {
    tenant_id: String,
    doc_id: String,
    received_at: DateTime<Utc>,
    original_source: Option<String>,
    attributes: Option<serde_json::Value>,
}
```

#### `IngestRequest`
Represents the raw ingest request from the user or API:
```rust
struct IngestRequest {
    source: IngestSource,
    metadata: Option<IngestMetadata>,
    payload: Option<String>,
}
```

#### `CanonicalIngestRecord`
The normalized output sent downstream:
```rust
struct CanonicalIngestRecord {
    id: String,
    tenant_id: String,
    doc_id: String,
    received_at: DateTime<Utc>,
    original_source: Option<String>,
    source: IngestSource,
    normalized_payload: Option<String>,
    attributes: Option<serde_json::Value>,
}
```

---

### 2. **Core Function: `ingest()`**

```rust
pub fn ingest(req: IngestRequest) -> Result<CanonicalIngestRecord, IngestError>
```

Performs:
- Metadata validation (tenant/doc IDs, timestamps)
- Payload verification (non-empty where required)
- Whitespace normalization
- UUID assignment for traceability

---

## Example Usage

### Input
```rust
use ufp_ingest::{ingest, IngestRequest, IngestSource, IngestMetadata};
use chrono::Utc;

let req = IngestRequest {
    source : IngestSource::RawText,
    metadata: Some(IngestMetadata {
        tenant_id: "tenant1".into(),
        doc_id: "doc1".into(),
        received_at: Utc::now(),
        original_source: None,
        attributes: None,
    }),
    payload: Some("  Hello   world\nThis  is\tUC-FP  ".into()),
};

let record = ingest(req).unwrap();
println!("{:#?}", record);
```

### Output
```rust
CanonicalIngestRecord {
    id: "f0c7b5de-99b2-4a24-94b5-4566ab14b2f7",
    tenant_id: "tenant1",
    doc_id: "doc1",
    received_at: 2025-10-31T18:23:55Z,
    original_source: None,
    source: RawText,
    normalized_payload: Some("Hello world This is UC-FP"),
    attributes: None,
}
```

---

## Error Handling
Errors are typed for safe use in higher layers:
```rust
enum IngestError {
    MissingPayload,        // required payload missing
    InvalidMetadata(String),
    Internal(String),
}
```

---

## Integration in the UCFP Pipeline
```
Raw Input (Text/File/URL)
        │
        ▼
[ ufcp_ingest::ingest() ]  →  CanonicalIngestRecord
        │
        ▼
[ ufp_canonical::canonicalize() ] → tokenized & cleaned text
        │
        ▼
[ ufp_perceptual ] → shingles, minhashes, fingerprints
        │
        ▼
[ ufp_index ] → RocksDB storage + retrieval
```

---

## Testing
Run all built-in tests:
```bash
cargo test -p ufp_ingest
```

Expected output:
```
running 3 tests
ok (all passed)
```

---

## Summary
✅ Converts raw user content into structured canonical input.
✅ Ensures deterministic normalization.
✅ Prepares uniform metadata for indexing & fingerprinting.
✅ Fully testable and extensible for future input types (audio, video, docs, images).

---
**Next step:** connect this to `ufp_canonical` for text canonicalization and fingerprint generation.
