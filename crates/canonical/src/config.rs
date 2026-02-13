//! Configuration types for the canonical text pipeline.
//!
//! This module defines [`CanonicalizeConfig`], which controls how text is
//! normalized and transformed during the canonicalization process.
//!
//! # Versioning
//!
//! The `version` field is critical for maintaining determinism. Any change to
//! canonicalization behavior (even bug fixes) must be accompanied by a version
//! bump. This ensures that:
//!
//! - Old canonicalizations remain stable and reproducible
//! - New canonicalizations use updated behavior
//! - Hashes from different versions are distinct
//!
//! # Configuration Stability
//!
//! For a given `version`, the configuration is stable across:
//! - Different machines and architectures
//! - Different Rust versions (within reason)
//! - Different operating systems
//! - Different locales
//!
//! # Examples
//!
//! ## Default Configuration
//!
//! ```rust
//! use canonical::CanonicalizeConfig;
//!
//! let config = CanonicalizeConfig::default();
//! assert_eq!(config.version, 1);
//! assert!(config.normalize_unicode);
//! assert!(!config.strip_punctuation);
//! assert!(config.lowercase);
//! ```
//!
//! ## Preserving Original Case
//!
//! ```rust
//! use canonical::CanonicalizeConfig;
//!
//! let config = CanonicalizeConfig {
//!     lowercase: false,
//!     ..Default::default()
//! };
//! ```
//!
//! ## Stripping Punctuation
//!
//! ```rust
//! use canonical::CanonicalizeConfig;
//!
//! let config = CanonicalizeConfig {
//!     strip_punctuation: true,
//!     ..Default::default()
//! };
//! ```

use serde::{Deserialize, Serialize};

/// Configuration for the canonical text pipeline.
///
/// `CanonicalizeConfig` controls all aspects of text normalization and
/// transformation. It is designed to be cheap to clone and serializable
/// for configuration management.
///
/// # Fields
///
/// - `version`: Semantic version for tracking behavior changes
/// - `normalize_unicode`: Apply Unicode NFKC normalization
/// - `strip_punctuation`: Remove punctuation characters
/// - `lowercase`: Apply locale-free Unicode lowercasing
///
/// # Version Requirements
///
/// The `version` field must be >= 1. Version 0 is reserved and will be
/// rejected with [`CanonicalError::InvalidConfig`](crate::CanonicalError::InvalidConfig).
///
/// # Serialization
///
/// This struct supports JSON and other serde-compatible formats:
///
/// ```json
/// {
///   "version": 1,
///   "normalize_unicode": true,
///   "strip_punctuation": false,
///   "lowercase": true
/// }
/// ```
///
/// # Examples
///
/// ```rust
/// use canonical::CanonicalizeConfig;
///
/// // Default configuration (recommended)
/// let config = CanonicalizeConfig::default();
///
/// // Custom configuration
/// let custom = CanonicalizeConfig {
///     version: 2,
///     normalize_unicode: true,
///     strip_punctuation: true,
///     lowercase: true,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CanonicalizeConfig {
    /// Semantic version of the canonicalization configuration.
    ///
    /// This version number tracks changes to canonicalization behavior.
    /// Increment this when making changes that affect output (even bug fixes).
    ///
    /// # Requirements
    ///
    /// - Must be >= 1 (version 0 is reserved and rejected)
    /// - Must be monotonically increasing for behavior changes
    ///
    /// # Impact on Hashing
    ///
    /// The version is included in the document identity hash:
    /// ```text
    /// SHA-256(version.to_be_bytes() || 0x00 || text_bytes)
    /// ```
    ///
    /// This ensures different versions produce different hashes for the same input.
    ///
    /// # Example
    ///
    /// ```rust
    /// use canonical::CanonicalizeConfig;
    ///
    /// let config = CanonicalizeConfig::default();
    /// assert_eq!(config.version, 1);
    /// ```
    pub version: u32,

    /// If true, apply Unicode NFKC normalization before other transforms.
    ///
    /// NFKC (Normalization Form KC) performs compatibility decomposition
    /// followed by canonical composition. This:
    /// - Merges composed characters (e.g., "é" U+00E9 and "e"+combining accent)
    /// - Normalizes compatibility characters
    /// - Ensures consistent representation across different input forms
    ///
    /// # Unicode Equivalence
    ///
    /// With normalization enabled:
    /// ```text
    /// "Café" (U+00E9) → "Café"
    /// "Cafe"+accent (U+0301) → "Café"
    /// ```
    ///
    /// Both produce the same canonical form.
    ///
    /// # When to Enable
    ///
    /// **Recommended**: Always enable for text comparison and fingerprinting.
    /// Different systems may produce different Unicode representations of the
    /// same logical text.
    ///
    /// # When to Disable
    ///
    /// Only disable if you need to preserve exact byte-for-byte input,
    /// such as for cryptographic verification of original content.
    ///
    /// # Default
    ///
    /// `true` (enabled by default)
    pub normalize_unicode: bool,

    /// If true, strip punctuation characters before tokenizing.
    ///
    /// When enabled, all Unicode punctuation characters are treated as
    /// delimiters and removed from the canonical text.
    ///
    /// # Examples
    ///
    /// With `strip_punctuation: true`:
    /// ```text
    /// "Hello, world!" → "hello world"
    /// "It's 100% fun." → "it s 100 fun"
    /// "email@domain.com" → "email domain com"
    /// ```
    ///
    /// # When to Enable
    ///
    /// Enable for:
    /// - Keyword extraction
    /// - Bag-of-words analysis
    /// - Comparison tasks where punctuation is irrelevant
    ///
    /// # When to Disable
    ///
    /// Disable for:
    /// - Preserving semantic meaning (punctuation matters)
    /// - Code analysis
    /// - Email address preservation
    ///
    /// # Caution
    ///
    /// Stripping punctuation can change meaning:
    /// - "Let's eat, Grandma!" vs "Let's eat Grandma!"
    ///
    /// # Default
    ///
    /// `false` (disabled by default)
    pub strip_punctuation: bool,

    /// If true, apply locale-free Unicode case folding.
    ///
    /// When enabled, text is converted to lowercase using Unicode case folding,
    /// which is locale-independent and consistent across systems.
    ///
    /// # Unicode Case Folding
    ///
    /// Unlike system locale-based lowercasing, Unicode case folding:
    /// - Is consistent across all platforms and locales
    /// - Handles special cases like Turkish "İ" correctly
    /// - Applies standard Unicode case mapping rules
    ///
    /// # Examples
    ///
    /// ```text
    /// "Hello World" → "hello world"
    /// "İstanbul" → "i̇stanbul" (Turkish dotted capital I)
    /// "HELLO" → "hello"
    /// ```
    ///
    /// # When to Enable
    ///
    /// Enable for:
    /// - Case-insensitive matching
    /// - Search indexing
    /// - Deduplication
    /// - Most fingerprinting tasks
    ///
    /// # When to Disable
    ///
    /// Disable for:
    /// - Preserving proper nouns (names, brands)
    /// - Case-sensitive analysis
    /// - Display text preservation
    ///
    /// # Default
    ///
    /// `true` (enabled by default)
    pub lowercase: bool,
}

impl Default for CanonicalizeConfig {
    /// Creates the default `CanonicalizeConfig`.
    ///
    /// # Defaults
    ///
    /// - `version`: 1
    /// - `normalize_unicode`: true (recommended for consistency)
    /// - `strip_punctuation`: false (preserves punctuation)
    /// - `lowercase`: true (case-insensitive)
    ///
    /// # Recommended Settings
    ///
    /// The default configuration is suitable for most text fingerprinting
    /// and comparison tasks. It provides:
    /// - Unicode normalization for consistent character representation
    /// - Case folding for case-insensitive matching
    /// - Preservation of punctuation for semantic meaning
    ///
    /// # Example
    ///
    /// ```rust
    /// use canonical::CanonicalizeConfig;
    ///
    /// let config = CanonicalizeConfig::default();
    /// assert_eq!(config.version, 1);
    /// assert!(config.normalize_unicode);
    /// assert!(!config.strip_punctuation);
    /// assert!(config.lowercase);
    /// ```
    fn default() -> Self {
        Self {
            version: 1,
            normalize_unicode: true,
            strip_punctuation: false,
            lowercase: true,
        }
    }
}
