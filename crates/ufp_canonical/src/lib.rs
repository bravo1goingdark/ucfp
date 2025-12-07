//! # UCFP Canonicalization
//!
//! This crate provides the core text canonicalization pipeline for the Universal
//! Content Fingerprinting (UCFP) framework. Its primary responsibility is to
//! transform raw text into a deterministic, normalized representation that can be
//! reliably used by downstream consumers for fingerprinting, embedding, and
//! content-based analysis.
//!
//! ## Core Responsibilities
//!
//! - **Unicode Normalization**: Converts text to a canonical form (NFKC by default)
//!   to handle variations in Unicode representation (e.g., "Café" vs. "Café").
//! - **Text Cleaning**: Optionally strips punctuation and normalizes case
//!   (lowercase by default) to reduce noise.
//! - **Whitespace Collapsing**: Collapses all whitespace sequences (spaces, tabs,
//!   newlines) into a single space to ensure tokenization is consistent.
//! - **Tokenization**: Splits the normalized text into a stream of tokens,
//!   preserving the byte offsets of each token relative to the canonical text.
//!   This is crucial for mapping fingerprints back to the original content.
//! - **Checksumming**: Computes a SHA-256 digest of the canonical text, providing
//!   a deterministic identifier for byte-identical content.
//!
//! ## Key Concepts
//!
//! The pipeline is driven by the [`CanonicalizeConfig`] struct, which allows callers
//! to fine-tune the normalization process. The main entry point is the
//! [`canonicalize`] function, which takes a document ID and raw text, and returns
//! a [`CanonicalizedDocument`]. This output struct contains the cleaned text, a
//! vector of [`Token`]s with their offsets, and the SHA-256 hash.
//!
//! The design emphasizes determinism and efficiency. By processing the text in a
//! single pass, it minimizes allocations and computational overhead while ensuring
//! that the same input always produces the same output for a given configuration.
//!
//! ## Example Usage
//!
//! ```
//! use ufp_canonical::{canonicalize, CanonicalizeConfig};
//!
//! let cfg = CanonicalizeConfig {
//!     strip_punctuation: true,
//!     ..Default::default()
//! };
//!
//! let doc = canonicalize("doc-123", "  Hello,  \n world!  ", &cfg)
//!     .expect("Canonicalization should succeed");
//!
//! assert_eq!(doc.canonical_text, "hello world");
//! assert_eq!(doc.tokens.len(), 2);
//! assert_eq!(doc.tokens[0].text, "hello");
//! assert_eq!(doc.tokens[1].text, "world");
//! ```

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use unicode_categories::UnicodeCategories;
use unicode_normalization::UnicodeNormalization;

/// Configuration for canonicalization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalizeConfig {
    /// Semantic version of the canonicalization configuration.
    pub version: u32,
    /// If true, apply Unicode NFKC normalization before other transforms.
    pub normalize_unicode: bool,
    /// If true, strip punctuation characters before tokenizing.
    pub strip_punctuation: bool,
    /// If true, lowercase the text.
    pub lowercase: bool,
}

impl Default for CanonicalizeConfig {
    fn default() -> Self {
        Self {
            version: 1,
            normalize_unicode: true,
            strip_punctuation: false,
            lowercase: true,
        }
    }
}

/// Errors that can occur during canonicalization.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CanonicalError {
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("canonical document requires a non-empty doc_id")]
    MissingDocId,
    #[error("input text empty after normalization")]
    EmptyInput,
}

/// A token with its UTF-8 byte offsets in the canonical text.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Token {
    pub text: String,
    pub start: usize, // byte offset (inclusive)
    pub end: usize,   // byte offset (exclusive)
}

impl AsRef<str> for Token {
    fn as_ref(&self) -> &str {
        self.text.as_str()
    }
}

/// Output of canonicalization: canonical text, token stream and checksum.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CanonicalizedDocument {
    pub doc_id: String,
    pub canonical_text: String,
    pub tokens: Vec<Token>,
    pub sha256_hex: String,
}

/// Main entry point. Takes an optional payload and config and returns a canonicalized document.
pub fn canonicalize(
    doc_id: impl Into<String>,
    input: &str,
    cfg: &CanonicalizeConfig,
) -> Result<CanonicalizedDocument, CanonicalError> {
    // Config validation: version 0 is reserved and invalid.
    if cfg.version == 0 {
        return Err(CanonicalError::InvalidConfig(
            "config version must be >= 1".into(),
        ));
    }

    // A document ID is required for traceability.
    let doc_id = doc_id.into();
    let doc_id = doc_id.trim();
    if doc_id.is_empty() {
        return Err(CanonicalError::MissingDocId);
    }
    let doc_id = doc_id.to_string();

    // Pre-allocate for efficiency, assuming canonical text is roughly the same size as input.
    let mut canonical_text = String::with_capacity(input.len());
    let mut hasher = Sha256::new();
    let mut tokens: Vec<Token> = Vec::new();
    // State machine for tokenization and whitespace collapsing.
    let mut pending_space = false;
    let mut current_token_start: Option<usize> = None;

    // Unicode normalization is the first step, as it can affect character boundaries.
    if cfg.normalize_unicode {
        process_chars(
            input.nfkc(),
            cfg,
            &mut canonical_text,
            &mut hasher,
            &mut tokens,
            &mut pending_space,
            &mut current_token_start,
        );
    } else {
        process_chars(
            input.chars(),
            cfg,
            &mut canonical_text,
            &mut hasher,
            &mut tokens,
            &mut pending_space,
            &mut current_token_start,
        );
    }

    // The last token needs to be finalized after the loop.
    finalize_token(&mut tokens, &canonical_text, &mut current_token_start);

    // After all transformations, if the text is empty, it's an error.
    if canonical_text.is_empty() {
        return Err(CanonicalError::EmptyInput);
    }

    let sha256_hex = hex::encode(hasher.finalize());

    Ok(CanonicalizedDocument {
        doc_id,
        canonical_text,
        tokens,
        sha256_hex,
    })
}

/// Helper to iterate over characters and dispatch them for processing.
fn process_chars<I>(
    iter: I,
    cfg: &CanonicalizeConfig,
    canonical_text: &mut String,
    hasher: &mut Sha256,
    tokens: &mut Vec<Token>,
    pending_space: &mut bool,
    current_token_start: &mut Option<usize>,
) where
    I: Iterator<Item = char>,
{
    for ch in iter {
        // Lowercasing can expand a single character into multiple (e.g., German ß -> ss).
        if cfg.lowercase {
            for lower in ch.to_lowercase() {
                dispatch_char(
                    lower,
                    cfg,
                    canonical_text,
                    hasher,
                    tokens,
                    pending_space,
                    current_token_start,
                );
            }
        } else {
            dispatch_char(
                ch,
                cfg,
                canonical_text,
                hasher,
                tokens,
                pending_space,
                current_token_start,
            );
        }
    }
}

/// Decides whether a character is part of a token or a delimiter.
fn dispatch_char(
    ch: char,
    cfg: &CanonicalizeConfig,
    canonical_text: &mut String,
    hasher: &mut Sha256,
    tokens: &mut Vec<Token>,
    pending_space: &mut bool,
    current_token_start: &mut Option<usize>,
) {
    let is_delim = ch.is_whitespace() || (cfg.strip_punctuation && ch.is_punctuation());
    if is_delim {
        // When a delimiter is found, the current token (if any) is finalized.
        finalize_token(tokens, canonical_text, current_token_start);
        // We note that a space should be added before the next token.
        if !canonical_text.is_empty() {
            *pending_space = true;
        }
    } else {
        // If it's not a delimiter, it's part of a token.
        append_char(
            ch,
            canonical_text,
            hasher,
            current_token_start,
            pending_space,
        );
    }
}

/// Appends a character to the canonical text and updates the hasher.
fn append_char(
    ch: char,
    canonical_text: &mut String,
    hasher: &mut Sha256,
    current_token_start: &mut Option<usize>,
    pending_space: &mut bool,
) {
    // If a space is pending, add it and mark the start of a new token.
    if *pending_space {
        canonical_text.push(' ');
        hasher.update(b" ");
        *pending_space = false;
        *current_token_start = Some(canonical_text.len());
    } else if current_token_start.is_none() {
        // If there's no token in progress, start a new one.
        *current_token_start = Some(canonical_text.len());
    }

    // Append the character to the canonical text and update the hash.
    let mut buf = [0u8; 4];
    let encoded = ch.encode_utf8(&mut buf);
    canonical_text.push_str(encoded);
    hasher.update(encoded.as_bytes());
}

/// Creates a token from the current token start and adds it to the list.
fn finalize_token(
    tokens: &mut Vec<Token>,
    canonical_text: &str,
    current_token_start: &mut Option<usize>,
) {
    // `take()` removes the value from the Option, leaving `None`.
    if let Some(start) = current_token_start.take() {
        // Ensure the token has content before creating it.
        if start < canonical_text.len() {
            let end = canonical_text.len();
            tokens.push(Token {
                text: canonical_text[start..end].to_string(),
                start,
                end,
            });
        }
    }
}

/// Collapses repeated whitespace, trims edges, and normalizes newlines to single spaces.
pub fn collapse_whitespace(s: &str) -> String {
    let mut normalized = String::with_capacity(s.len());
    for segment in s.split_whitespace() {
        if !normalized.is_empty() {
            normalized.push(' ');
        }
        normalized.push_str(segment);
    }
    normalized
}

/// Tokenizes canonical text and produces byte offsets.
pub fn tokenize(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut start: Option<usize> = None;

    for (idx, ch) in text.char_indices() {
        if ch.is_whitespace() {
            if let Some(token_start) = start.take() {
                tokens.push(Token {
                    text: text[token_start..idx].to_string(),
                    start: token_start,
                    end: idx,
                });
            }
        } else if start.is_none() {
            start = Some(idx);
        }
    }

    if let Some(token_start) = start {
        tokens.push(Token {
            text: text[token_start..].to_string(),
            start: token_start,
            end: text.len(),
        });
    }

    tokens
}

/// Hashes canonical text with SHA-256 and returns hex digest.
pub fn hash_text(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hex::encode(hasher.finalize())
}

// -----------------------------
// Unit tests
// -----------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_canonicalize_default() {
        let input = "  HAcllo\nWORLD!  This is   UCFP. ";
        let cfg = CanonicalizeConfig::default();
        let out = canonicalize("doc-basic", input, &cfg).expect("canonicalization succeeds");

        assert_eq!(out.canonical_text, "hacllo world! this is ucfp.");
        assert_eq!(out.doc_id, "doc-basic");

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

        assert_eq!(out.sha256_hex, hash_text(&out.canonical_text));
    }

    #[test]
    fn test_strip_punctuation_canonicalize() {
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
    fn test_unicode_equivalence() {
        let composed = "Caf\u{00E9}";
        let decomposed = "Cafe\u{0301}";
        let cfg = CanonicalizeConfig::default();

        let doc_a = canonicalize("doc-a", composed, &cfg).expect("canonical composed");
        let doc_b = canonicalize("doc-b", decomposed, &cfg).expect("canonical decomposed");

        assert_eq!(doc_a.canonical_text, doc_b.canonical_text);
        assert_eq!(doc_a.sha256_hex, doc_b.sha256_hex);
    }

    #[test]
    fn test_token_offsets_stable() {
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
    fn test_hash_text_determinism() {
        let texts = ["", "hello world", "こんにちは世界", "emoji \u{1f600}"];

        for text in texts {
            let hash_once = hash_text(text);
            let hash_twice = hash_text(text);
            assert_eq!(hash_once, hash_twice);
        }
    }

    #[test]
    fn test_empty_input_rejected() {
        let cfg = CanonicalizeConfig::default();
        let res = canonicalize("empty-doc", "   ", &cfg);
        assert!(matches!(res, Err(CanonicalError::EmptyInput)));
    }

    #[test]
    fn test_missing_doc_id_rejected() {
        let cfg = CanonicalizeConfig::default();
        let res = canonicalize("", "content", &cfg);
        assert!(matches!(res, Err(CanonicalError::MissingDocId)));
    }

    #[test]
    fn test_disable_unicode_normalization() {
        let cfg = CanonicalizeConfig {
            normalize_unicode: false,
            ..Default::default()
        };
        let doc = canonicalize("doc-raw", "Cafe\u{0301}", &cfg).expect("canonicalization succeeds");
        assert_eq!(doc.canonical_text, "cafe\u{0301}");
    }

    #[test]
    fn test_invalid_config_version_rejected() {
        let cfg = CanonicalizeConfig {
            version: 0,
            ..Default::default()
        };
        let res = canonicalize("doc-invalid", "content", &cfg);
        assert!(matches!(res, Err(CanonicalError::InvalidConfig(_))));
    }
}
