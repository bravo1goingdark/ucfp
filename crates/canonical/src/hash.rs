//! Hashing utilities for the canonical text pipeline.
//!
//! This module provides SHA-256 hashing functions for:
//! - Document identity hashes (version-aware)
//! - Token-level hashes (version-aware)
//! - Simple text hashing (version-agnostic)
//!
//! # Hash Algorithms
//!
//! ## Document Identity Hash
//!
//! ```text
//! SHA-256(version.to_be_bytes() || 0x00 || canonical_text_bytes)
//! ```
//!
//! The discriminator byte `0x00` distinguishes document hashes from token hashes.
//!
//! ## Token Hash
//!
//! ```text
//! SHA-256(version.to_be_bytes() || 0x01 || token_text_bytes)
//! ```
//!
//! The discriminator byte `0x01` distinguishes token hashes from document hashes.
//!
//! # Version Inclusion
//!
//! All canonical hashes include the configuration version to ensure that
//! different canonicalization versions produce different hashes even for
//! the same input text. This prevents silent data corruption when upgrading
//! canonicalization logic.
//!
//! # Examples
//!
//! ```rust
//! use canonical::{hash_text, hash_canonical_bytes};
//!
//! // Simple text hash (version-agnostic)
//! let hash = hash_text("hello world");
//! assert_eq!(hash.len(), 64); // 256 bits as hex
//!
//! // Version-aware canonical hash
//! let canonical_hash = hash_canonical_bytes(1, b"hello world");
//! assert_eq!(canonical_hash.len(), 64);
//! ```

use fxhash::FxHasher64;
use sha2::{Digest, Sha256};
use std::hash::Hasher;

/// Compute a fast 64-bit hash for an individual token under a given canonical
/// configuration version.
///
/// This is used for token-level hashes in perceptual fingerprinting.
/// It's much faster than SHA-256 but provides a 64-bit hash which is sufficient
/// for non-cryptographic purposes like MinHash.
///
/// # Algorithm
///
/// ```text
/// FxHash64(version.to_le_bytes() || 0x01 || token_text_bytes)
/// ```
///
/// - `version.to_le_bytes()`: 4-byte little-endian version number  
/// - `0x01`: Discriminator byte (token level)
/// - `token_text_bytes`: UTF-8 bytes of token text
///
/// # Returns
///
/// A 16-character hexadecimal string representing the 64-bit FxHash digest.
pub fn hash_token_bytes_fast(canonical_version: u32, token_bytes: &[u8]) -> String {
    let mut hasher = FxHasher64::default();
    hasher.write(&canonical_version.to_le_bytes());
    hasher.write(&[1]); // discriminator
    hasher.write(token_bytes);
    format!("{:016x}", hasher.finish())
}

/// Hash arbitrary text with SHA-256 and return a hex digest.
///
/// This is a general-purpose hashing function suitable for diagnostics,
/// quick hashes, and non-canonical use cases. It does **not** include
/// version information.
///
/// For canonical identity hashes, use [`hash_canonical_bytes`] instead.
///
/// # Algorithm
///
/// ```text
/// SHA-256(text_bytes) → hex string
/// ```
///
/// # Returns
///
/// A 64-character hexadecimal string representing the SHA-256 digest.
///
/// # Examples
///
/// ```rust
/// use canonical::hash_text;
///
/// let hash = hash_text("hello world");
/// assert_eq!(hash.len(), 64);
///
/// // Deterministic
/// let hash2 = hash_text("hello world");
/// assert_eq!(hash, hash2);
///
/// // Different inputs produce different hashes
/// let hash3 = hash_text("hello world!");
/// assert_ne!(hash, hash3);
/// ```
///
/// # Use Cases
///
/// - Diagnostics and logging
/// - Quick content verification
/// - Non-canonical hashing needs
/// - Testing
///
/// # When Not to Use
///
/// Do **not** use this for:
/// - Canonical document identity (use [`hash_canonical_bytes`])
/// - Token-level hashing in pipelines (handled internally)
pub fn hash_text(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hex::encode(hasher.finalize())
}

/// Compute the canonical identity hash for canonical text and version.
///
/// This is the primary hash for identifying canonical documents. It includes
/// the configuration version to ensure different versions produce different
/// hashes.
///
/// # Algorithm
///
/// ```text
/// SHA-256(version.to_be_bytes() || 0x00 || canonical_text_bytes)
/// ```
///
/// - `version.to_be_bytes()`: 4-byte big-endian version number
/// - `0x00`: Discriminator byte (document level)
/// - `canonical_text_bytes`: UTF-8 bytes of canonical text
///
/// # Arguments
///
/// * `canonical_version` - The configuration version (from `CanonicalizeConfig`)
/// * `canonical_bytes` - The canonical text as UTF-8 bytes
///
/// # Returns
///
/// A 64-character hexadecimal string representing the SHA-256 digest.
///
/// # Examples
///
/// ```rust
/// use canonical::hash_canonical_bytes;
///
/// let hash_v1 = hash_canonical_bytes(1, b"hello world");
/// let hash_v2 = hash_canonical_bytes(2, b"hello world");
///
/// // Same text, different versions = different hashes
/// assert_ne!(hash_v1, hash_v2);
///
/// // Same version and text = same hash
/// let hash_v1_again = hash_canonical_bytes(1, b"hello world");
/// assert_eq!(hash_v1, hash_v1_again);
/// ```
///
/// # Use Cases
///
/// - Document identity and deduplication
/// - Content addressing
/// - Version-aware comparison
/// - Canonical document storage
///
/// # See Also
///
/// - [`hash_canonical_bytes`] for document-level hashing
/// - [`hash_text`] for simple text hashing
pub fn hash_canonical_bytes(canonical_version: u32, canonical_bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(canonical_version.to_be_bytes());
    hasher.update([0]);
    hasher.update(canonical_bytes);
    hex::encode(hasher.finalize())
}
