use sha2::{Digest, Sha256};

/// Hash arbitrary text bytes with SHA-256 and return a hex digest.
///
/// This helper is intentionally version-agnostic and is suitable for
/// non-canonical uses (e.g., diagnostics). The canonical identity hash used
/// by `CanonicalizedDocument` also incorporates the canonical version.
pub fn hash_text(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hex::encode(hasher.finalize())
}

/// Compute the canonical identity hash for canonical text and version.
///
/// The hash is defined as:
/// `SHA-256( canonical_version.to_be_bytes() || 0x00 || canonical_text_bytes )`.
pub fn hash_canonical_bytes(canonical_version: u32, canonical_bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(canonical_version.to_be_bytes());
    hasher.update([0]);
    hasher.update(canonical_bytes);
    hex::encode(hasher.finalize())
}

/// Compute a stable hash for an individual token under a given canonical
/// configuration version.
///
/// This uses a distinct domain separator from the document-level hash:
/// `SHA-256( canonical_version.to_be_bytes() || 0x01 || token_text_bytes )`.
pub fn hash_token_bytes(canonical_version: u32, token_bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(canonical_version.to_be_bytes());
    hasher.update([1]);
    hasher.update(token_bytes);
    hex::encode(hasher.finalize())
}
