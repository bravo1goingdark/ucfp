use serde::{Deserialize, Serialize};

use crate::config::CanonicalizeConfig;
use crate::token::Token;

/// Canonical representation of a text document.
///
/// For a fixed `CanonicalizeConfig`, the same input text must always produce
/// the same `CanonicalizedDocument` on any machine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CanonicalizedDocument {
    /// Application-level document identifier.
    pub doc_id: String,
    /// Canonical text after normalization, casing, and whitespace policies.
    pub canonical_text: String,
    /// Token stream with UTF-8 byte offsets into `canonical_text`.
    pub tokens: Vec<Token>,
    /// Stable per-token hashes aligned with `tokens`.
    pub token_hashes: Vec<String>,
    /// Canonical identity hash (version-aware) for this document.
    pub sha256_hex: String,
    /// Canonical configuration version used to produce this document.
    pub canonical_version: u32,
    /// Snapshot of the canonicalization configuration (normalization/profile).
    pub config: CanonicalizeConfig,
}
