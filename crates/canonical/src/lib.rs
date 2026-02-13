//! UCFP Canonical Layer - Deterministic Text Canonicalization
//!
//! This crate provides the second stage in the Universal Content Fingerprinting (UCFP)
//! pipeline, transforming text into a deterministic, versioned format suitable for
//! downstream processing (perceptual fingerprinting, semantic embeddings, indexing).
//!
//! # Overview
//!
//! The `canonical` crate is responsible for:
//! - **Unicode Normalization**: NFKC normalization for consistent character representation
//! - **Text Transformation**: Locale-free lowercasing, optional punctuation stripping
//! - **Whitespace Normalization**: Collapsing consecutive whitespace to single spaces
//! - **Tokenization**: Producing offset-aware tokens for downstream stages
//! - **Versioned Hashing**: Computing stable identity hashes that include config version
//!
//! # Core Guarantee
//!
//! > **Same input text + same `CanonicalizeConfig` → identical `CanonicalizedDocument`, forever.**
//!
//! This crate is **pure** and **side-effect free**:
//! - No I/O operations
//! - No network calls
//! - No dependence on wall-clock time, locale, or hardware
//!
//! # Pipeline Position
//!
//! ```text
//! Raw Text ──▶ Ingest ──▶ Canonical ──▶ Perceptual/Semantic ──▶ Index ──▶ Match
//!                              ↑
//!                           (this crate)
//! ```
//!
//! # Quick Start
//!
//! ```rust
//! use canonical::{canonicalize, CanonicalizeConfig};
//!
//! // Use default configuration (recommended)
//! let config = CanonicalizeConfig::default();
//!
//! // Canonicalize some text
//! let doc = canonicalize("doc-001", "  Hello   WORLD  ", &config).unwrap();
//!
//! assert_eq!(doc.canonical_text, "hello world");
//! assert_eq!(doc.tokens.len(), 2);
//! assert!(!doc.sha256_hex.is_empty()); // Version-aware identity hash
//! ```
//!
//! # Configuration
//!
//! ## Default Configuration (Recommended)
//!
//! ```rust
//! use canonical::CanonicalizeConfig;
//!
//! let config = CanonicalizeConfig::default();
//! // version: 1
//! // normalize_unicode: true
//! // strip_punctuation: false
//! // lowercase: true
//! ```
//!
//! ## Custom Configuration
//!
//! ```rust
//! use canonical::CanonicalizeConfig;
//!
//! let config = CanonicalizeConfig {
//!     version: 1,
//!     normalize_unicode: true,
//!     strip_punctuation: true,  // Remove punctuation
//!     lowercase: true,
//! };
//! ```
//!
//! # Canonicalization Pipeline
//!
//! The `canonicalize()` function implements a deterministic pipeline:
//!
//! 1. **Config Validation**: Ensure version >= 1, doc_id non-empty
//! 2. **Unicode Normalization** (optional): NFKC normalization
//! 3. **Case Folding** (optional): Locale-free lowercasing
//! 4. **Character Processing**: Delimiter detection, punctuation handling
//! 5. **Whitespace Collapsing**: Multiple spaces → single spaces
//! 6. **Tokenization**: Extract tokens with byte offsets
//! 7. **Hashing**: Compute version-aware identity hash
//!
//! # Hash Algorithms
//!
//! ## Document Identity Hash
//!
//! ```text
//! SHA-256(version.to_be_bytes() || 0x00 || canonical_text_bytes)
//! ```
//!
//! This hash uniquely identifies the canonical document and includes the version,
//! ensuring that different canonicalization versions produce different hashes even
//! for the same input text.
//!
//! ## Token Hash
//!
//! ```text
//! SHA-256(version.to_be_bytes() || 0x01 || token_text_bytes)
//! ```
//!
//! Each token gets its own stable hash using a different discriminator byte.
//!
//! # Module Structure
//!
//! - `config`: Configuration types (`CanonicalizeConfig`)
//! - `document`: Output types (`CanonicalizedDocument`, `Token`)
//! - `error`: Error types (`CanonicalError`)
//! - `hash`: Hashing utilities (`hash_text`, `hash_canonical_bytes`)
//! - `pipeline`: Main canonicalization logic (`canonicalize`)
//! - `token`: Tokenization utilities (`tokenize`)
//! - `whitespace`: Whitespace normalization (`collapse_whitespace`)
//!
//! # Error Handling
//!
//! All errors are typed via [`CanonicalError`]:
//!
//! ```rust
//! use canonical::{canonicalize, CanonicalizeConfig, CanonicalError};
//!
//! let config = CanonicalizeConfig::default();
//!
//! match canonicalize("doc-001", "   ", &config) {
//!     Ok(doc) => println!("Canonicalized: {}", doc.canonical_text),
//!     Err(CanonicalError::EmptyInput) => println!("Input is empty after normalization"),
//!     Err(CanonicalError::MissingDocId) => println!("Document ID is required"),
//!     Err(CanonicalError::InvalidConfig(msg)) => println!("Invalid config: {}", msg),
//! }
//! ```
//!
//! # Performance
//!
//! All operations are O(n) where n is the input length:
//! - Unicode normalization: O(n)
//! - Case folding: O(n)
//! - Whitespace collapsing: O(n)
//! - Tokenization: O(n)
//! - SHA-256 hashing: O(n)
//!
//! Memory allocations are minimized using `Cow<str>` and pre-allocated vectors.
//!
//! # Examples
//!
//! See the `examples/` directory for complete working examples:
//! - `demo.rs`: Basic canonicalization examples
//!
//! # See Also
//!
//! - [Crate documentation](doc/canonical.md) for comprehensive guides
//! - `config` module for configuration details
//! - `document` module for output structure definitions

mod config;
mod document;
mod error;
mod hash;
mod pipeline;
mod token;
mod whitespace;

pub use crate::config::CanonicalizeConfig;
pub use crate::document::CanonicalizedDocument;
pub use crate::error::CanonicalError;
pub use crate::hash::{hash_canonical_bytes, hash_text};
pub use crate::pipeline::canonicalize;
pub use crate::token::{tokenize, Token};
pub use crate::whitespace::collapse_whitespace;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_canonicalize_default() {
        let input = "  HAcllo\nWORLD!  This is   UCFP. ";
        let cfg = CanonicalizeConfig::default();
        let out = canonicalize("doc-basic", input, &cfg).expect("canonicalization succeeds");

        assert_eq!(out.canonical_text, "hacllo world! this is ucfp.");
        assert_eq!(out.doc_id, "doc-basic");
        assert_eq!(out.canonical_version, cfg.version);
        assert_eq!(out.config, cfg);

        let expected_tokens = vec![
            ("hacllo", 0usize, 6usize),
            ("world!", 7, 13),
            ("this", 14, 18),
            ("is", 19, 21),
            ("ucfp.", 22, 27),
        ];
        assert_eq!(out.tokens.len(), expected_tokens.len());
        for (token, (text, start, end)) in out.tokens.iter().zip(expected_tokens.into_iter()) {
            assert_eq!(token.text, text);
            assert_eq!(token.start, start);
            assert_eq!(token.end, end);
        }

        let expected_hash =
            hash_canonical_bytes(out.canonical_version, out.canonical_text.as_bytes());
        assert_eq!(out.sha256_hex, expected_hash);
    }

    #[test]
    fn strip_punctuation_canonicalize() {
        let input = "Hello, world! It's UCFP: 100% fun.";
        let cfg = CanonicalizeConfig {
            strip_punctuation: true,
            ..Default::default()
        };
        let out = canonicalize("doc-strip", input, &cfg).expect("canonicalization succeeds");
        assert_eq!(out.canonical_text, "hello world it s ucfp 100 fun");
        let token_texts: Vec<String> = out.tokens.iter().map(|t| t.text.clone()).collect();
        assert_eq!(
            token_texts,
            vec!["hello", "world", "it", "s", "ucfp", "100", "fun"]
        );
    }

    #[test]
    fn unicode_equivalence_nfkc() {
        let composed = "Caf\u{00E9}";
        let decomposed = "Cafe\u{0301}";
        let cfg = CanonicalizeConfig::default();

        let doc_a = canonicalize("doc-a", composed, &cfg).expect("canonical composed");
        let doc_b = canonicalize("doc-b", decomposed, &cfg).expect("canonical decomposed");

        assert_eq!(doc_a.canonical_text, doc_b.canonical_text);
        assert_eq!(doc_a.sha256_hex, doc_b.sha256_hex);
    }

    #[test]
    fn token_offsets_stable_for_non_bmp() {
        let cfg = CanonicalizeConfig::default();
        let doc =
            canonicalize("doc-token", " a\u{10348}b  c ", &cfg).expect("canonicalization succeeds");

        let expected = vec![
            Token {
                text: "a\u{10348}b".to_string(),
                start: 0,
                end: "a\u{10348}b".len(),
            },
            Token {
                text: "c".to_string(),
                start: "a\u{10348}b ".len(),
                end: "a\u{10348}b c".len(),
            },
        ];
        assert_eq!(doc.tokens, expected);
    }

    #[test]
    fn hash_text_determinism() {
        let texts = ["", "hello world", "こんにちは世界", "emoji \u{1f600}"];

        for text in texts {
            let hash_once = hash_text(text);
            let hash_twice = hash_text(text);
            assert_eq!(hash_once, hash_twice);
        }
    }

    #[test]
    fn empty_input_rejected() {
        let cfg = CanonicalizeConfig::default();
        let res = canonicalize("empty-doc", "   ", &cfg);
        assert!(matches!(res, Err(CanonicalError::EmptyInput)));
    }

    #[test]
    fn missing_doc_id_rejected() {
        let cfg = CanonicalizeConfig::default();
        let res = canonicalize("", "content", &cfg);
        assert!(matches!(res, Err(CanonicalError::MissingDocId)));
    }

    #[test]
    fn disable_unicode_normalization() {
        let cfg = CanonicalizeConfig {
            normalize_unicode: false,
            ..Default::default()
        };
        let doc = canonicalize("doc-raw", "Cafe\u{0301}", &cfg).expect("canonicalization succeeds");
        assert_eq!(doc.canonical_text, "cafe\u{0301}");
    }

    #[test]
    fn invalid_config_version_rejected() {
        let cfg = CanonicalizeConfig {
            version: 0,
            ..Default::default()
        };
        let res = canonicalize("doc-invalid", "content", &cfg);
        assert!(matches!(res, Err(CanonicalError::InvalidConfig(_))));
    }

    #[test]
    fn canonical_hash_includes_version() {
        let cfg_v1 = CanonicalizeConfig::default();
        let cfg_v2 = CanonicalizeConfig {
            version: cfg_v1.version + 1,
            ..CanonicalizeConfig::default()
        };

        let doc_v1 = canonicalize("doc", "Same text", &cfg_v1).expect("v1");
        let doc_v2 = canonicalize("doc", "Same text", &cfg_v2).expect("v2");

        assert_eq!(doc_v1.canonical_text, doc_v2.canonical_text);
        assert_ne!(doc_v1.sha256_hex, doc_v2.sha256_hex);
    }
}
