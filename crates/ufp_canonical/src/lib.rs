//! Canonical text normalization utilities for the Universal Content Fingerprinting (UCFP) pipeline.
//!
//! Responsibilities:
//! - Unicode NFKC normalization
//! - Lowercasing (Unicode-aware)
//! - Optional punctuation stripping
//! - Collapsing whitespace to single spaces
//! - Tokenization into tokens with byte offsets (UTF-8 byte offsets)
//! - SHA-256 checksum of canonical text

use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::borrow::Cow;
use std::sync::OnceLock;
use unicode_normalization::UnicodeNormalization;

/// Configuration for canonicalization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalizeConfig {
    /// If true, strip punctuation characters before tokenizing.
    pub strip_punctuation: bool,
    /// If true, lowercase the text.
    pub lowercase: bool,
}

impl Default for CanonicalizeConfig {
    fn default() -> Self {
        Self {
            strip_punctuation: false,
            lowercase: true,
        }
    }
}

/// A token with its UTF-8 byte offsets in the canonical text.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Token {
    pub text: String,
    pub start: usize, // byte offset (inclusive)
    pub end: usize,   // byte offset (exclusive)
}

/// Output of canonicalization: canonical text, token stream and checksum.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CanonicalizedDocument {
    pub canonical_text: String,
    pub tokens: Vec<Token>,
    pub sha256_hex: String,
}

/// Main entry point. Takes an optional payload and config and returns a canonicalized document.
pub fn canonicalize(input: &str, cfg: &CanonicalizeConfig) -> CanonicalizedDocument {
    let normalized: String = input.nfkc().collect();

    let lower = if cfg.lowercase {
        normalized.to_lowercase()
    } else {
        normalized
    };

    let cleaned: Cow<'_, str> = if cfg.strip_punctuation {
        match strip_punctuation(&lower) {
            Some(stripped) => Cow::Owned(stripped),
            None => Cow::Borrowed(lower.as_str()),
        }
    } else {
        Cow::Borrowed(lower.as_str())
    };

    let canonical_text = collapse_whitespace(cleaned.as_ref());
    let tokens = tokenize(&canonical_text);
    let sha256_hex = hash_text(&canonical_text);

    CanonicalizedDocument {
        canonical_text,
        tokens,
        sha256_hex,
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

fn strip_punctuation(input: &str) -> Option<String> {
    let regex = punctuation_regex()?;
    if !regex.is_match(input) {
        return None;
    }
    let replaced = regex.replace_all(input, " ");
    Some(replaced.into_owned())
}

fn punctuation_regex() -> Option<&'static Regex> {
    static REGEX: OnceLock<Option<Regex>> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\p{P}+").ok()).as_ref()
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
        let out = canonicalize(input, &cfg);

        assert_eq!(out.canonical_text, "hacllo world! this is ucfp.");

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
        let out = canonicalize(input, &cfg);
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

        let doc_a = canonicalize(composed, &cfg);
        let doc_b = canonicalize(decomposed, &cfg);

        assert_eq!(doc_a.canonical_text, doc_b.canonical_text);
        assert_eq!(doc_a.sha256_hex, doc_b.sha256_hex);
    }

    #[test]
    fn test_token_offsets_stable() {
        let cfg = CanonicalizeConfig::default();
        let doc = canonicalize(" a\u{10348}b  c ", &cfg);

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
}
