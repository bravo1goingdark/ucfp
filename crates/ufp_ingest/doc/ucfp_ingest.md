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
    pub metadata_policy: MetadataPolicy,
    pub max_payload_bytes: Option<usize>,
    pub max_normalized_bytes: Option<usize>,
}

pub struct MetadataPolicy {
    pub required_fields: Vec<RequiredField>,
    pub max_attribute_bytes: Option<usize>,
    pub reject_future_timestamps: bool,
}

#[non_exhaustive]
pub enum RequiredField {
    TenantId,
    DocId,
    ReceivedAt,
    OriginalSource,
}

#[non_exhaustive]
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

#[non_exhaustive]
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

#[non_exhaustive]
pub enum CanonicalPayload {
    Text(String),
    Binary(Vec<u8>),
}
```

### IngestConfig

`IngestConfig` controls how the ingest pipeline behaves at runtime:

- `version`: semantic version of the ingest behavior. This is logged with each record so you can correlate
  behavior with configuration rollouts.
- `default_tenant_id`: fallback tenant identifier used when `IngestMetadata::tenant_id` is absent or empty.
  This ensures every canonical record is associated with some tenant, even when callers omit it.
- `doc_id_namespace`: namespace UUID used when deterministically deriving document IDs via UUIDv5.
  Given a namespace, tenant, and ingest id, the generated `doc_id` is stable across runs.
- `strip_control_chars`: when `true`, ASCII control characters are stripped from metadata strings
  (tenant id, doc id, original source) before further processing and logging.
- `metadata_policy`: a [`MetadataPolicy`] instance that describes which metadata fields are required
  and how large the `attributes` JSON blob is allowed to be.
- `max_payload_bytes`: optional limit on the size of the *raw* payload, in bytes, before any normalization.
  If present and exceeded, ingest fails fast with `IngestError::PayloadTooLarge`.
- `max_normalized_bytes`: optional limit on the size of the normalized text payload. This is enforced
  *after* whitespace collapsing and is independent of the raw size limit.

`IngestConfig::default()` provides a conservative baseline: version `1`, a `"default"` tenant, a stable
UUID namespace, control-character stripping enabled, and no payload-size limits.

### MetadataPolicy and RequiredField

`MetadataPolicy` lets you enforce metadata presence and constrain arbitrary attributes:

- `required_fields`: a list of [`RequiredField`] values that must be present *after* sanitization and
  defaulting. For example, adding `TenantId` here forces callers to explicitly provide a non-empty
  tenant id instead of relying on `default_tenant_id`.
- `max_attribute_bytes`: optional upper bound, in bytes, on the serialized size of `IngestMetadata::attributes`.
  This protects downstream systems from very large or abusive metadata blobs.
- `reject_future_timestamps`: when `true`, ingests whose `received_at` is strictly in the future (relative
  to `Utc::now()`) are rejected with `IngestError::InvalidMetadata`.

`RequiredField` is marked `#[non_exhaustive]` so additional required-field kinds can be added in the future
without breaking downstream code. Callers should always include a catch-all arm when matching it.

### IngestSource

`IngestSource` describes where the raw content came from:

- `RawText`: plain text supplied directly in the request body.
- `Url(String)`: content logically associated with a URL (e.g., text crawled from a web page).
- `File { filename, content_type }`: an uploaded file, such as an image, PDF, or document. Binary payloads
  are typically paired with this variant.
- `Api`: catch-all for ingests that originate from an API call without a more specific source.

The enum is `#[non_exhaustive]`, so future source kinds can be added without breaking callers.

### IngestMetadata

`IngestMetadata` holds caller-supplied metadata attached to a single ingest:

- `tenant_id`: optional tenant; when missing or empty, it is sanitized and may fall back to
  `IngestConfig::default_tenant_id` unless `MetadataPolicy` marks it as required.
- `doc_id`: optional external document identifier; if absent, a deterministic UUIDv5 is derived from the
  tenant and ingest id using `doc_id_namespace`.
- `received_at`: optional timestamp associated with when the content was received or observed.
  When omitted and not required by policy, it defaults to the current UTC time.
- `original_source`: optional human-meaningful identifier such as an upstream URL, external id, or path.
  Control characters are stripped so this string is safe to log and store.
- `attributes`: optional arbitrary JSON, e.g., extra tags, labels, or contextual information.
  This field is size-limited by `MetadataPolicy::max_attribute_bytes`.

### IngestPayload

`IngestPayload` represents the raw content flowing into the ingest layer:

- `Text(String)`: UTF-8 text that will be whitespace-normalized via `normalize_payload`.
- `TextBytes(Vec<u8>)`: raw bytes expected to contain UTF-8 text; invalid UTF-8 results in
  `IngestError::InvalidUtf8`.
- `Binary(Vec<u8>)`: arbitrary binary blob (images, audio, documents). Binary data is passed through
  unmodified except for an emptiness check.

Some `IngestSource` variants require a payload (`RawText`, `File`), and some additionally require that the
payload normalize to text (`RawText`, `Url`). These rules are enforced during ingest.

### RawIngestRecord

`RawIngestRecord` is the complete request structure accepted by `ingest`:

- `id`: ingest id supplied by callers. This is a stable identifier used for tracing, log correlation,
  and deterministic document-id derivation.
- `source`: an [`IngestSource`] describing where the payload came from.
- `metadata`: an [`IngestMetadata`] instance containing caller-supplied metadata.
- `payload`: optional [`IngestPayload`]. Some sources require this to be present, while others
  may legitimately carry no payload (for metadata-only events).

### CanonicalIngestRecord and CanonicalPayload

`CanonicalIngestRecord` is the normalized, validated representation emitted by `ingest` and consumed by
later stages in the pipeline:

- `id`: the sanitized ingest id (after control-character stripping and trimming).
- `tenant_id`: effective tenant identifier after applying defaults and policy checks.
- `doc_id`: either the sanitized caller-supplied document id or a deterministic UUIDv5 derived from the
  tenant and ingest id.
- `received_at`: resolved timestamp, defaulting to the current time when not required and not supplied.
- `original_source`: sanitized version of `IngestMetadata::original_source`.
- `source`: the original [`IngestSource`] variant from the raw record.
- `normalized_payload`: optional [`CanonicalPayload`]; either normalized text or preserved binary bytes.
- `attributes`: the final `attributes` JSON blob after size checks.

`CanonicalPayload` is the normalized payload ready for downstream processing:

- `Text(String)`: text after whitespace collapsing and size-limit checks.
- `Binary(Vec<u8>)`: non-empty binary payload preserved exactly as provided by the caller.

The enum is `#[non_exhaustive]` so future payload representations (e.g., structured text segments) can be
introduced without a breaking change.

The ingest id is supplied by callers. Optional metadata is automatically normalized: tenant ids fall back
to the configuration default, document ids are deterministically derived from the ingest id and tenant when
omitted, and missing timestamps are replaced with the current UTC time unless a `MetadataPolicy` requires
them to be present. Control characters in metadata are stripped to prevent log or storage injection. Policy
defined via [`MetadataPolicy`] additionally allows you to reject ingests that omit required metadata, cap
the serialized byte length of `metadata.attributes`, and reject timestamps that lie in the future.

## Public API

```rust
pub fn ingest(record: RawIngestRecord, cfg: &IngestConfig)
    -> Result<CanonicalIngestRecord, IngestError>;

pub fn ingest_with_observer<O>(
    record: RawIngestRecord,
    cfg: &IngestConfig,
    observer: &O,
) -> Result<CanonicalIngestRecord, IngestError>
where
    O: IngestObserver + ?Sized;

pub fn normalize_payload(text: &str) -> String;

pub trait IngestObserver {
    fn on_success(&self, record: &CanonicalIngestRecord, elapsed_micros: u128);
    fn on_failure(&self, error: &IngestError, elapsed_micros: u128);
}

impl IngestConfig {
    pub fn validate(&self) -> Result<(), ConfigError>;
}
```

`ingest` records structured tracing spans for every attempt, including normalized payload length,
tenant/doc identifiers, and outcome. The function rejects:

- payloads that decode to empty text (`IngestError::EmptyNormalizedText`), i.e., text that becomes empty
  after whitespace collapsing.
- missing or invalid metadata (`IngestError::InvalidMetadata`), such as absent required fields, attributes
  that exceed `max_attribute_bytes`, or timestamps that violate policy.
- malformed UTF-8 when using `IngestPayload::TextBytes` (`IngestError::InvalidUtf8`), triggered when the
  incoming byte sequence cannot be decoded as UTF-8.
- missing payloads for sources that require one (`IngestError::MissingPayload`), for example `RawText` or
  `File` sources without any associated payload.
- empty binary payloads (`IngestError::EmptyBinaryPayload`), which are rejected to avoid meaningless
  ingests with zero-length binary content.
- payloads that violate configured size limits on raw or normalized text (`IngestError::PayloadTooLarge`),
  covering both `max_payload_bytes` and `max_normalized_bytes` when set.

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
