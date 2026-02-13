//! Document types for the canonical text pipeline.
//!
//! This module defines [`CanonicalizedDocument`] and related types that represent
//! the output of the canonicalization process.
//!
//! # Document Structure
//!
//! A `CanonicalizedDocument` contains:
//! - The canonical text (normalized, transformed)
//! - Tokens with byte offsets
//! - Version-aware hashes
//! - Configuration snapshot
//!
//! # Determinism
//!
//! For a fixed configuration version and input text, all fields of
//! `CanonicalizedDocument` are deterministic:
//! - Same `canonical_text`
//! - Same `tokens` (content and offsets)
//! - Same `sha256_hex` identity hash
//! - Same `token_hashes`
//!
//! # Examples
//!
//! ```rust
//! use canonical::{canonicalize, CanonicalizeConfig};
//!
//! let config = CanonicalizeConfig::default();
//! let doc = canonicalize("doc-001", "Hello world", &config).unwrap();
//!
//! // Access canonical text
//! assert_eq!(doc.canonical_text, "hello world");
//!
//! // Access tokens
//! assert_eq!(doc.tokens.len(), 2);
//! assert_eq!(doc.tokens[0].text, "hello");
//!
//! // Access identity hash
//! assert!(!doc.sha256_hex.is_empty());
//!
//! // Access configuration used
//! assert_eq!(doc.config.version, 1);
//! ```

use serde::{Deserialize, Serialize};

use crate::config::CanonicalizeConfig;
use crate::token::Token;

/// The canonical representation of a text document.
///
/// `CanonicalizedDocument` is the output of the canonicalization pipeline.
/// It contains the normalized text, tokens, hashes, and a snapshot of the
/// configuration used to produce it.
///
/// # Determinism Guarantee
///
/// For a fixed [`CanonicalizeConfig`] version and input text, the same
/// `CanonicalizedDocument` is produced on any machine, at any time.
///
/// # Structure
///
/// ```text
/// CanonicalizedDocument
/// ├── doc_id: String                    # Application document identifier
/// ├── canonical_text: String            # Normalized, transformed text
/// ├── tokens: Vec<Token>                # Tokens with byte offsets
/// ├── token_hashes: Vec<String>         # Per-token stable hashes
/// ├── sha256_hex: String                # Document identity hash
/// ├── canonical_version: u32            # Config version used
/// └── config: CanonicalizeConfig        # Config snapshot
/// ```
///
/// # Examples
///
/// ## Basic Usage
///
/// ```rust
/// use canonical::{canonicalize, CanonicalizeConfig};
///
/// let config = CanonicalizeConfig::default();
/// let doc = canonicalize("doc-001", "Hello world", &config).unwrap();
///
/// assert_eq!(doc.doc_id, "doc-001");
/// assert_eq!(doc.canonical_text, "hello world");
/// assert_eq!(doc.tokens.len(), 2);
/// assert!(!doc.sha256_hex.is_empty());
/// ```
///
/// ## Accessing Tokens
///
/// ```rust
/// use canonical::{canonicalize, CanonicalizeConfig};
///
/// let config = CanonicalizeConfig::default();
/// let doc = canonicalize("doc-001", "Hello world", &config).unwrap();
///
/// for token in &doc.tokens {
///     println!("Token '{}' at bytes [{}..{}]", token.text, token.start, token.end);
/// }
/// ```
///
/// ## Version-Aware Comparison
///
/// ```rust
/// use canonical::{canonicalize, CanonicalizeConfig};
///
/// let cfg_v1 = CanonicalizeConfig { version: 1, ..Default::default() };
/// let cfg_v2 = CanonicalizeConfig { version: 2, ..Default::default() };
///
/// let doc_v1 = canonicalize("doc", "hello", &cfg_v1).unwrap();
/// let doc_v2 = canonicalize("doc", "hello", &cfg_v2).unwrap();
///
/// // Same text but different versions have different hashes
/// assert_ne!(doc_v1.sha256_hex, doc_v2.sha256_hex);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CanonicalizedDocument {
    /// Application-level document identifier.
    ///
    /// This is copied from the `doc_id` parameter passed to [`canonicalize()`](crate::canonicalize).
    /// It serves as a traceable identifier for the document in downstream systems.
    ///
    /// # Example
    ///
    /// ```rust
    /// use canonical::{canonicalize, CanonicalizeConfig};
    ///
    /// let config = CanonicalizeConfig::default();
    /// let doc = canonicalize("my-document-id", "Hello", &config).unwrap();
    /// assert_eq!(doc.doc_id, "my-document-id");
    /// ```
    pub doc_id: String,

    /// Canonical text after normalization, casing, and whitespace policies.
    ///
    /// This is the "source of truth" text that downstream stages should use.
    /// It has been processed according to the [`CanonicalizeConfig`]:
    /// - Unicode normalization (if enabled)
    /// - Case folding (if enabled)
    /// - Punctuation handling (if enabled)
    /// - Whitespace collapsing
    ///
    /// # Example
    ///
    /// ```rust
    /// use canonical::{canonicalize, CanonicalizeConfig};
    ///
    /// let config = CanonicalizeConfig::default();
    /// let doc = canonicalize("doc", "  Hello   WORLD  ", &config).unwrap();
    /// assert_eq!(doc.canonical_text, "hello world");
    /// ```
    pub canonical_text: String,

    /// Token stream with UTF-8 byte offsets into `canonical_text`.
    ///
    /// Each token represents a contiguous sequence of non-delimiter characters
    /// in the canonical text. The offsets are byte positions (not character
    /// positions) to handle multi-byte UTF-8 characters correctly.
    ///
    /// # Structure
    ///
    /// ```rust,ignore
    /// Token {
    ///     text: String,   // The token text
    ///     start: usize,   // Byte offset (inclusive)
    ///     end: usize,     // Byte offset (exclusive)
    /// }
    /// ```
    ///
    /// # Example
    ///
    /// ```rust
    /// use canonical::{canonicalize, CanonicalizeConfig};
    ///
    /// let config = CanonicalizeConfig::default();
    /// let doc = canonicalize("doc", "hello world", &config).unwrap();
    ///
    /// assert_eq!(doc.tokens.len(), 2);
    /// assert_eq!(doc.tokens[0].text, "hello");
    /// assert_eq!(doc.tokens[0].start, 0);
    /// assert_eq!(doc.tokens[0].end, 5);
    /// ```
    pub tokens: Vec<Token>,

    /// Stable per-token hashes aligned with `tokens`.
    ///
    /// Each hash is computed as:
    /// ```text
    /// SHA-256(version.to_be_bytes() || 0x01 || token_text_bytes)
    /// ```
    ///
    /// The discriminator byte `0x01` distinguishes token hashes from the
    /// document identity hash (which uses `0x00`).
    ///
    /// # Use Cases
    ///
    /// - Token-level deduplication
    /// - Perceptual fingerprinting (MinHash on token hashes)
    /// - Change detection at token granularity
    ///
    /// # Example
    ///
    /// ```rust
    /// use canonical::{canonicalize, CanonicalizeConfig};
    ///
    /// let config = CanonicalizeConfig::default();
    /// let doc = canonicalize("doc", "hello world", &config).unwrap();
    ///
    /// assert_eq!(doc.token_hashes.len(), doc.tokens.len());
    /// // Each token has a unique stable hash
    /// assert_ne!(doc.token_hashes[0], doc.token_hashes[1]);
    /// ```
    pub token_hashes: Vec<String>,

    /// Canonical identity hash (version-aware) for this document.
    ///
    /// This is the primary identifier for the canonical document. It is computed as:
    /// ```text
    /// SHA-256(version.to_be_bytes() || 0x00 || canonical_text_bytes)
    /// ```
    ///
    /// # Properties
    ///
    /// - **Stable**: Same input + same config version = same hash
    /// - **Version-aware**: Different versions produce different hashes
    /// - **Cross-platform**: Consistent across all systems
    /// - **Collision-resistant**: Uses SHA-256
    ///
    /// # Use Cases
    ///
    /// - Document deduplication
    /// - Content addressing
    /// - Cache keys
    /// - Audit trails
    ///
    /// # Example
    ///
    /// ```rust
    /// use canonical::{canonicalize, CanonicalizeConfig};
    ///
    /// let config = CanonicalizeConfig::default();
    /// let doc = canonicalize("doc", "hello world", &config).unwrap();
    ///
    /// // Hash is 64 hex characters (256 bits)
    /// assert_eq!(doc.sha256_hex.len(), 64);
    /// ```
    pub sha256_hex: String,

    /// Canonical configuration version used to produce this document.
    ///
    /// This is a copy of [`CanonicalizeConfig::version`] for convenience.
    /// It allows you to quickly check which version was used without
    /// accessing the full config snapshot.
    ///
    /// # Example
    ///
    /// ```rust
    /// use canonical::{canonicalize, CanonicalizeConfig};
    ///
    /// let config = CanonicalizeConfig::default();
    /// let doc = canonicalize("doc", "hello", &config).unwrap();
    ///
    /// assert_eq!(doc.canonical_version, config.version);
    /// assert_eq!(doc.canonical_version, doc.config.version);
    /// ```
    pub canonical_version: u32,

    /// Snapshot of the canonicalization configuration.
    ///
    /// This allows you to recreate the exact canonicalization if needed,
    /// or compare configurations between documents.
    ///
    /// # Use Cases
    ///
    /// - Configuration audit trails
    /// - Re-canonicalizing with same settings
    /// - Detecting configuration drift
    /// - Debugging canonicalization differences
    ///
    /// # Example
    ///
    /// ```rust
    /// use canonical::{canonicalize, CanonicalizeConfig};
    ///
    /// let config = CanonicalizeConfig::default();
    /// let doc = canonicalize("doc", "hello", &config).unwrap();
    ///
    /// // Verify the config used
    /// assert_eq!(doc.config.normalize_unicode, true);
    /// assert_eq!(doc.config.lowercase, true);
    /// ```
    pub config: CanonicalizeConfig,
}
