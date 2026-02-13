//! Error types produced by the ingest crate.
//!
//! This module defines the error surface for ingest operations. All errors are
//! typed, cloneable, and comparable to enable precise error handling and testing.
//!
//! # Error Philosophy
//!
//! The ingest crate uses typed errors (not generic strings) to allow callers to:
//! - Handle specific error cases differently
//! - Map errors to appropriate HTTP status codes
//! - Display user-friendly error messages
//! - Log structured error information
//!
//! # Error Categories
//!
//! | Error | Category | Description |
//! |-------|----------|-------------|
//! | [`MissingPayload`](IngestError::MissingPayload) | Validation | Source requires payload but none provided |
//! | [`EmptyBinaryPayload`](IngestError::EmptyBinaryPayload) | Validation | Binary payload has zero bytes |
//! | [`InvalidMetadata`](IngestError::InvalidMetadata) | Validation | Metadata policy violation |
//! | [`InvalidUtf8`](IngestError::InvalidUtf8) | Validation | TextBytes not valid UTF-8 |
//! | [`EmptyNormalizedText`](IngestError::EmptyNormalizedText) | Validation | Text empty after normalization |
//! | [`PayloadTooLarge`](IngestError::PayloadTooLarge) | Validation | Size limit exceeded |
//!
//! # HTTP Status Code Mapping
//!
//! ```rust
//! use ingest::IngestError;
//!
//! fn to_http_status(error: &IngestError) -> u16 {
//!     match error {
//!         IngestError::PayloadTooLarge(_) => 413, // Payload Too Large
//!         _ => 400, // Bad Request (all validation errors)
//!     }
//! }
//! ```
//!
//! # Examples
//!
//! ## Basic Error Handling
//!
//! ```rust
//! use ingest::{ingest, IngestConfig, IngestError};
//! use ingest::{RawIngestRecord, IngestMetadata, IngestSource};
//!
//! let config = IngestConfig::default();
//! let record = RawIngestRecord {
//!     id: "test".to_string(),
//!     source: IngestSource::RawText,
//!     metadata: IngestMetadata {
//!         tenant_id: Some("t".to_string()),
//!         doc_id: Some("d".to_string()),
//!         received_at: None,
//!         original_source: None,
//!         attributes: None,
//!     },
//!     payload: None, // Missing required payload
//! };
//!
//! match ingest(record, &config) {
//!     Ok(canonical) => println!("Success: {}", canonical.doc_id),
//!     Err(IngestError::MissingPayload) => {
//!         println!("Error: Payload is required for this source");
//!     }
//!     Err(e) => println!("Error: {}", e),
//! }
//! ```
//!
//! ## Pattern Matching
//!
//! ```rust
//! use ingest::IngestError;
//!
//! fn handle_error(error: IngestError) -> String {
//!     match error {
//!         IngestError::EmptyNormalizedText => {
//!             "Content cannot be empty or whitespace-only".to_string()
//!         }
//!         IngestError::InvalidUtf8(msg) => {
//!             format!("Invalid text encoding: {}", msg)
//!         }
//!         IngestError::PayloadTooLarge(msg) => {
//!             format!("Content too large: {}", msg)
//!         }
//!         _ => error.to_string(),
//!     }
//! }
//! ```
use thiserror::Error;

/// Errors that can occur during ingest normalization and validation.
///
/// These errors represent validation failures that prevent content from being
/// ingested. All variants are:
///
/// - **Cloneable**: Can be copied for error propagation
/// - **Comparable**: Support equality checks for testing
/// - **Displayable**: Implement `std::fmt::Display` for user messages
/// - **Debuggable**: Implement `std::fmt::Debug` for development
///
/// The enum is marked `#[non_exhaustive]` to allow future additions without
/// breaking existing code. Callers should always include a catch-all arm when
/// matching.
///
/// # Examples
///
/// ```rust
/// use ingest::IngestError;
///
/// // Error messages
/// let err = IngestError::MissingPayload;
/// assert_eq!(err.to_string(), "missing payload for source that requires payload");
///
/// let err = IngestError::InvalidMetadata("tenant required".to_string());
/// assert!(err.to_string().contains("tenant required"));
/// ```
#[derive(Error, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum IngestError {
    /// Missing payload for source that requires one.
    ///
    /// This error occurs when a source type (e.g., [`RawText`](crate::IngestSource::RawText),
    /// [`File`](crate::IngestSource::File)) requires a payload but `None` was provided.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ingest::{ingest, IngestConfig, IngestError};
    /// use ingest::{RawIngestRecord, IngestMetadata, IngestSource};
    ///
    /// let record = RawIngestRecord {
    ///     id: "test".to_string(),
    ///     source: IngestSource::RawText, // Requires payload
    ///     metadata: IngestMetadata {
    ///         tenant_id: Some("t".to_string()),
    ///         doc_id: Some("d".to_string()),
    ///         received_at: None,
    ///         original_source: None,
    ///         attributes: None,
    ///     },
    ///     payload: None, // ERROR: Required but missing
    /// };
    ///
    /// // This will fail with MissingPayload
    /// // let result = ingest(record, &IngestConfig::default());
    /// ```
    #[error("missing payload for source that requires payload")]
    MissingPayload,

    /// Binary payload is empty (zero bytes).
    ///
    /// This error occurs when [`IngestPayload::Binary`](crate::IngestPayload::Binary)
    /// contains an empty vector. Empty binary payloads are rejected to prevent
    /// meaningless ingests.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ingest::IngestPayload;
    ///
    /// // This will be rejected
    /// let empty_binary = IngestPayload::Binary(vec![]);
    /// ```
    #[error("binary payload is empty")]
    EmptyBinaryPayload,

    /// Invalid metadata or policy violation.
    ///
    /// This is a catch-all error for metadata validation failures:
    /// - Required field missing (per [`MetadataPolicy`](crate::MetadataPolicy))
    /// - Attributes exceed size limit
    /// - Future timestamp (when [`reject_future_timestamps`](crate::MetadataPolicy::reject_future_timestamps) is enabled)
    /// - Empty required field after sanitization
    /// - Invalid source/payload combination
    ///
    /// The message provides details about the specific violation.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ingest::IngestError;
    ///
    /// let err = IngestError::InvalidMetadata(
    ///     "tenant_id is required by ingest policy".to_string()
    /// );
    /// ```
    #[error("invalid metadata: {0}")]
    InvalidMetadata(String),

    /// Invalid UTF-8 in TextBytes payload.
    ///
    /// This error occurs when [`IngestPayload::TextBytes`](crate::IngestPayload::TextBytes)
    /// contains bytes that cannot be decoded as valid UTF-8.
    ///
    /// # Solutions
    ///
    /// - Use [`IngestPayload::Binary`](crate::IngestPayload::Binary) for non-text data
    /// - Validate encoding before ingest
    /// - Use encoding detection libraries (e.g., `chardetng`)
    /// - Use `String::from_utf8_lossy` and convert to `Text`
    ///
    /// # Example
    ///
    /// ```rust
    /// use ingest::{IngestPayload, ingest, IngestConfig, IngestError};
    /// use ingest::{RawIngestRecord, IngestMetadata, IngestSource};
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
    #[error("invalid utf-8 payload: {0}")]
    InvalidUtf8(String),

    /// Text payload became empty after normalization.
    ///
    /// This error occurs when [`normalize_payload()`](crate::normalize_payload) produces
    /// an empty string, typically because:
    ///
    /// - Input was whitespace-only (e.g., `"   "`, `"\n\n"`)
    /// - Input contained only control characters (which were stripped)
    /// - Input was empty string
    ///
    /// # Solutions
    ///
    /// - Check input before ingest: `if content.trim().is_empty()`
    /// - Provide meaningful error messages to users
    /// - Consider rejecting at API layer before calling ingest
    ///
    /// # Example
    ///
    /// ```rust
    /// use ingest::{ingest, IngestConfig, IngestError, IngestPayload};
    /// use ingest::{RawIngestRecord, IngestMetadata, IngestSource};
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
    ///     payload: Some(IngestPayload::Text("   \n\t   ".to_string())), // Whitespace only
    /// };
    ///
    /// // This will fail with EmptyNormalizedText
    /// // let result = ingest(record, &IngestConfig::default());
    /// ```
    #[error("text payload empty after normalization")]
    EmptyNormalizedText,

    /// Payload exceeds configured size limit.
    ///
    /// This error occurs when a payload violates:
    /// - [`IngestConfig::max_payload_bytes`](crate::IngestConfig::max_payload_bytes) (raw size)
    /// - [`IngestConfig::max_normalized_bytes`](crate::IngestConfig::max_normalized_bytes) (after normalization)
    ///
    /// The message contains details about which limit was exceeded and by how much.
    ///
    /// # HTTP Status Code
    ///
    /// This error should map to **413 Payload Too Large** in HTTP contexts.
    ///
    /// # Solutions
    ///
    /// - Increase limits if appropriate
    /// - Reject at API layer before calling ingest
    /// - Implement chunked processing for large documents
    ///
    /// # Example
    ///
    /// ```rust
    /// use ingest::{IngestConfig, IngestError};
    ///
    /// let err = IngestError::PayloadTooLarge(
    ///     "raw payload size 15000000 exceeds limit of 10000000".to_string()
    /// );
    ///
    /// // Map to HTTP status
    /// let status = match err {
    ///     IngestError::PayloadTooLarge(_) => 413,
    ///     _ => 400,
    /// };
    /// ```
    #[error("payload exceeds size limit: {0}")]
    PayloadTooLarge(String),
}

impl IngestError {
    /// Returns true if this error indicates a client-side issue.
    ///
    /// All ingest errors are client-side issues (invalid input), so this
    /// always returns true. It is provided for API consistency with other
    /// error types that might have server-side variants.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ingest::IngestError;
    ///
    /// let err = IngestError::MissingPayload;
    /// assert!(err.is_client_error());
    /// ```
    pub fn is_client_error(&self) -> bool {
        true
    }

    /// Returns a suggested HTTP status code for this error.
    ///
    /// This is a convenience method for HTTP API implementations.
    ///
    /// # Status Codes
    ///
    /// - `PayloadTooLarge`: 413
    /// - All others: 400
    ///
    /// # Example
    ///
    /// ```rust
    /// use ingest::IngestError;
    ///
    /// let err = IngestError::PayloadTooLarge("too big".to_string());
    /// assert_eq!(err.http_status_code(), 413);
    ///
    /// let err = IngestError::MissingPayload;
    /// assert_eq!(err.http_status_code(), 400);
    /// ```
    pub fn http_status_code(&self) -> u16 {
        match self {
            IngestError::PayloadTooLarge(_) => 413,
            _ => 400,
        }
    }
}
