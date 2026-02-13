# UCFP Ingest Crate

> **Content ingestion, validation, and normalization for the Universal Content Fingerprinting pipeline**

[![API Docs](https://img.shields.io/badge/docs-api-blue)](https://docs.rs/ingest)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)

## Table of Contents

- [Overview](#overview)
- [Quick Start](#quick-start)
- [Architecture](#architecture)
- [Core Concepts](#core-concepts)
- [Configuration](#configuration)
- [API Reference](#api-reference)
- [Error Handling](#error-handling)
- [Examples](#examples)
- [Best Practices](#best-practices)
- [Performance](#performance)
- [Troubleshooting](#troubleshooting)

---

## Overview

The `ingest` crate is the **entry point** to the Universal Content Fingerprinting (UCFP) pipeline. It transforms raw, potentially malformed input into clean, deterministic `CanonicalIngestRecord` instances that downstream stages can process reliably.

### What This Crate Does

| Function | Description |
|----------|-------------|
| **Validation** | Enforce metadata policies, size limits, and business rules |
| **Normalization** | Collapse whitespace, strip control characters, sanitize inputs |
| **ID Generation** | Derive stable document IDs using UUIDv5 when not provided |
| **Multi-modal Support** | Handle text, binary, and structured payloads |
| **Observability** | Structured logging via `tracing` for production debugging |

### Pipeline Position

```
┌─────────┐     ┌──────────┐     ┌──────────────────┐     ┌───────┐     ┌───────┐
│  Ingest │────▶│Canonical │────▶│Perceptual/Semantic│────▶│ Index │────▶│ Match │
│  (this) │     │          │     │                  │     │       │     │       │
└─────────┘     └──────────┘     └──────────────────┘     └───────┘     └───────┘
```

---

## Quick Start

### Basic Text Ingestion

```rust
use ingest::{
    ingest, IngestConfig, RawIngestRecord, 
    IngestSource, IngestMetadata, IngestPayload
};
use chrono::Utc;

// 1. Configure (use defaults for quick start)
let config = IngestConfig::default();

// 2. Create a raw record
let record = RawIngestRecord {
    id: "doc-001".to_string(),
    source: IngestSource::RawText,
    metadata: IngestMetadata {
        tenant_id: Some("acme-corp".to_string()),
        doc_id: Some("report-q4-2024".to_string()),
        received_at: Some(Utc::now()),
        original_source: None,
        attributes: None,
    },
    payload: Some(IngestPayload::Text(
        "  Quarterly report: revenue up 15% YoY.   ".to_string()
    )),
};

// 3. Ingest and get canonical record
let canonical = ingest(record, &config)?;

// 4. Use the result
assert_eq!(canonical.tenant_id, "acme-corp");
assert_eq!(canonical.doc_id, "report-q4-2024");
// Whitespace normalized: "Quarterly report: revenue up 15% YoY."
```

### Production Configuration

```rust
use ingest::{IngestConfig, MetadataPolicy, RequiredField};
use uuid::Uuid;

let config = IngestConfig {
    version: 1,
    default_tenant_id: "default".to_string(),
    doc_id_namespace: Uuid::new_v5(&Uuid::NAMESPACE_DNS, b"myapp.example.com"),
    strip_control_chars: true,
    metadata_policy: MetadataPolicy {
        required_fields: vec![
            RequiredField::TenantId,
            RequiredField::DocId,
        ],
        max_attribute_bytes: Some(1024 * 1024), // 1 MB
        reject_future_timestamps: true,
    },
    max_payload_bytes: Some(50 * 1024 * 1024),      // 50 MB raw
    max_normalized_bytes: Some(10 * 1024 * 1024),   // 10 MB normalized
};

// Validate at startup
config.validate()?;
```

---

## Architecture

### Data Flow

```
RawIngestRecord
      │
      ▼
┌─────────────────────────────────────────┐
│           Ingest Pipeline               │
├─────────────────────────────────────────┤
│  1. Validate Payload Requirements       │
│     - Check source mandates payload     │
│     - Enforce raw size limits           │
├─────────────────────────────────────────┤
│  2. Normalize Metadata                  │
│     - Apply defaults (tenant, doc_id)   │
│     - Validate timestamps               │
│     - Enforce required fields           │
│     - Strip control characters          │
├─────────────────────────────────────────┤
│  3. Normalize Payload                   │
│     - Decode UTF-8 (TextBytes)          │
│     - Collapse whitespace (Text)        │
│     - Preserve binary (Binary)          │
│     - Enforce normalized size limits    │
├─────────────────────────────────────────┤
│  4. Construct Canonical Record          │
│     - All fields guaranteed present     │
│     - Deterministic output              │
└─────────────────────────────────────────┘
      │
      ▼
CanonicalIngestRecord
```

### Key Design Principles

1. **Fail Fast**: Validation happens before any transformation
2. **Deterministic**: Same input always produces same output (critical for fingerprinting)
3. **Observable**: Every operation is logged with structured tracing
4. **Safe**: Control characters stripped, sizes bounded, UTF-8 validated

---

## Core Concepts

### Ingest Sources

The `IngestSource` enum defines where content originates:

```rust
pub enum IngestSource {
    /// Plain text from request body (requires text payload)
    RawText,
    /// Content from a URL (requires text payload)
    Url(String),
    /// Uploaded file with metadata
    File { 
        filename: String, 
        content_type: Option<String> 
    },
    /// Generic API call (payload optional)
    Api,
}
```

### Payload Types

Three variants support multi-modal content:

| Variant | Use Case | Processing |
|---------|----------|------------|
| `Text(String)` | Clean UTF-8 text | Whitespace normalization |
| `TextBytes(Vec<u8>)` | Raw UTF-8 bytes | UTF-8 validation + normalization |
| `Binary(Vec<u8>)` | Images, PDFs, audio | Passthrough (size-checked) |

### Document ID Generation

When `doc_id` is not provided, a deterministic UUIDv5 is derived:

```
doc_id = UUIDv5(namespace, tenant_id + "\0" + record_id)
```

This ensures:
- **Idempotency**: Re-ingesting same content yields same ID
- **Multi-tenancy isolation**: Tenant + record ID prevents collisions
- **Traceability**: Derived IDs are reproducible

---

## Configuration

### IngestConfig

Central configuration struct controlling all ingest behavior:

```rust
pub struct IngestConfig {
    /// Configuration version for tracking breaking changes
    pub version: u32,
    
    /// Default tenant when not provided in metadata
    pub default_tenant_id: String,
    
    /// Namespace for UUIDv5 doc ID generation
    pub doc_id_namespace: Uuid,
    
    /// Strip control characters from metadata strings
    pub strip_control_chars: bool,
    
    /// Metadata validation policies
    pub metadata_policy: MetadataPolicy,
    
    /// Maximum raw payload size (bytes)
    pub max_payload_bytes: Option<usize>,
    
    /// Maximum normalized text size (bytes)
    pub max_normalized_bytes: Option<usize>,
}
```

### MetadataPolicy

Fine-grained control over metadata requirements:

```rust
pub struct MetadataPolicy {
    /// Fields that must be present (after sanitization)
    pub required_fields: Vec<RequiredField>,
    
    /// Maximum size for attributes JSON blob
    pub max_attribute_bytes: Option<usize>,
    
    /// Reject timestamps in the future
    pub reject_future_timestamps: bool,
}
```

### RequiredField

Fields that can be made mandatory:

- `TenantId` - Multi-tenant isolation identifier
- `DocId` - Document identifier (must provide explicitly)
- `ReceivedAt` - Timestamp when content was received
- `OriginalSource` - Human-readable source reference

### Configuration Validation

Always validate configuration at startup:

```rust
let config = load_config()?;
config.validate().map_err(|e| {
    eprintln!("Invalid ingest configuration: {}", e);
    std::process::exit(1);
})?;
```

**Validation Rules:**
- `max_normalized_bytes` ≤ `max_payload_bytes` (normalized can't exceed raw)
- No duplicate entries in `required_fields`
- `doc_id_namespace` must be valid UUID

---

## API Reference

### Main Function

#### `ingest()`

```rust
pub fn ingest(
    record: RawIngestRecord,
    cfg: &IngestConfig,
) -> Result<CanonicalIngestRecord, IngestError>
```

**Primary entry point** for the ingest pipeline.

**Parameters:**
- `record`: Raw input with metadata and optional payload
- `cfg`: Runtime configuration

**Returns:**
- `Ok(CanonicalIngestRecord)`: Clean, normalized record
- `Err(IngestError)`: Specific error variant describing the failure

**Side Effects:**
- Emits structured tracing spans (success/failure)
- Records timing metrics

**Example:**
```rust
match ingest(record, &config) {
    Ok(canonical) => {
        tracing::info!(doc_id = %canonical.doc_id, "ingest_success");
        // Process canonical record...
    }
    Err(IngestError::PayloadTooLarge(msg)) => {
        tracing::warn!(error = %msg, "payload_rejected");
        // Return 413 Payload Too Large to client...
    }
    Err(e) => {
        tracing::error!(error = %e, "ingest_failed");
        // Handle other errors...
    }
}
```

### Utility Functions

#### `normalize_payload()`

```rust
pub fn normalize_payload(text: &str) -> String
```

Collapses whitespace in text:
- Trims leading/trailing whitespace
- Collapses multiple spaces/tabs/newlines into single spaces
- Preserves Unicode characters

**Example:**
```rust
let raw = "  Hello   \n\n  world\t\t!  ";
let normalized = normalize_payload(raw);
assert_eq!(normalized, "Hello world !");
```

### Data Types

See the [types module](src/types.rs) for complete definitions of:
- `RawIngestRecord` - Input structure
- `CanonicalIngestRecord` - Output structure  
- `IngestMetadata` - Metadata container
- `IngestPayload` / `CanonicalPayload` - Payload variants
- `IngestSource` - Source enumeration

---

## Error Handling

### IngestError Variants

All errors are typed and cloneable for easy handling:

| Error | Trigger | HTTP Equivalent |
|-------|---------|-----------------|
| `MissingPayload` | Source requires payload but none provided | 400 Bad Request |
| `EmptyBinaryPayload` | Binary payload has zero bytes | 400 Bad Request |
| `InvalidMetadata(msg)` | Policy violation or bad metadata | 400 Bad Request |
| `InvalidUtf8(msg)` | TextBytes not valid UTF-8 | 400 Bad Request |
| `EmptyNormalizedText` | Text empty after normalization | 400 Bad Request |
| `PayloadTooLarge(msg)` | Size limit exceeded | 413 Payload Too Large |

### Error Handling Patterns

**Pattern 1: Map to HTTP Responses**

```rust
use ingest::IngestError;

fn handle_ingest_error(err: IngestError) -> HttpResponse {
    match err {
        IngestError::PayloadTooLarge(_) => {
            HttpResponse::PayloadTooLarge().body(err.to_string())
        }
        IngestError::InvalidMetadata(_) | 
        IngestError::InvalidUtf8(_) |
        IngestError::EmptyNormalizedText |
        IngestError::MissingPayload |
        IngestError::EmptyBinaryPayload => {
            HttpResponse::BadRequest().body(err.to_string())
        }
    }
}
```

**Pattern 2: Structured Logging**

```rust
use tracing::{error, warn};

match ingest(record, &config) {
    Ok(canonical) => Ok(canonical),
    Err(e @ IngestError::PayloadTooLarge(_)) => {
        warn!(error = %e, "payload_size_exceeded");
        Err(e)
    }
    Err(e) => {
        error!(error = %e, "ingest_failure");
        Err(e)
    }
}
```

---

## Examples

### Example 1: Web API Handler

```rust
use axum::{extract::State, http::StatusCode, Json};
use ingest::{ingest, IngestConfig, RawIngestRecord, IngestSource, IngestMetadata, IngestPayload};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
struct UploadRequest {
    content: String,
    tenant_id: String,
    doc_id: Option<String>,
}

async fn upload_content(
    State(config): State<Arc<IngestConfig>>,
    Json(req): Json<UploadRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let record = RawIngestRecord {
        id: uuid::Uuid::new_v4().to_string(),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: Some(req.tenant_id),
            doc_id: req.doc_id,
            received_at: Some(chrono::Utc::now()),
            original_source: None,
            attributes: None,
        },
        payload: Some(IngestPayload::Text(req.content)),
    };
    
    match ingest(record, &config) {
        Ok(canonical) => {
            Ok(Json(serde_json::json!({
                "doc_id": canonical.doc_id,
                "tenant_id": canonical.tenant_id,
                "status": "ingested"
            })))
        }
        Err(e) => {
            tracing::error!(error = %e, "ingest_failed");
            Err(StatusCode::BAD_REQUEST)
        }
    }
}
```

### Example 2: File Upload Handler

```rust
use ingest::{IngestSource, IngestPayload};

async fn handle_file_upload(
    filename: String,
    content_type: String,
    bytes: Vec<u8>,
    tenant_id: String,
) -> Result<String, IngestError> {
    let record = RawIngestRecord {
        id: format!("upload-{}", uuid::Uuid::new_v4()),
        source: IngestSource::File {
            filename: filename.clone(),
            content_type: Some(content_type),
        },
        metadata: IngestMetadata {
            tenant_id: Some(tenant_id),
            doc_id: Some(filename),
            received_at: Some(Utc::now()),
            original_source: None,
            attributes: None,
        },
        payload: Some(IngestPayload::Binary(bytes)),
    };
    
    let canonical = ingest(record, &CONFIG)?;
    Ok(canonical.doc_id)
}
```

### Example 3: Batch Processing

```rust
use ingest::RawIngestRecord;
use rayon::prelude::*;

fn process_batch(
    records: Vec<RawIngestRecord>,
    config: &IngestConfig,
) -> (Vec<CanonicalIngestRecord>, Vec<(String, IngestError)>) {
    let mut successes = Vec::new();
    let mut failures = Vec::new();
    
    for record in records {
        let id = record.id.clone();
        match ingest(record, config) {
            Ok(canonical) => successes.push(canonical),
            Err(e) => failures.push((id, e)),
        }
    }
    
    (successes, failures)
}

// Or using rayon for parallel processing
fn process_batch_parallel(
    records: Vec<RawIngestRecord>,
    config: &IngestConfig,
) -> Vec<Result<CanonicalIngestRecord, IngestError>> {
    // Note: ingest() is pure, so parallel processing is safe
    records
        .into_par_iter()
        .map(|record| ingest(record, config))
        .collect()
}
```

### Example 4: Custom Metadata Attributes

```rust
use serde_json::json;

let record = RawIngestRecord {
    id: "doc-with-attrs".to_string(),
    source: IngestSource::Api,
    metadata: IngestMetadata {
        tenant_id: Some("tenant-1".to_string()),
        doc_id: Some("custom-id".to_string()),
        received_at: Some(Utc::now()),
        original_source: Some("https://example.com/source".to_string()),
        attributes: Some(json!({
            "category": "report",
            "priority": "high",
            "tags": ["finance", "q4", "earnings"],
            "metadata": {
                "author": "Jane Smith",
                "department": "Finance"
            }
        })),
    },
    payload: Some(IngestPayload::Text(report_content)),
};

let canonical = ingest(record, &config)?;
// Access attributes in downstream processing
if let Some(attrs) = &canonical.attributes {
    println!("Category: {}", attrs["category"]);
}
```

---

## Best Practices

### 1. Always Validate Configuration at Startup

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config()?;
    if let Err(e) = config.validate() {
        eprintln!("Configuration error: {}", e);
        std::process::exit(1);
    }
    // ...
}
```

### 2. Set Production-Ready Size Limits

```rust
let config = IngestConfig {
    max_payload_bytes: Some(100 * 1024 * 1024),      // 100 MB
    max_normalized_bytes: Some(50 * 1024 * 1024),    // 50 MB
    metadata_policy: MetadataPolicy {
        max_attribute_bytes: Some(1024 * 1024),      // 1 MB
        ..Default::default()
    },
    ..Default::default()
};
```

### 3. Use Namespaced Document IDs

```rust
use uuid::Uuid;

// Create a namespace unique to your application
const DOC_NAMESPACE: Uuid = Uuid::from_u128(0x1234567890abcdef1234567890abcdef);

let config = IngestConfig {
    doc_id_namespace: DOC_NAMESPACE,
    ..Default::default()
};
```

### 4. Handle Errors Appropriately

```rust
match ingest(record, &config) {
    Ok(canonical) => process(canonical),
    Err(IngestError::PayloadTooLarge(_)) => {
        metrics::counter!("ingest.payload_too_large").increment(1);
        Err(StatusCode::PAYLOAD_TOO_LARGE)
    }
    Err(e) => {
        tracing::error!(error = %e, "unexpected_ingest_error");
        Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
}
```

### 5. Enable Control Character Stripping

Always keep `strip_control_chars: true` to prevent:
- Log injection attacks
- Terminal escape sequences
- Issues with downstream processing

### 6. Use Structured Logging

```rust
// Good
info!(
    tenant_id = %canonical.tenant_id,
    doc_id = %canonical.doc_id,
    payload_size = payload_length,
    duration_ms = elapsed.as_millis(),
    "ingest_success"
);

// Bad
info!("Successfully ingested document");
```

---

## Performance

### Benchmarks

Typical performance on modern hardware:

| Operation | Latency (μs) | Throughput |
|-----------|--------------|------------|
| Empty payload validation | ~5 | 200K ops/sec |
| Small text (1KB) | ~15 | 65K ops/sec |
| Medium text (100KB) | ~200 | 5K ops/sec |
| Large text (10MB) | ~50ms | 20 ops/sec |
| Binary payload (10MB) | ~10μs | 100K ops/sec |

### Optimization Tips

1. **Reuse Config**: `IngestConfig` is cheap to clone
2. **Batch Processing**: Process multiple records together
3. **Size Limits**: Prevent abuse with appropriate limits
4. **Parallel Processing**: `ingest()` is pure and thread-safe

### Memory Usage

- Base overhead: ~200 bytes per ingest call
- Text normalization: Allocates new String (2x input size during processing)
- Binary passthrough: Zero-copy (references original bytes)

---

## Troubleshooting

### Common Issues

#### "EmptyNormalizedText" Error

**Problem**: Text becomes empty after whitespace normalization.

**Common Causes:**
- Input is whitespace-only (`"   "`)
- Input contains only control characters
- Input is empty string

**Solutions:**
```rust
// Check before ingest
if content.trim().is_empty() {
    return Err("Content cannot be empty");
}

// Or catch the error
match ingest(record, &config) {
    Err(IngestError::EmptyNormalizedText) => {
        eprintln!("Please provide non-empty content");
    }
    // ...
}
```

#### "InvalidUtf8" Error

**Problem**: `TextBytes` payload contains invalid UTF-8.

**Solutions:**
```rust
// Option 1: Use Binary payload for non-text data
IngestPayload::Binary(bytes)

// Option 2: Detect encoding and convert
use encoding_rs::Encoding;
let (decoded, _, had_errors) = Encoding::windows_1252().decode(&bytes);
if !had_errors {
    IngestPayload::Text(decoded.to_string())
}

// Option 3: Use lossy conversion
let text = String::from_utf8_lossy(&bytes).to_string();
IngestPayload::Text(text)
```

#### "PayloadTooLarge" Error

**Problem**: Payload exceeds configured size limits.

**Solutions:**
```rust
// Check size before creating record
if content.len() > config.max_payload_bytes.unwrap_or(usize::MAX) {
    return Err("Payload too large");
}

// Or handle gracefully
match ingest(record, &config) {
    Err(IngestError::PayloadTooLarge(msg)) => {
        tracing::warn!(error = %msg, "large_payload_rejected");
        return Ok(HttpResponse::PayloadTooLarge()
            .body("Content too large. Max size: 10MB"));
    }
    // ...
}
```

#### Document ID Collisions

**Problem**: Different documents getting the same ID.

**Causes & Solutions:**

1. **Duplicate ingest IDs**: Ensure unique `id` per record
   ```rust
   id: format!("{}-{}", tenant_id, uuid::Uuid::new_v4())
   ```

2. **Namespace collision**: Use unique namespace per application
   ```rust
   doc_id_namespace: Uuid::new_v5(&Uuid::NAMESPACE_DNS, b"myapp.example.com")
   ```

3. **Better: Always provide explicit doc_id**
   ```rust
   metadata: IngestMetadata {
       doc_id: Some(external_doc_id), // Never derive
       // ...
   }
   ```

### Debugging

Enable debug logging:

```rust
// In your main.rs or test
tracing_subscriber::fmt()
    .with_env_filter("ingest=debug")
    .init();
```

Trace output includes:
- Record IDs
- Timing information
- Payload sizes
- Error details
- Metadata field processing

---

## Integration Guide

### With Axum/Web Frameworks

```rust
use axum::{
    routing::post,
    Router,
    extract::State,
    Json,
};
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    ingest_config: Arc<IngestConfig>,
}

async fn ingest_handler(
    State(state): State<AppState>,
    Json(record): Json<RawIngestRecord>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match ingest(record, &state.ingest_config) {
        Ok(canonical) => Ok(Json(json!({"status": "ok", "doc_id": canonical.doc_id}))),
        Err(e) => {
            tracing::error!(error = %e, "ingest_failed");
            Err(StatusCode::BAD_REQUEST)
        }
    }
}

let app = Router::new()
    .route("/ingest", post(ingest_handler))
    .with_state(AppState { 
        ingest_config: Arc::new(config) 
    });
```

### With tokio::sync::mpsc

```rust
use tokio::sync::mpsc;

let (tx, mut rx) = mpsc::channel::<RawIngestRecord>(1000);

// Producer
tokio::spawn(async move {
    for record in records {
        if tx.send(record).await.is_err() {
            break;
        }
    }
});

// Consumer
while let Some(record) = rx.recv().await {
    match ingest(record, &config) {
        Ok(canonical) => process(canonical).await,
        Err(e) => handle_error(e).await,
    }
}
```

---

## Testing

### Running Tests

```bash
# Run all tests
cargo test -p ingest

# Run with output
cargo test -p ingest -- --nocapture

# Run specific test
cargo test -p ingest test_ingest_rawtext_success

# Run benchmarks
cargo bench -p ingest
```

### Example Programs

```bash
# Single text payload
cargo run --package ingest --example ingest_demo

# Batch processing
cargo run --package ingest --example batch_ingest

# Size limits demo
cargo run --package ingest --example size_limit_demo
```

---

---

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](../../LICENSE) for details.

---

## Contributing

Contributions are welcome! Please ensure:
- All tests pass: `cargo test -p ingest`
- Documentation is updated
- Examples are provided for new features
- Error handling is comprehensive

---

## Support

For issues and questions:
- GitHub Issues: [github.com/bravo1goingdark/ufcp/issues](https://github.com/bravo1goingdark/ufcp/issues)
- Documentation: [docs.rs/ingest](https://docs.rs/ingest)
