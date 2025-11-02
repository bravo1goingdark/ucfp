//! Canonical text normalization utilities for the Universal Content Fingerprinting (UCFP) pipeline.
//!
//! Responsibilities:
//! - Unicode NFKC normalization
//! - Lowercasing (Unicode-aware)
//! - Optional punctuation stripping
//! - Collapsing whitespace to single spaces
//! - Tokenization into tokens with byte offsets (UTF-8 byte offsets)
//! - SHA-256 checksum of canonical text

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use unicode_categories::UnicodeCategories;
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
    let mut canonical_text = String::with_capacity(input.len());
    let mut hasher = Sha256::new();
    let mut tokens: Vec<Token> = Vec::new();
    let mut pending_space = false;
    let mut current_token_start: Option<usize> = None;

    let push_char = |ch: char,
                     canonical_text: &mut String,
                     hasher: &mut Sha256,
                     current_token_start: &mut Option<usize>,
                     pending_space: &mut bool| {
        if *pending_space {
            canonical_text.push(' ');
            hasher.update(b" ");
            *pending_space = false;
            *current_token_start = Some(canonical_text.len());
        } else if current_token_start.is_none() {
            *current_token_start = Some(canonical_text.len());
        }

        let mut buf = [0u8; 4];
        let encoded = ch.encode_utf8(&mut buf);
        canonical_text.push_str(encoded);
        hasher.update(encoded.as_bytes());
    };

    let finalize_token = |tokens: &mut Vec<Token>,
                          canonical_text: &String,
                          current_token_start: &mut Option<usize>| {
        if let Some(start) = current_token_start.take() {
            let end = canonical_text.len();
            tokens.push(Token {
                text: canonical_text[start..end].to_string(),
                start,
                end,
            });
        }
    };

    for ch in input.nfkc() {
        if cfg.lowercase {
            for lower in ch.to_lowercase() {
                let is_delim = lower.is_whitespace()
                    || (cfg.strip_punctuation && lower.is_punctuation());
                if is_delim {
                    finalize_token(&mut tokens, &canonical_text, &mut current_token_start);
                    if !canonical_text.is_empty() {
                        pending_space = true;
                    }
                } else {
                    push_char(
                        lower,
                        &mut canonical_text,
                        &mut hasher,
                        &mut current_token_start,
                        &mut pending_space,
                    );
                }
            }
        } else {
            let is_delim = ch.is_whitespace()
                || (cfg.strip_punctuation && ch.is_punctuation());
            if is_delim {
                finalize_token(&mut tokens, &canonical_text, &mut current_token_start);
                if !canonical_text.is_empty() {
                    pending_space = true;
                }
            } else {
                push_char(
                    ch,
                    &mut canonical_text,
                    &mut hasher,
                    &mut current_token_start,
                    &mut pending_space,
                );
            }
        }
    }

    finalize_token(&mut tokens, &canonical_text, &mut current_token_start);
    let sha256_hex = hex::encode(hasher.finalize());

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
