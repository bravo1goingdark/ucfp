# UCFP Ingest Layer (`ufp_ingest`)

## Overview

`ufp_ingest` is the front door to the Universal Content Fingerprinting (UCFP) pipeline. It validates
incoming metadata, normalizes payloads, and emits deterministic `CanonicalIngestRecord` values that
downstream stages can rely on.

This crate is the **first stage** in the UCFP linear pipeline:
```
ufp_ingest → ufp_canonical → ufp_perceptual/semantic → ufp_index → ufp_match
```

## Key Responsibilities

1. **Metadata Validation & Normalization**
   - Enforce required metadata fields (`tenant_id`, `doc_id`, timestamps, ingest id)
   - Apply sensible defaults when fields are missing
   - Validate metadata policies (e.g., reject future timestamps)

2. **Payload Processing**
   - Ensure payload presence when required (raw text and file sources)
   - Reject empty normalized text
   - Collapse whitespace for text payloads via `normalize_payload`
   - Sanitize control characters from metadata
   - Decode UTF-8 from byte payloads with proper error handling
   - Preserve binary payloads without mutation for later perceptual processing

3. **Deterministic Document ID Generation**
   - Derive stable document IDs using UUIDv5 when not provided
   - Ensure multi-tenant isolation through tenant-aware ID generation

4. **Size & Policy Enforcement**
   - Enforce maximum payload size limits
   - Enforce maximum normalized text size
   - Enforce attribute size limits
   - Support configurable metadata policies

## Core Data Structures

### IngestConfig

```rust
pub struct IngestConfig {
    pub version: u32,
    pub default_tenant_id: String,
    pub doc_id_namespace: Uuid,
    pub strip_control_chars: bool,
    pub metadata_policy: MetadataPolicy,
    pub max_payload_bytes: Option<usize>,
    pub max_normalized_bytes: Option<usize>,
}
```

**Configuration Fields:**

- **`version`** - Semantic version of the ingest behavior (default: 1). This is logged with each record so you can correlate behavior with configuration rollouts. Any breaking change to ingest behavior should bump this version.

- **`default_tenant_id`** - Fallback tenant identifier used when `IngestMetadata::tenant_id` is absent or empty. This ensures every canonical record is associated with some tenant, even when callers omit it. Default: `"default"`.

- **`doc_id_namespace`** - Namespace UUID used when deterministically deriving document IDs via UUIDv5. Given a namespace, tenant, and ingest id, the generated `doc_id` is stable across runs. Changing this namespace will change all derived document IDs.

- **`strip_control_chars`** - When `true`, ASCII control characters are stripped from metadata strings (tenant id, doc id, original source) before further processing and logging. This prevents log injection attacks and keeps downstream systems safe. Default: `true`.

- **`metadata_policy`** - A `MetadataPolicy` instance that describes which metadata fields are required and how large the `attributes` JSON blob is allowed to be.

- **`max_payload_bytes`** - Optional limit on the size of the *raw* payload, in bytes, before any normalization. If present and exceeded, ingest fails fast with `IngestError::PayloadTooLarge`.

- **`max_normalized_bytes`** - Optional limit on the size of the normalized text payload. This is enforced *after* whitespace collapsing and is independent of the raw size limit.

### MetadataPolicy

```rust
pub struct MetadataPolicy {
    pub required_fields: Vec<RequiredField>,
    pub max_attribute_bytes: Option<usize>,
    pub reject_future_timestamps: bool,
}
```

**Policy Fields:**

- **`required_fields`** - A list of `RequiredField` values that must be present *after* sanitization and defaulting. For example, adding `TenantId` here forces callers to explicitly provide a non-empty tenant id instead of relying on `default_tenant_id`.

- **`max_attribute_bytes`** - Optional upper bound, in bytes, on the serialized size of `IngestMetadata::attributes`. This protects downstream systems from very large or abusive metadata blobs.

- **`reject_future_timestamps`** - When `true`, ingests whose `received_at` is strictly in the future (relative to `Utc::now()`) are rejected with `IngestError::InvalidMetadata`.

### RequiredField

```rust
#[non_exhaustive]
pub enum RequiredField {
    TenantId,
    DocId,
    ReceivedAt,
    OriginalSource,
}
```

`RequiredField` is marked `#[non_exhaustive]` so additional required-field kinds can be added in the future without breaking downstream code. Callers should always include a catch-all arm when matching it.

### IngestSource

```rust
#[non_exhaustive]
pub enum IngestSource {
    RawText,
    Url(String),
    File { filename: String, content_type: Option<String> },
    Api,
}
```

**Source Variants:**

- **`RawText`** - Plain text supplied directly in the request body. This source requires a text payload.

- **`Url(String)`** - Content logically associated with a URL (e.g., text crawled from a web page). This source requires a text payload.

- **`File { filename, content_type }`** - An uploaded file, such as an image, PDF, or document. Binary payloads are typically paired with this variant.

- **`Api`** - Catch-all for ingests that originate from an API call without a more specific source.

The enum is `#[non_exhaustive]`, so future source kinds can be added without breaking callers.

### IngestMetadata

```rust
pub struct IngestMetadata {
    pub tenant_id: Option<String>,
    pub doc_id: Option<String>,
    pub received_at: Option<DateTime<Utc>>,
    pub original_source: Option<String>,
    pub attributes: Option<serde_json::Value>,
}
```

**Metadata Fields:**

- **`tenant_id`** - Optional tenant; when missing or empty, it is sanitized and may fall back to `IngestConfig::default_tenant_id` unless `MetadataPolicy` marks it as required.

- **`doc_id`** - Optional external document identifier; if absent, a deterministic UUIDv5 is derived from the tenant and ingest id using `doc_id_namespace`.

- **`received_at`** - Optional timestamp associated with when the content was received or observed. When omitted and not required by policy, it defaults to the current UTC time.

- **`original_source`** - Optional human-meaningful identifier such as an upstream URL, external id, or path. Control characters are stripped so this string is safe to log and store.

- **`attributes`** - Optional arbitrary JSON, e.g., extra tags, labels, or contextual information. This field is size-limited by `MetadataPolicy::max_attribute_bytes`.

### IngestPayload

```rust
#[non_exhaustive]
pub enum IngestPayload {
    Text(String),
    TextBytes(Vec<u8>),
    Binary(Vec<u8>),
}
```

**Payload Variants:**

- **`Text(String)`** - UTF-8 text that will be whitespace-normalized via `normalize_payload`.

- **`TextBytes(Vec<u8>)`** - Raw bytes expected to contain UTF-8 text; invalid UTF-8 results in `IngestError::InvalidUtf8`.

- **`Binary(Vec<u8>)`** - Arbitrary binary blob (images, audio, documents). Binary data is passed through unmodified except for an emptiness check.

### RawIngestRecord

```rust
pub struct RawIngestRecord {
    pub id: String,
    pub source: IngestSource,
    pub metadata: IngestMetadata,
    pub payload: Option<IngestPayload>,
}
```

**Record Fields:**

- **`id`** - Ingest id supplied by callers. This is a stable identifier used for tracing, log correlation, and deterministic document-id derivation.

- **`source`** - An `IngestSource` describing where the payload came from.

- **`metadata`** - An `IngestMetadata` instance containing caller-supplied metadata.

- **`payload`** - Optional `IngestPayload`. Some sources require this to be present, while others may legitimately carry no payload (for metadata-only events).

### CanonicalIngestRecord

```rust
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

**Canonical Record Fields:**

- **`id`** - The sanitized ingest id (after control-character stripping and trimming).

- **`tenant_id`** - Effective tenant identifier after applying defaults and policy checks.

- **`doc_id`** - Either the sanitized caller-supplied document id or a deterministic UUIDv5 derived from the tenant and ingest id.

- **`received_at`** - Resolved timestamp, defaulting to the current time when not required and not supplied.

- **`original_source`** - Sanitized version of `IngestMetadata::original_source`.

- **`source`** - The original `IngestSource` variant from the raw record.

- **`normalized_payload`** - Optional `CanonicalPayload`; either normalized text or preserved binary bytes.

- **`attributes`** - The final `attributes` JSON blob after size checks.

### CanonicalPayload

```rust
#[non_exhaustive]
pub enum CanonicalPayload {
    Text(String),
    Binary(Vec<u8>),
}
```

**Payload Variants:**

- **`Text(String)`** - Text after whitespace collapsing and size-limit checks.

- **`Binary(Vec<u8>)`** - Non-empty binary payload preserved exactly as provided by the caller.

The enum is `#[non_exhaustive]` so future payload representations (e.g., structured text segments) can be introduced without a breaking change.

## Public API

### Main Functions

```rust
/// Ingest a raw record and produce a canonical record.
pub fn ingest(
    record: RawIngestRecord, 
    cfg: &IngestConfig
) -> Result<CanonicalIngestRecord, IngestError>;

/// Ingest with an observer for custom instrumentation.
pub fn ingest_with_observer<O>(
    record: RawIngestRecord,
    cfg: &IngestConfig,
    observer: &O,
) -> Result<CanonicalIngestRecord, IngestError>
where
    O: IngestObserver + ?Sized;

/// Normalize whitespace in text (collapse multiple spaces, trim ends).
pub fn normalize_payload(text: &str) -> String;
```

### Observer Trait

```rust
pub trait IngestObserver {
    fn on_success(&self, record: &CanonicalIngestRecord, elapsed_micros: u128);
    fn on_failure(&self, error: &IngestError, elapsed_micros: u128);
}
```

The observer trait allows you to hook into ingest events for metrics, logging, or custom instrumentation without modifying the core ingest logic.

### Config Validation

```rust
impl IngestConfig {
    pub fn validate(&self) -> Result<(), ConfigError>;
}
```

Validation checks:
- Size limits are consistent (normalized limit shouldn't exceed raw limit)
- Required fields list doesn't contain duplicates
- Default values are reasonable

## Error Handling

`ingest` records structured tracing spans for every attempt, including normalized payload length, tenant/doc identifiers, and outcome. The function rejects:

### IngestError Variants

- **`EmptyNormalizedText`** - Payloads that decode to empty text after whitespace collapsing. This prevents processing of whitespace-only inputs.

- **`InvalidMetadata`** - Missing or invalid metadata, such as absent required fields, attributes that exceed `max_attribute_bytes`, or timestamps that violate policy (e.g., future timestamps when `reject_future_timestamps` is true).

- **`InvalidUtf8`** - Malformed UTF-8 when using `IngestPayload::TextBytes`. Triggered when the incoming byte sequence cannot be decoded as valid UTF-8.

- **`MissingPayload`** - Missing payloads for sources that require one (e.g., `RawText` or `File` sources without any associated payload).

- **`EmptyBinaryPayload`** - Zero-length binary payloads, which are rejected to avoid meaningless ingests.

- **`PayloadTooLarge`** - Payloads that violate configured size limits on raw or normalized text. Covers both `max_payload_bytes` and `max_normalized_bytes` when set.

- **`DocIdDerivationFailed`** - When document ID derivation fails (e.g., missing tenant for UUIDv5 generation).

## Examples

### Basic Text Ingest

```rust
use chrono::{NaiveDate, Utc};
use ufp_ingest::{
    ingest, CanonicalPayload, IngestConfig, IngestMetadata, IngestPayload, 
    IngestSource, RawIngestRecord,
};

let cfg = IngestConfig::default();
let date = NaiveDate::from_ymd_opt(2024, 1, 1)
    .unwrap()
    .and_hms_opt(0, 0, 0)
    .unwrap();
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

### With Custom Configuration

```rust
use ufp_ingest::{
    IngestConfig, MetadataPolicy, RequiredField,
};
use uuid::Uuid;

let cfg = IngestConfig {
    version: 2,
    default_tenant_id: "my-app".into(),
    doc_id_namespace: Uuid::new_v5(&Uuid::NAMESPACE_DNS, b"my-app.example.com"),
    strip_control_chars: true,
    metadata_policy: MetadataPolicy {
        required_fields: vec![RequiredField::TenantId, RequiredField::DocId],
        max_attribute_bytes: Some(1024 * 1024), // 1MB
        reject_future_timestamps: true,
    },
    max_payload_bytes: Some(10 * 1024 * 1024), // 10MB
    max_normalized_bytes: Some(5 * 1024 * 1024), // 5MB
};
```

### With Observer for Metrics

```rust
use ufp_ingest::{ingest_with_observer, IngestObserver, CanonicalIngestRecord, IngestError};
use std::sync::atomic::{AtomicU64, Ordering};

struct MetricsObserver {
    success_count: AtomicU64,
    failure_count: AtomicU64,
    total_micros: AtomicU64,
}

impl IngestObserver for MetricsObserver {
    fn on_success(&self, _record: &CanonicalIngestRecord, elapsed_micros: u128) {
        self.success_count.fetch_add(1, Ordering::Relaxed);
        self.total_micros.fetch_add(elapsed_micros as u64, Ordering::Relaxed);
    }
    
    fn on_failure(&self, _error: &IngestError, elapsed_micros: u128) {
        self.failure_count.fetch_add(1, Ordering::Relaxed);
        self.total_micros.fetch_add(elapsed_micros as u64, Ordering::Relaxed);
    }
}

let observer = MetricsObserver {
    success_count: AtomicU64::new(0),
    failure_count: AtomicU64::new(0),
    total_micros: AtomicU64::new(0),
};

let result = ingest_with_observer(record, &cfg, &observer)?;
```

### Binary File Ingest

```rust
use ufp_ingest::{IngestPayload, IngestSource};

let record = RawIngestRecord {
    id: "ingest-file-1".into(),
    source: IngestSource::File {
        filename: "document.pdf".into(),
        content_type: Some("application/pdf".into()),
    },
    metadata: IngestMetadata {
        tenant_id: Some("tenant-a".into()),
        doc_id: Some("doc-123".into()),
        received_at: Some(Utc::now()),
        original_source: Some("uploads/document.pdf".into()),
        attributes: Some(json!({"file_size": 1024567})),
    },
    payload: Some(IngestPayload::Binary(pdf_bytes)),
};
```

## Best Practices

### 1. Always Set Document IDs Explicitly

While `ufp_ingest` can derive document IDs deterministically, it's better to set them explicitly for:
- Better traceability
- Easier debugging
- Consistency with external systems

```rust
let record = RawIngestRecord {
    id: format!("ingest-{}", uuid::Uuid::new_v4()),
    // ...
    metadata: IngestMetadata {
        doc_id: Some(external_doc_id), // Always provide this
        // ...
    },
};
```

### 2. Use Size Limits in Production

Always set `max_payload_bytes` and `max_normalized_bytes` to prevent abuse:

```rust
let cfg = IngestConfig {
    max_payload_bytes: Some(50 * 1024 * 1024),    // 50MB raw
    max_normalized_bytes: Some(10 * 1024 * 1024), // 10MB normalized
    // ...
};
```

### 3. Strip Control Characters

Keep `strip_control_chars: true` (the default) to prevent:
- Log injection attacks
- Terminal escape sequences in metadata
- Issues with downstream text processing

### 4. Validate Configuration at Startup

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = load_ingest_config()?;
    cfg.validate()?; // Fail fast on bad config
    // ...
}
```

### 5. Use Namespaced Document IDs

When deriving document IDs, use a consistent namespace:

```rust
use uuid::Uuid;

// Create a namespace unique to your application
const DOC_ID_NAMESPACE: Uuid = Uuid::from_u128(0x1234567890abcdef1234567890abcdef);

let cfg = IngestConfig {
    doc_id_namespace: DOC_ID_NAMESPACE,
    // ...
};
```

## Testing

### Running Tests

```bash
# Run all unit tests
cargo test -p ufp_ingest

# Run with output
cargo test -p ufp_ingest -- --nocapture
```

### Test Coverage

Unit tests in `src/lib.rs` cover:
- Defaulting behavior (missing tenant, doc_id, timestamps)
- Control character stripping
- Deterministic doc id derivation via UUIDv5
- Binary passthrough
- Size limit enforcement
- UTF-8 validation
- Future timestamp rejection

### Examples

Run the binaries shipped with the crate for hands-on usage:

```bash
# Ingest a single text payload with deterministic metadata
cargo run --package ufp_ingest --example ingest_demo

# Process a mix of raw-text, URL, and binary fixtures
cargo run --package ufp_ingest --example batch_ingest

# Demonstrate size limit enforcement
cargo run --package ufp_ingest --example size_limit_demo
```

## Troubleshooting

### "EmptyNormalizedText" Error

This means your text became empty after whitespace normalization. Common causes:
- Input was whitespace-only
- Input contained only control characters (which were stripped)

**Fix:** Check input before ingest or catch the error and provide a meaningful message.

### "InvalidUtf8" Error

The `TextBytes` payload couldn't be decoded as UTF-8.

**Fix:** 
- Use `IngestPayload::Binary` for non-text data
- Validate encoding before ingest
- Use encoding detection libraries for unknown sources

### "PayloadTooLarge" Error

The payload exceeded `max_payload_bytes` or `max_normalized_bytes`.

**Fix:**
- Increase limits if appropriate
- Reject at API layer before calling ingest
- Implement chunked processing for large documents

### Document ID Collisions

If you're seeing unexpected document ID collisions:

**Fix:**
- Ensure unique `ingest_id` values
- Use a unique `doc_id_namespace` per application
- Set explicit `doc_id` in metadata

## Integration with Downstream Stages

`CanonicalIngestRecord` instances feed directly into `ufp_canonical::canonicalize`:

```rust
use ufp_ingest::{ingest, CanonicalPayload};
use ufp_canonical::canonicalize;

// 1. Ingest
let canonical_record = ingest(raw_record, &ingest_cfg)?;

// 2. Extract text for canonicalization
if let Some(CanonicalPayload::Text(text)) = canonical_record.normalized_payload {
    // 3. Canonicalize
    let doc = canonicalize(&canonical_record.doc_id, &text, &canonical_cfg)?;
    // Continue to perceptual/semantic stages...
}
```

This keeps the full ingest pipeline deterministic and side-effect free.

## Performance Considerations

- **Whitespace normalization** is O(n) and allocates a new String
- **Control character stripping** is O(n) and allocates a new String
- **UTF-8 validation** is O(n)
- **Document ID derivation** (UUIDv5) is fast and deterministic

For high-throughput scenarios:
- Use `ingest_with_observer` to track performance
- Consider batch processing
- Set appropriate size limits to prevent abuse
- Reuse `IngestConfig` instances

## Security Considerations

1. **Control Character Stripping** - Always enabled to prevent log injection
2. **Size Limits** - Set appropriate limits to prevent DoS
3. **Future Timestamp Rejection** - Enable to prevent timestamp manipulation
4. **Input Validation** - All inputs are validated before processing
5. **No Execution** - `ufp_ingest` never executes code from payloads

## Version History

- **v1** - Initial release with basic ingest functionality
- **v2** - Added metadata policies and size limits

## License

Licensed under the Apache License, Version 2.0.
