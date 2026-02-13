//! Core data model types for the ingest crate.
//!
//! These types represent the shape of ingest requests and the normalized
//! records that flow to downstream pipeline stages. They are designed to be:
//!
//! - **Serializable**: Support for JSON, binary formats via serde
//! - **Cloneable**: Cheap to clone for pipeline processing
//! - **Comparable**: Support equality checks for testing
//! - **Extensible**: Marked `#[non_exhaustive]` where appropriate
//!
//! # Type Hierarchy
//!
//! ```text
//! RawIngestRecord
//! ├── id: String
//! ├── source: IngestSource
//! ├── metadata: IngestMetadata
//! │   ├── tenant_id: Option<String>
//! │   ├── doc_id: Option<String>
//! │   ├── received_at: Option<DateTime<Utc>>
//! │   ├── original_source: Option<String>
//! │   └── attributes: Option<Value>
//! └── payload: Option<IngestPayload>
//!     ├── Text(String)
//!     ├── TextBytes(Vec<u8>)
//!     └── Binary(Vec<u8>)
//!
//!         ↓ ingest()
//!
//! CanonicalIngestRecord
//! ├── id: String (sanitized)
//! ├── tenant_id: String (default applied)
//! ├── doc_id: String (derived or provided)
//! ├── received_at: DateTime<Utc> (default applied)
//! ├── original_source: Option<String> (sanitized)
//! ├── source: IngestSource
//! ├── normalized_payload: Option<CanonicalPayload>
//! │   ├── Text(String) (whitespace normalized)
//! │   └── Binary(Vec<u8>) (preserved)
//! └── attributes: Option<Value>
//! ```
//!
//! # Examples
//!
//! ## Creating a Raw Record
//!
//! ```rust
//! use ingest::{
//!     RawIngestRecord, IngestMetadata, IngestSource,
//!     IngestPayload
//! };
//! use chrono::Utc;
//!
//! let record = RawIngestRecord {
//!     id: "doc-001".to_string(),
//!     source: IngestSource::RawText,
//!     metadata: IngestMetadata {
//!         tenant_id: Some("acme-corp".to_string()),
//!         doc_id: Some("report-q4-2024".to_string()),
//!         received_at: Some(Utc::now()),
//!         original_source: None,
//!         attributes: None,
//!     },
//!     payload: Some(IngestPayload::Text(
//!         "Quarterly report content...".to_string()
//!     )),
//! };
//! ```
//!
//! ## Working with Canonical Records
//!
//! ```rust
//! use ingest::{CanonicalIngestRecord, CanonicalPayload};
//!
//! fn process_text(record: &CanonicalIngestRecord) -> Option<String> {
//!     match &record.normalized_payload {
//!         Some(CanonicalPayload::Text(text)) => Some(text.clone()),
//!         Some(CanonicalPayload::Binary(_)) => {
//!             println!("Skipping binary payload");
//!             None
//!         }
//!         None => {
//!             println!("No payload");
//!             None
//!         }
//!     }
//! }
//! ```
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Source kinds accepted at ingest time.
///
/// `IngestSource` identifies where content originated, which affects validation
/// rules (e.g., whether a payload is required) and downstream processing.
///
/// # Source Types
///
/// - `RawText`: Plain text supplied directly (requires text payload)
/// - `Url(String)`: Content from a URL (requires text payload)
/// - `File { filename, content_type }`: Uploaded file (requires payload)
/// - `Api`: Generic API call (payload optional)
///
/// # Payload Requirements
///
/// | Source | Payload Required | Text Required |
/// |--------|-----------------|---------------|
/// | `RawText` | Yes | Yes |
/// | `Url` | Yes | Yes |
/// | `File` | Yes | No |
/// | `Api` | No | No |
///
/// # Examples
///
/// ```rust
/// use ingest::IngestSource;
///
/// // Raw text input
/// let source = IngestSource::RawText;
///
/// // URL-sourced content
/// let source = IngestSource::Url("https://example.com/page".to_string());
///
/// // File upload
/// let source = IngestSource::File {
///     filename: "document.pdf".to_string(),
///     content_type: Some("application/pdf".to_string()),
/// };
///
/// // Generic API call
/// let source = IngestSource::Api;
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub enum IngestSource {
    /// Plain text supplied directly in the request body.
    ///
    /// This source requires a text payload. The content will be whitespace-normalized
    /// during ingest.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ingest::{IngestSource, IngestPayload, RawIngestRecord, IngestMetadata};
    ///
    /// let record = RawIngestRecord {
    ///     id: "text-001".to_string(),
    ///     source: IngestSource::RawText,
    ///     metadata: IngestMetadata {
    ///         tenant_id: Some("tenant".to_string()),
    ///         doc_id: Some("doc".to_string()),
    ///         received_at: None,
    ///         original_source: None,
    ///         attributes: None,
    ///     },
    ///     payload: Some(IngestPayload::Text("Hello world".to_string())),
    /// };
    /// ```
    RawText,

    /// Content logically associated with a URL.
    ///
    /// This source requires a text payload and is typically used for content
    /// crawled from web pages.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ingest::IngestSource;
    ///
    /// let source = IngestSource::Url(
    ///     "https://example.com/article".to_string()
    /// );
    /// ```
    Url(String),

    /// An uploaded file with associated metadata.
    ///
    /// This source requires a payload (text or binary) and captures file metadata
    /// for downstream processing.
    ///
    /// # Fields
    ///
    /// - `filename`: The original filename
    /// - `content_type`: Optional MIME type (e.g., "application/pdf")
    ///
    /// # Example
    ///
    /// ```rust
    /// use ingest::{IngestSource, IngestPayload, RawIngestRecord, IngestMetadata};
    ///
    /// let record = RawIngestRecord {
    ///     id: "file-001".to_string(),
    ///     source: IngestSource::File {
    ///         filename: "report.pdf".to_string(),
    ///         content_type: Some("application/pdf".to_string()),
    ///     },
    ///     metadata: IngestMetadata {
    ///         tenant_id: Some("tenant".to_string()),
    ///         doc_id: Some("doc-123".to_string()),
    ///         received_at: None,
    ///         original_source: Some("uploads/report.pdf".to_string()),
    ///         attributes: None,
    ///     },
    ///     payload: Some(IngestPayload::Binary(vec![0x89, 0x50, 0x4E, 0x47])), // PNG header
    /// };
    /// ```
    File {
        /// The original filename of the uploaded file.
        filename: String,
        /// Optional MIME type of the file (e.g., "application/pdf", "image/png").
        content_type: Option<String>,
    },

    /// Catch-all for ingests originating from an API call.
    ///
    /// Unlike other sources, `Api` does not require a payload, making it suitable
    /// for metadata-only events or API calls without content.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ingest::{IngestSource, RawIngestRecord, IngestMetadata};
    ///
    /// let record = RawIngestRecord {
    ///     id: "api-001".to_string(),
    ///     source: IngestSource::Api,
    ///     metadata: IngestMetadata {
    ///         tenant_id: Some("tenant".to_string()),
    ///         doc_id: Some("doc".to_string()),
    ///         received_at: None,
    ///         original_source: None,
    ///         attributes: Some(serde_json::json!({"event": "user_action"})),
    ///     },
    ///     payload: None, // Optional for Api source
    /// };
    /// ```
    Api,
}

/// Metadata associated with an ingest request.
///
/// `IngestMetadata` carries contextual information about the content being ingested.
/// All fields are optional and will be defaulted during normalization if not provided.
///
/// # Field Defaults
///
/// | Field | Default Behavior |
/// |-------|------------------|
/// | `tenant_id` | Falls back to `IngestConfig::default_tenant_id` |
/// | `doc_id` | Derived via UUIDv5 if not provided |
/// | `received_at` | Set to current UTC time |
/// | `original_source` | Remains `None` if not provided |
/// | `attributes` | Remains `None` if not provided |
///
/// # Examples
///
/// ## Minimal Metadata
///
/// ```rust
/// use ingest::IngestMetadata;
///
/// let metadata = IngestMetadata {
///     tenant_id: None,
///     doc_id: None,
///     received_at: None,
///     original_source: None,
///     attributes: None,
/// };
/// // All fields will be defaulted during ingest
/// ```
///
/// ## Full Metadata
///
/// ```rust
/// use ingest::IngestMetadata;
/// use chrono::Utc;
/// use serde_json::json;
///
/// let metadata = IngestMetadata {
///     tenant_id: Some("acme-corp".to_string()),
///     doc_id: Some("report-q4-2024".to_string()),
///     received_at: Some(Utc::now()),
///     original_source: Some("https://docs.example.com/reports/q4".to_string()),
///     attributes: Some(json!({
///         "department": "Engineering",
///         "classification": "internal",
///         "tags": ["quarterly", "2024"]
///     })),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IngestMetadata {
    /// Optional tenant identifier for multi-tenant isolation.
    ///
    /// When `None` or empty after sanitization, falls back to
    /// `IngestConfig::default_tenant_id`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ingest::IngestMetadata;
    ///
    /// let metadata = IngestMetadata {
    ///     tenant_id: Some("tenant-123".to_string()),
    ///     doc_id: None,
    ///     received_at: None,
    ///     original_source: None,
    ///     attributes: None,
    /// };
    /// ```
    pub tenant_id: Option<String>,

    /// Optional document identifier.
    ///
    /// When `None` or empty after sanitization, a deterministic UUIDv5 is generated
    /// using `IngestConfig::doc_id_namespace`:
    /// `UUIDv5(namespace, tenant_id + "\0" + record_id)`
    ///
    /// # Example
    ///
    /// ```rust
    /// use ingest::IngestMetadata;
    ///
    /// let metadata = IngestMetadata {
    ///     tenant_id: None,
    ///     doc_id: Some("doc-abc-123".to_string()),
    ///     received_at: None,
    ///     original_source: None,
    ///     attributes: None,
    /// };
    /// ```
    pub doc_id: Option<String>,

    /// Optional timestamp when the content was received.
    ///
    /// When `None`, defaults to the current UTC time at ingest.
    /// Can be validated against future time if
    /// [`MetadataPolicy::reject_future_timestamps`](crate::MetadataPolicy::reject_future_timestamps)
    /// is enabled.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ingest::IngestMetadata;
    /// use chrono::Utc;
    ///
    /// let metadata = IngestMetadata {
    ///     tenant_id: None,
    ///     doc_id: None,
    ///     received_at: Some(Utc::now()),
    ///     original_source: None,
    ///     attributes: None,
    /// };
    /// ```
    pub received_at: Option<DateTime<Utc>>,

    /// Optional original source identifier (e.g., URL or external ID).
    ///
    /// This is a human-readable reference to where the content originated.
    /// Control characters are stripped during sanitization.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ingest::IngestMetadata;
    ///
    /// let metadata = IngestMetadata {
    ///     tenant_id: None,
    ///     doc_id: None,
    ///     received_at: None,
    ///     original_source: Some("https://example.com/source".to_string()),
    ///     attributes: None,
    /// };
    /// ```
    pub original_source: Option<String>,

    /// Arbitrary JSON attributes for extensibility.
    ///
    /// This field can store any JSON-serializable data for application-specific
    /// use cases. Size is limited by
    /// [`MetadataPolicy::max_attribute_bytes`](crate::MetadataPolicy::max_attribute_bytes)
    /// when configured.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ingest::IngestMetadata;
    /// use serde_json::json;
    ///
    /// let metadata = IngestMetadata {
    ///     tenant_id: None,
    ///     doc_id: None,
    ///     received_at: None,
    ///     original_source: None,
    ///     attributes: Some(json!({
    ///         "category": "report",
    ///         "priority": "high",
    ///         "metadata": {
    ///             "author": "Jane Smith",
    ///             "department": "Engineering"
    ///         }
    ///     })),
    /// };
    /// ```
    pub attributes: Option<serde_json::Value>,
}

/// The inbound record for ingest.
///
/// `RawIngestRecord` is the primary input type for the ingest pipeline. It contains
/// all information needed to process content: identification, source metadata, and
/// optional payload.
///
/// # Lifecycle
///
/// 1. Create `RawIngestRecord` with raw data
/// 2. Call [`ingest()`](crate::ingest) to normalize
/// 3. Receive [`CanonicalIngestRecord`] for downstream processing
///
/// # Examples
///
/// ## Text Content
///
/// ```rust
/// use ingest::{RawIngestRecord, IngestMetadata, IngestSource, IngestPayload};
/// use chrono::Utc;
///
/// let record = RawIngestRecord {
///     id: "text-001".to_string(),
///     source: IngestSource::RawText,
///     metadata: IngestMetadata {
///         tenant_id: Some("tenant".to_string()),
///         doc_id: Some("doc".to_string()),
///         received_at: Some(Utc::now()),
///         original_source: None,
///         attributes: None,
///     },
///     payload: Some(IngestPayload::Text(
///         "  Content with   extra spaces  ".to_string()
///     )),
/// };
/// ```
///
/// ## Binary File
///
/// ```rust
/// use ingest::{RawIngestRecord, IngestMetadata, IngestSource, IngestPayload};
///
/// let record = RawIngestRecord {
///     id: "file-001".to_string(),
///     source: IngestSource::File {
///         filename: "image.png".to_string(),
///         content_type: Some("image/png".to_string()),
///     },
///     metadata: IngestMetadata {
///         tenant_id: Some("tenant".to_string()),
///         doc_id: Some("doc-123".to_string()),
///         received_at: None,
///         original_source: Some("uploads/image.png".to_string()),
///         attributes: None,
///     },
///     payload: Some(IngestPayload::Binary(vec![0x89, 0x50, 0x4E, 0x47])),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RawIngestRecord {
    /// Unique identifier for this ingest operation.
    ///
    /// This ID is used for:
    /// - Tracing and log correlation
    /// - Deterministic document ID derivation (when `doc_id` not provided)
    /// - Deduplication and idempotency
    ///
    /// Should be unique per ingest request. If not provided, a UUID should be
    /// generated by the caller.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ingest::RawIngestRecord;
    ///
    /// let record = RawIngestRecord {
    ///     id: "ingest-550e8400-e29b-41d4-a716-446655440000".to_string(),
    ///     ..Default::default()
    /// };
    /// ```
    pub id: String,

    /// Source of the content.
    ///
    /// Indicates where the content came from and affects validation rules.
    /// See [`IngestSource`] for details.
    pub source: IngestSource,

    /// Metadata associated with the record.
    ///
    /// Contains contextual information like tenant, timestamps, and custom attributes.
    /// See [`IngestMetadata`] for details.
    pub metadata: IngestMetadata,

    /// Raw payload content.
    ///
    /// The actual content being ingested. May be `None` for metadata-only events
    /// (e.g., `IngestSource::Api`).
    ///
    /// See [`IngestPayload`] for the different payload types.
    pub payload: Option<IngestPayload>,
}

impl Default for RawIngestRecord {
    fn default() -> Self {
        Self {
            id: String::new(),
            source: IngestSource::Api,
            metadata: IngestMetadata {
                tenant_id: None,
                doc_id: None,
                received_at: None,
                original_source: None,
                attributes: None,
            },
            payload: None,
        }
    }
}

/// Raw payload content provided during ingest.
///
/// `IngestPayload` supports multi-modal content ingestion, allowing the same
/// pipeline to handle text and binary data uniformly.
///
/// # Payload Types
///
/// - `Text(String)`: Clean UTF-8 text (will be whitespace-normalized)
/// - `TextBytes(Vec<u8>)`: Raw bytes expected to be valid UTF-8 (will be validated + normalized)
/// - `Binary(Vec<u8>)`: Arbitrary binary data (passed through unchanged)
///
/// # Processing
///
/// | Variant | Validation | Normalization | Size Limits |
/// |---------|-----------|---------------|-------------|
/// | `Text` | None | Whitespace collapsed | Both limits |
/// | `TextBytes` | UTF-8 | Whitespace collapsed | Both limits |
/// | `Binary` | Non-empty | None | Raw limit only |
///
/// # Examples
///
/// ```rust
/// use ingest::IngestPayload;
///
/// // Text payload
/// let text = IngestPayload::Text("Hello world".to_string());
///
/// // Text from bytes (validates UTF-8)
/// let text_bytes = IngestPayload::TextBytes(b"Hello world".to_vec());
///
/// // Binary payload (preserved as-is)
/// let binary = IngestPayload::Binary(vec![0x89, 0x50, 0x4E, 0x47]); // PNG magic
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub enum IngestPayload {
    /// UTF-8 text payload for normalization and canonicalization.
    ///
    /// This is the preferred variant for text content. The text will have
    /// whitespace collapsed during ingest.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ingest::IngestPayload;
    ///
    /// let payload = IngestPayload::Text(
    ///     "  Content with   extra whitespace  ".to_string()
    /// );
    /// // After ingest: "Content with extra whitespace"
    /// ```
    Text(String),

    /// Raw UTF-8 bytes that will be decoded during ingest.
    ///
    /// Use this variant when you have bytes that should be valid UTF-8 but
    /// need validation. Invalid UTF-8 will result in
    /// [`IngestError::InvalidUtf8`](crate::IngestError::InvalidUtf8).
    ///
    /// # Example
    ///
    /// ```rust
    /// use ingest::IngestPayload;
    ///
    /// let payload = IngestPayload::TextBytes(
    ///     b"Hello from bytes".to_vec()
    /// );
    /// ```
    ///
    /// # Error
    ///
    /// ```rust
    /// use ingest::{IngestPayload, ingest, IngestError};
    /// use ingest::{RawIngestRecord, IngestMetadata, IngestSource, IngestConfig};
    ///
    /// let record = RawIngestRecord {
    ///     id: "test".to_string(),
    ///     source: IngestSource::RawText,
    ///     metadata: IngestMetadata {
    ///         tenant_id: Some("t".to_string()),
    ///         doc_id: Some("d".to_string()),
    ///         received_at: None,
    ///         original_source: None,
    ///         attributes: None,
    ///     },
    ///     payload: Some(IngestPayload::TextBytes(vec![0xFF, 0xFE])), // Invalid UTF-8
    /// };
    ///
    /// // This will fail with InvalidUtf8
    /// // let result = ingest(record, &IngestConfig::default());
    /// ```
    TextBytes(Vec<u8>),

    /// Arbitrary binary payload for downstream processing.
    ///
    /// Binary payloads are passed through unmodified (except for emptiness check).
    /// They are suitable for images, PDFs, audio files, and other non-text content.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ingest::IngestPayload;
    ///
    /// // PNG file header
    /// let payload = IngestPayload::Binary(vec![0x89, 0x50, 0x4E, 0x47]);
    /// ```
    ///
    /// # Validation
    ///
    /// Empty binary payloads (zero bytes) are rejected with
    /// [`IngestError::EmptyBinaryPayload`](crate::IngestError::EmptyBinaryPayload).
    Binary(Vec<u8>),
}

/// Normalized record produced by ingest.
///
/// `CanonicalIngestRecord` is the output of the ingest pipeline. It represents
/// a cleaned, validated, and deterministic version of the input that downstream
/// stages can rely on.
///
/// # Guarantees
///
/// - All required fields are present (tenant_id, doc_id, received_at)
/// - Metadata is sanitized (control characters stripped)
/// - Payload is normalized (text whitespace collapsed, binary preserved)
/// - Document ID is stable (derived deterministically if not provided)
///
/// # Examples
///
/// ```rust
/// use ingest::{ingest, IngestConfig, RawIngestRecord, CanonicalPayload};
/// use ingest::{IngestMetadata, IngestSource, IngestPayload};
///
/// let config = IngestConfig::default();
/// let record = RawIngestRecord {
///     id: "test-001".to_string(),
///     source: IngestSource::RawText,
///     metadata: IngestMetadata {
///         tenant_id: Some("tenant".to_string()),
///         doc_id: None, // Will be derived
///         received_at: None, // Will default to now
///         original_source: None,
///         attributes: None,
///     },
///     payload: Some(IngestPayload::Text("  Hello   world  ".to_string())),
/// };
///
/// let canonical = ingest(record, &config).unwrap();
///
/// // All fields are guaranteed present
/// assert!(!canonical.tenant_id.is_empty());
/// assert!(!canonical.doc_id.is_empty());
///
/// // Text is normalized
/// match &canonical.normalized_payload {
///     Some(CanonicalPayload::Text(text)) => {
///         assert_eq!(text, "Hello world");
///     }
///     _ => panic!("Expected text payload"),
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CanonicalIngestRecord {
    /// Unique identifier for this ingest operation (mirrors [`RawIngestRecord::id`]).
    ///
    /// This is the sanitized version of the original ID (control characters stripped).
    pub id: String,

    /// Tenant identifier for multi-tenant isolation.
    ///
    /// This is the effective tenant ID after applying defaults:
    /// - If provided and non-empty: the sanitized provided value
    /// - Otherwise: `IngestConfig::default_tenant_id`
    pub tenant_id: String,

    /// Document identifier.
    ///
    /// This is the effective document ID after derivation:
    /// - If provided and non-empty: the sanitized provided value
    /// - Otherwise: UUIDv5 derived from tenant + record ID
    pub doc_id: String,

    /// Timestamp when the record was received.
    ///
    /// This is the effective timestamp after applying defaults:
    /// - If provided: the sanitized provided value
    /// - Otherwise: current UTC time at ingest
    pub received_at: DateTime<Utc>,

    /// Original source information if provided.
    ///
    /// Sanitized version of [`IngestMetadata::original_source`] with control
    /// characters stripped. `None` if not provided.
    pub original_source: Option<String>,

    /// Source of the content (mirrors [`RawIngestRecord::source`]).
    pub source: IngestSource,

    /// Normalized payload ready for downstream stages.
    ///
    /// - For text: whitespace collapsed, size limits enforced
    /// - For binary: preserved unchanged, non-empty check performed
    /// - `None` if no payload was provided
    pub normalized_payload: Option<CanonicalPayload>,

    /// Attributes JSON preserved for downstream use.
    ///
    /// This is the sanitized and size-checked version of
    /// [`IngestMetadata::attributes`]. `None` if not provided.
    pub attributes: Option<serde_json::Value>,
}

impl CanonicalIngestRecord {
    /// Returns true if this record has a text payload.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ingest::{CanonicalIngestRecord, CanonicalPayload};
    ///
    /// let record = CanonicalIngestRecord {
    ///     id: "test".to_string(),
    ///     tenant_id: "tenant".to_string(),
    ///     doc_id: "doc".to_string(),
    ///     received_at: chrono::Utc::now(),
    ///     original_source: None,
    ///     source: ingest::IngestSource::RawText,
    ///     normalized_payload: Some(CanonicalPayload::Text("hello".to_string())),
    ///     attributes: None,
    /// };
    ///
    /// assert!(record.has_text_payload());
    /// ```
    pub fn has_text_payload(&self) -> bool {
        matches!(self.normalized_payload, Some(CanonicalPayload::Text(_)))
    }

    /// Returns true if this record has a binary payload.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ingest::{CanonicalIngestRecord, CanonicalPayload};
    ///
    /// let record = CanonicalIngestRecord {
    ///     id: "test".to_string(),
    ///     tenant_id: "tenant".to_string(),
    ///     doc_id: "doc".to_string(),
    ///     received_at: chrono::Utc::now(),
    ///     original_source: None,
    ///     source: ingest::IngestSource::File {
    ///         filename: "test.bin".to_string(),
    ///         content_type: None,
    ///     },
    ///     normalized_payload: Some(CanonicalPayload::Binary(vec![1, 2, 3])),
    ///     attributes: None,
    /// };
    ///
    /// assert!(record.has_binary_payload());
    /// ```
    pub fn has_binary_payload(&self) -> bool {
        matches!(self.normalized_payload, Some(CanonicalPayload::Binary(_)))
    }

    /// Returns the text payload if present, otherwise None.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ingest::{CanonicalIngestRecord, CanonicalPayload};
    ///
    /// let record = CanonicalIngestRecord {
    ///     id: "test".to_string(),
    ///     tenant_id: "tenant".to_string(),
    ///     doc_id: "doc".to_string(),
    ///     received_at: chrono::Utc::now(),
    ///     original_source: None,
    ///     source: ingest::IngestSource::RawText,
    ///     normalized_payload: Some(CanonicalPayload::Text("hello world".to_string())),
    ///     attributes: None,
    /// };
    ///
    /// assert_eq!(record.text_payload(), Some("hello world"));
    /// ```
    pub fn text_payload(&self) -> Option<&str> {
        match &self.normalized_payload {
            Some(CanonicalPayload::Text(text)) => Some(text),
            _ => None,
        }
    }

    /// Returns the binary payload if present, otherwise None.
    pub fn binary_payload(&self) -> Option<&[u8]> {
        match &self.normalized_payload {
            Some(CanonicalPayload::Binary(bytes)) => Some(bytes),
            _ => None,
        }
    }
}

/// Normalized payload ready for downstream stages.
///
/// `CanonicalPayload` represents the final, processed form of ingest payload.
/// Text payloads have whitespace normalized, while binary payloads pass through
/// unchanged.
///
/// # Variants
///
/// - `Text(String)`: Normalized UTF-8 text with collapsed whitespace
/// - `Binary(Vec<u8>)`: Binary payload preserved exactly
///
/// # Examples
///
/// ```rust
/// use ingest::CanonicalPayload;
///
/// // Normalized text
/// let text = CanonicalPayload::Text("Hello world".to_string());
///
/// // Preserved binary
/// let binary = CanonicalPayload::Binary(vec![0x89, 0x50, 0x4E, 0x47]);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub enum CanonicalPayload {
    /// Normalized UTF-8 text payload.
    ///
    /// Text has been through whitespace normalization (multiple spaces/tabs/newlines
    /// collapsed to single spaces, leading/trailing whitespace trimmed).
    ///
    /// # Example
    ///
    /// ```rust
    /// use ingest::CanonicalPayload;
    ///
    /// // This represents text that was "  Hello   world  " before normalization
    /// let payload = CanonicalPayload::Text("Hello world".to_string());
    /// ```
    Text(String),

    /// Binary payload preserved for downstream perceptual/semantic stages.
    ///
    /// Binary data (images, PDFs, audio, etc.) passes through ingest unchanged
    /// except for the non-empty validation.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ingest::CanonicalPayload;
    ///
    /// let payload = CanonicalPayload::Binary(vec![0x89, 0x50, 0x4E, 0x47]);
    /// ```
    Binary(Vec<u8>),
}

impl CanonicalPayload {
    /// Returns the length of the payload in bytes.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ingest::CanonicalPayload;
    ///
    /// let text = CanonicalPayload::Text("Hello".to_string());
    /// assert_eq!(text.len(), 5);
    ///
    /// let binary = CanonicalPayload::Binary(vec![1, 2, 3, 4]);
    /// assert_eq!(binary.len(), 4);
    /// ```
    pub fn len(&self) -> usize {
        match self {
            CanonicalPayload::Text(s) => s.len(),
            CanonicalPayload::Binary(b) => b.len(),
        }
    }

    /// Returns true if the payload is empty.
    ///
    /// Note: Empty payloads should never reach this stage (they are rejected
    /// during ingest), but this method is provided for completeness.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns true if this is a text payload.
    pub fn is_text(&self) -> bool {
        matches!(self, CanonicalPayload::Text(_))
    }

    /// Returns true if this is a binary payload.
    pub fn is_binary(&self) -> bool {
        matches!(self, CanonicalPayload::Binary(_))
    }
}
