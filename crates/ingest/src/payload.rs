//! Payload validation and normalization utilities.
//!
//! This module contains helpers for enforcing payload presence/shape policies
//! and transforming raw payloads into [`CanonicalPayload`] values suitable for
//! downstream processing.
//!
//! # Responsibilities
//!
//! - **Requirement Validation**: Check if source type mandates a payload
//! - **Type Validation**: Ensure text sources get text payloads
//! - **Content Validation**: UTF-8 validation, emptiness checks
//! - **Normalization**: Whitespace collapsing for text
//! - **Size Enforcement**: Apply payload size limits
//!
//! # Payload Flow
//!
//! ```text
//! IngestPayload (raw)
//!        │
//!        ▼
//! ┌─────────────────────────────┐
//! │ 1. Check requirements       │
//! │    - Source mandates?       │
//! │    - Text source = text?    │
//! ├─────────────────────────────┤
//! │ 2. Decode/validate          │
//! │    - UTF-8 validation       │
//! │    - Binary checks          │
//! ├─────────────────────────────┤
//! │ 3. Normalize                │
//! │    - Collapse whitespace    │
//! │    - Size limits            │
//! └─────────────────────────────┘
//!        │
//!        ▼
//! CanonicalPayload (normalized)
//! ```
//!
//! # Examples
//!
//! ```rust
//! use ingest::{
//!     validate_payload_requirements, normalize_payload_option,
//!     IngestPayload, IngestSource, IngestConfig
//! };
//!
//! let source = IngestSource::RawText;
//! let payload = Some(IngestPayload::Text("Hello world".to_string()));
//!
//! // Validate requirements
//! validate_payload_requirements(&source, &payload).unwrap();
//!
//! // Normalize
//! let config = IngestConfig::default();
//! let canonical = normalize_payload_option(&source, payload, &config).unwrap();
//! ```
use crate::config::IngestConfig;
use crate::error::IngestError;
use crate::types::{CanonicalPayload, IngestPayload, IngestSource};

/// Checks if the source requires a payload.
///
/// This function validates that sources which mandate payloads (like `RawText`
/// and `File`) actually have one provided. This is an early validation step
/// before any processing occurs.
///
/// # Source Requirements
///
/// | Source | Payload Required |
/// |--------|-----------------|
/// | `RawText` | Yes |
/// | `Url` | No (but typically has one) |
/// | `File` | Yes |
/// | `Api` | No |
///
/// # Arguments
///
/// * `source` - The ingest source type
/// * `payload` - The optional payload
///
/// # Returns
///
/// - `Ok(())` - Payload requirements satisfied
/// - `Err(IngestError::MissingPayload)` - Required payload is missing
///
/// # Examples
///
/// ```rust
/// use ingest::{validate_payload_requirements, IngestPayload, IngestSource};
///
/// // RawText requires payload
/// let source = IngestSource::RawText;
/// let result = validate_payload_requirements(&source, &Some(IngestPayload::Text("test".to_string())));
/// assert!(result.is_ok());
///
/// // Missing required payload
/// let result = validate_payload_requirements(&source, &None);
/// assert!(result.is_err());
///
/// // Api doesn't require payload
/// let source = IngestSource::Api;
/// let result = validate_payload_requirements(&source, &None);
/// assert!(result.is_ok());
/// ```
pub fn validate_payload_requirements(
    source: &IngestSource,
    payload: &Option<IngestPayload>,
) -> Result<(), IngestError> {
    let has_payload = payload.is_some();
    if source_requires_payload(source) && !has_payload {
        return Err(IngestError::MissingPayload);
    }
    Ok(())
}

/// Determines if a source type requires a payload.
///
/// This internal function defines which source types mandate payload presence.
///
/// # Arguments
///
/// * `source` - The ingest source type
///
/// # Returns
///
/// `true` if the source requires a payload, `false` otherwise
fn source_requires_payload(source: &IngestSource) -> bool {
    matches!(source, IngestSource::RawText | IngestSource::File { .. })
}

/// Determines if a source type requires a text payload.
///
/// Text-based sources (like `RawText` and `Url`) should receive text payloads
/// rather than binary data.
///
/// # Arguments
///
/// * `source` - The ingest source type
///
/// # Returns
///
/// `true` if the source requires text content, `false` otherwise
fn source_requires_text_payload(source: &IngestSource) -> bool {
    matches!(source, IngestSource::RawText | IngestSource::Url(_))
}

/// Normalizes the payload based on its type.
///
/// This is the main entry point for payload processing. It:
/// 1. Handles `None` payloads (returns `None`)
/// 2. Normalizes the payload value
/// 3. Validates text source requirements
/// 4. Returns the canonical payload
///
/// # Arguments
///
/// * `source` - The ingest source (for type validation)
/// * `payload` - The optional raw payload
/// * `cfg` - Configuration for normalization
///
/// # Returns
///
/// - `Ok(Some(CanonicalPayload))` - Successfully normalized payload
/// - `Ok(None)` - No payload provided
/// - `Err(IngestError)` - Validation or normalization failure
///
/// # Errors
///
/// - [`IngestError::InvalidMetadata`] - Text source received binary payload
/// - All errors from `normalize_payload_value`
///
/// # Examples
///
/// ```rust,ignore
/// use ingest::{normalize_payload_option, IngestPayload, IngestSource, IngestConfig};
///
/// let config = IngestConfig::default();
///
/// // Text normalization
/// let result = normalize_payload_option(
///     &IngestSource::RawText,
///     Some(IngestPayload::Text("  Hello   world  ".to_string())),
///     &config
/// ).unwrap();
///
/// // Binary preservation
/// let result = normalize_payload_option(
///     &IngestSource::File { filename: "test.bin".to_string(), content_type: None },
///     Some(IngestPayload::Binary(vec![1, 2, 3])),
///     &config
/// ).unwrap();
/// ```
pub fn normalize_payload_option(
    source: &IngestSource,
    payload: Option<IngestPayload>,
    cfg: &IngestConfig,
) -> Result<Option<CanonicalPayload>, IngestError> {
    let payload = match payload {
        Some(value) => value,
        None => return Ok(None),
    };

    let canonical = normalize_payload_value(payload, cfg)?;
    // Some sources only make sense with a text payload.
    if source_requires_text_payload(source) && !matches!(canonical, CanonicalPayload::Text(_)) {
        return Err(IngestError::InvalidMetadata(
            "text-based source requires text payload".into(),
        ));
    }
    Ok(Some(canonical))
}

/// Normalizes the payload value itself.
///
/// This function processes the actual payload content based on its type:
/// - `Text`: Validates and normalizes whitespace
/// - `TextBytes`: Validates UTF-8, then treats as Text
/// - `Binary`: Validates non-empty, passes through unchanged
///
/// # Arguments
///
/// * `payload` - The raw payload value
/// * `cfg` - Configuration for normalization
///
/// # Returns
///
/// - `Ok(CanonicalPayload)` - Successfully normalized payload
/// - `Err(IngestError)` - Validation or normalization failure
///
/// # Errors
///
/// - [`IngestError::InvalidUtf8`] - TextBytes contains invalid UTF-8
/// - [`IngestError::EmptyBinaryPayload`] - Binary payload is empty
/// - [`IngestError::InvalidMetadata`] - Binary contains suspicious patterns
/// - [`IngestError::PayloadTooLarge`] - Size limit exceeded
/// - [`IngestError::EmptyNormalizedText`] - Text empty after normalization
fn normalize_payload_value(
    payload: IngestPayload,
    cfg: &IngestConfig,
) -> Result<CanonicalPayload, IngestError> {
    match payload {
        IngestPayload::Text(text) => normalize_text_payload(text, cfg),
        IngestPayload::TextBytes(bytes) => {
            let text = String::from_utf8(bytes)
                .map_err(|err| IngestError::InvalidUtf8(err.to_string()))?;
            normalize_text_payload(text, cfg)
        }
        IngestPayload::Binary(bytes) => {
            if bytes.is_empty() {
                Err(IngestError::EmptyBinaryPayload)
            } else {
                // Scan for suspicious patterns in binary data
                if bytes.len() > 1024 {
                    let suspicious_patterns = [b'\x00', b'\xFF', b'\xFE'];
                    let pattern_count = bytes
                        .iter()
                        .filter(|&&b| suspicious_patterns.contains(&b))
                        .count();
                    if pattern_count > bytes.len() / 4 {
                        return Err(IngestError::InvalidMetadata(
                            "binary payload contains suspicious patterns".into(),
                        ));
                    }
                }

                Ok(CanonicalPayload::Binary(bytes))
            }
        }
    }
}

/// Validates text content for potential issues before normalization.
///
/// This function performs sanity checks on text content:
/// - Null byte detection
/// - Excessive control characters
/// - Empty content check
///
/// # Arguments
///
/// * `text` - The text to validate
/// * `cfg` - Configuration (controls control character checking)
///
/// # Returns
///
/// - `Ok(())` - Text is valid
/// - `Err(IngestError)` - Validation failure
///
/// # Errors
///
/// - [`IngestError::InvalidMetadata`] - Null bytes or too many control characters
/// - [`IngestError::EmptyNormalizedText`] - Text is empty/whitespace only
fn validate_text_content(text: &str, cfg: &IngestConfig) -> Result<(), IngestError> {
    // Check for null bytes
    if text.contains('\0') {
        return Err(IngestError::InvalidMetadata(
            "text contains null bytes".into(),
        ));
    }

    // Check for excessive control characters
    // Use ASCII fast path for better performance on common ASCII text
    let control_count = if text.is_ascii() {
        text.bytes()
            .filter(|&b| b < 32 && !matches!(b, b'\t' | b'\n' | b'\r'))
            .count()
    } else {
        text.chars()
            .filter(|c| c.is_control() && *c != '\t' && *c != '\n' && *c != '\r')
            .count()
    };
    if cfg.strip_control_chars && control_count > text.len() / 10 {
        return Err(IngestError::InvalidMetadata(
            "text contains too many control characters".into(),
        ));
    }

    // Check minimum content length
    if text.trim().is_empty() {
        return Err(IngestError::EmptyNormalizedText);
    }

    Ok(())
}

/// Normalizes a text payload by collapsing whitespace.
///
/// This function performs the full text normalization pipeline:
/// 1. Validates content (null bytes, control chars, emptiness)
/// 2. Collapses whitespace using [`normalize_payload`](crate::normalize_payload)
/// 3. Enforces size limits
/// 4. Checks for empty result
///
/// # Arguments
///
/// * `text` - The raw text to normalize
/// * `cfg` - Configuration for normalization and size limits
///
/// # Returns
///
/// - `Ok(CanonicalPayload::Text)` - Successfully normalized text
/// - `Err(IngestError)` - Validation or normalization failure
///
/// # Errors
///
/// - All errors from [`validate_text_content`]
/// - [`IngestError::PayloadTooLarge`] - Normalized text exceeds limit
/// - [`IngestError::EmptyNormalizedText`] - Result is empty
fn normalize_text_payload(
    text: String,
    cfg: &IngestConfig,
) -> Result<CanonicalPayload, IngestError> {
    // Validate content first
    validate_text_content(&text, cfg)?;

    let normalized = crate::normalize_payload(&text);
    if let Some(limit) = cfg.max_normalized_bytes {
        if normalized.len() > limit {
            return Err(IngestError::PayloadTooLarge(format!(
                "normalized payload size {} exceeds limit of {}",
                normalized.len(),
                limit
            )));
        }
    }

    if normalized.is_empty() {
        Err(IngestError::EmptyNormalizedText)
    } else {
        Ok(CanonicalPayload::Text(normalized))
    }
}

/// Returns a string representation of the payload kind for logging.
///
/// This is a utility function for structured logging to categorize payloads
/// without exposing actual content.
///
/// # Arguments
///
/// * `payload` - Optional reference to canonical payload
///
/// # Returns
///
/// String describing the payload type: `"text"`, `"binary"`, or `"none"`
///
/// # Examples
///
/// ```rust
/// use ingest::{payload_kind, CanonicalPayload};
///
/// let text = Some(CanonicalPayload::Text("hello".to_string()));
/// assert_eq!(payload_kind(text.as_ref()), "text");
///
/// let binary = Some(CanonicalPayload::Binary(vec![1, 2, 3]));
/// assert_eq!(payload_kind(binary.as_ref()), "binary");
///
/// assert_eq!(payload_kind(None), "none");
/// ```
pub fn payload_kind(payload: Option<&CanonicalPayload>) -> &'static str {
    match payload {
        Some(CanonicalPayload::Text(_)) => "text",
        Some(CanonicalPayload::Binary(_)) => "binary",
        None => "none",
    }
}

/// Returns the length of the payload for logging.
///
/// This is a utility function for structured logging to record payload sizes
/// without exposing actual content.
///
/// # Arguments
///
/// * `payload` - Optional reference to canonical payload
///
/// # Returns
///
/// Size in bytes, or 0 if no payload
///
/// # Examples
///
/// ```rust
/// use ingest::{payload_length, CanonicalPayload};
///
/// let text = Some(CanonicalPayload::Text("hello".to_string()));
/// assert_eq!(payload_length(text.as_ref()), 5);
///
/// let binary = Some(CanonicalPayload::Binary(vec![1, 2, 3, 4]));
/// assert_eq!(payload_length(binary.as_ref()), 4);
///
/// assert_eq!(payload_length(None), 0);
/// ```
pub fn payload_length(payload: Option<&CanonicalPayload>) -> usize {
    match payload {
        Some(CanonicalPayload::Text(text)) => text.len(),
        Some(CanonicalPayload::Binary(bytes)) => bytes.len(),
        None => 0,
    }
}
