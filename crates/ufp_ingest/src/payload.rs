//! Payload validation and normalization utilities.
//!
//! This module contains helpers for enforcing payload presence/shape policies
//! and transforming raw payloads into [`CanonicalPayload`](crate::CanonicalPayload)
//! values suitable for downstream processing.
use crate::config::IngestConfig;
use crate::error::IngestError;
use crate::types::{CanonicalPayload, IngestPayload, IngestSource};

/// Checks if the source requires a payload.
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
pub(crate) fn source_requires_payload(source: &IngestSource) -> bool {
    matches!(source, IngestSource::RawText | IngestSource::File { .. })
}

/// Normalizes the payload based on its type.
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

/// Determines if a source type requires a text payload.
pub(crate) fn source_requires_text_payload(source: &IngestSource) -> bool {
    matches!(source, IngestSource::RawText | IngestSource::Url(_))
}

/// Normalizes the payload value itself.
pub(crate) fn normalize_payload_value(
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
                Ok(CanonicalPayload::Binary(bytes))
            }
        }
    }
}

/// Normalizes a text payload by collapsing whitespace.
pub(crate) fn normalize_text_payload(
    text: String,
    cfg: &IngestConfig,
) -> Result<CanonicalPayload, IngestError> {
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
pub fn payload_kind(payload: Option<&CanonicalPayload>) -> &'static str {
    match payload {
        Some(CanonicalPayload::Text(_)) => "text",
        Some(CanonicalPayload::Binary(_)) => "binary",
        None => "none",
    }
}

/// Returns the length of the payload for logging.
pub fn payload_length(payload: Option<&CanonicalPayload>) -> usize {
    match payload {
        Some(CanonicalPayload::Text(text)) => text.len(),
        Some(CanonicalPayload::Binary(bytes)) => bytes.len(),
        None => 0,
    }
}
