//! Workspace umbrella crate for Universal Content Fingerprinting (UCFP).
//!
//! This crate stitches together ingest normalization and canonicalization so
//! callers can operate over text or binary payloads with a single API.

pub use ufp_canonical::{CanonicalizeConfig, CanonicalizedDocument, Token, canonicalize};
pub use ufp_ingest::{
    CanonicalIngestRecord, CanonicalPayload, IngestError, IngestMetadata, IngestPayload,
    IngestRequest, IngestSource, ingest,
};

use sha2::{Digest, Sha256};

/// Canonical content emitted by the pipeline.
#[derive(Debug, Clone)]
pub enum CanonicalContent {
    Text(CanonicalizedDocument),
    Binary(BinaryCanonical),
    None,
}

/// Canonical metadata for binary payloads.
#[derive(Debug, Clone)]
pub struct BinaryCanonical {
    pub bytes: Vec<u8>,
    pub sha256_hex: String,
}

/// Canonical ingest record paired with the canonicalized payload.
#[derive(Debug, Clone)]
pub struct CanonicalizedIngest {
    pub record: CanonicalIngestRecord,
    pub content: CanonicalContent,
}

/// Canonicalize an ingest record, producing canonical content for text or binary payloads.
pub fn canonicalize_ingest_record(
    mut record: CanonicalIngestRecord,
    cfg: &CanonicalizeConfig,
) -> CanonicalizedIngest {
    let content = match record.normalized_payload.take() {
        Some(CanonicalPayload::Text(text)) => {
            let doc = canonicalize(&text, cfg);
            CanonicalContent::Text(doc)
        }
        Some(CanonicalPayload::Binary(bytes)) => {
            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            let sha256_hex = hex::encode(hasher.finalize());
            CanonicalContent::Binary(BinaryCanonical { bytes, sha256_hex })
        }
        None => CanonicalContent::None,
    };

    CanonicalizedIngest { record, content }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn canonicalize_text_payload_produces_tokens() {
        let record = CanonicalIngestRecord {
            id: "id".into(),
            tenant_id: "tenant".into(),
            doc_id: "doc".into(),
            received_at: Utc::now(),
            original_source: None,
            source: IngestSource::RawText,
            normalized_payload: Some(CanonicalPayload::Text(" Hello   Rust ".into())),
            attributes: None,
        };
        let cfg = CanonicalizeConfig::default();
        let result = canonicalize_ingest_record(record, &cfg);

        match result.content {
            CanonicalContent::Text(doc) => {
                assert_eq!(doc.canonical_text, "hello rust");
                assert_eq!(doc.tokens.len(), 2);
            }
            _ => panic!("expected text canonical content"),
        }
    }

    #[test]
    fn canonicalize_binary_payload_returns_hash() {
        use sha2::{Digest, Sha256};

        let record = CanonicalIngestRecord {
            id: "id".into(),
            tenant_id: "tenant".into(),
            doc_id: "doc".into(),
            received_at: Utc::now(),
            original_source: None,
            source: IngestSource::File {
                filename: "file.bin".into(),
                content_type: None,
            },
            normalized_payload: Some(CanonicalPayload::Binary(vec![0, 1, 2, 3])),
            attributes: None,
        };
        let cfg = CanonicalizeConfig::default();
        let result = canonicalize_ingest_record(record, &cfg);

        match result.content {
            CanonicalContent::Binary(binary) => {
                assert_eq!(binary.bytes, vec![0, 1, 2, 3]);
                let mut hasher = Sha256::new();
                hasher.update([0, 1, 2, 3]);
                let expected = hex::encode(hasher.finalize());
                assert_eq!(binary.sha256_hex, expected);
            }
            _ => panic!("expected binary canonical content"),
        }
    }

    #[test]
    fn canonicalize_missing_payload_returns_none() {
        let record = CanonicalIngestRecord {
            id: "id".into(),
            tenant_id: "tenant".into(),
            doc_id: "doc".into(),
            received_at: Utc::now(),
            original_source: None,
            source: IngestSource::Url("https://example.com".into()),
            normalized_payload: None,
            attributes: None,
        };
        let cfg = CanonicalizeConfig::default();
        let result = canonicalize_ingest_record(record, &cfg);

        assert!(matches!(result.content, CanonicalContent::None));
    }
}
