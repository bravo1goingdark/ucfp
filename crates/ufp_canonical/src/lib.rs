// Responsibilities:
// - Unicode NFKC normalization
// - Lowercasing (Unicode-aware)
// - Optional punctuation stripping
// - Collapsing whitespace to single spaces
// - Tokenization into tokens with byte offsets (UTF-8 byte offsets)
// - SHA-256 checksum of canonical text

use regex::Regex;
use sha2::{Digest, Sha256};
use std::borrow::Cow;
use unicode_normalization::UnicodeNormalization;
use serde::{Serialize, Deserialize};

/// Configuration for canonicalization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalizeConfig {
    /// If true, strip punctuation characters (based on a unicode-aware regex) before tokenizing.
    pub strip_punctuation: bool,
    /// If true, lowercase the text.
    pub lowercase: bool,
}

impl Default for CanonicalizeConfig {
    fn default() -> Self {
        Self { strip_punctuation: false, lowercase: true }
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalizedDocument {
    pub canonical_text: String,
    pub tokens: Vec<Token>,
    pub sha256_hex: String,
}

/// Main entry point. Takes an optional payload and config and returns a canonicalized document.
pub fn canonicalize(input: &str, cfg: &CanonicalizeConfig) -> CanonicalizedDocument {
    // 1) Unicode NFKC normalization (composed form)
    let mut s: Cow<str> = input.nfkc().collect::<String>().into();

    // 2) Optionally lowercase (unicode-aware)
    if cfg.lowercase {
        s = Cow::Owned(s.to_lowercase());
    }

    // 3) Optionally strip punctuation
    if cfg.strip_punctuation {
        // Use a conservative regex: remove characters in Unicode punctuation category (\p{P})
        // Keep word characters, whitespace, and numbers. This removes punctuation marks.
        // Note: regex crate uses Rust Unicode properties.
        // Replace punctuation with a single space to avoid joining words.
        lazy_static::lazy_static! {
            static ref PUNCT_RE: Regex = Regex::new(r"\p{P}+").unwrap();
        }
        let replaced = PUNCT_RE.replace_all(&s, " ");
        s = Cow::Owned(replaced.into_owned());
    }

    // 4) Collapse whitespace (any Unicode whitespace) into single spaces and trim
    // We'll split_whitespace and join to ensure deterministic behaviour.
    let collapsed = s.split_whitespace().collect::<Vec<&str>>().join(" ");
    let canonical_text = collapsed.trim().to_string();

    // 5) Tokenize and collect byte offsets. Tokenization: split on spaces (simple) and record byte offsets.
    // This keeps tokenization deterministic and consistent with shingling parameters.
    let mut tokens: Vec<Token> = Vec::new();
    if canonical_text.is_empty() {
        // compute checksum of empty string too
        let sha256_hex = hex::encode(Sha256::digest(canonical_text.as_bytes()));
        return CanonicalizedDocument { canonical_text, tokens, sha256_hex };
    }

    // We'll iterate over the bytes while using char indices to compute byte offsets.
    // Simpler: walk through canonical_text and locate token boundaries by searching for spaces.
    let bytes = canonical_text.as_bytes();
    let mut i = 0usize;
    let n = bytes.len();
    while i < n {
        // skip spaces (shouldn't be any due to collapse, but be defensive)
        if bytes[i] == b' ' {
            i += 1;
            continue;
        }
        // start of token at byte i
        let start = i;
        // find next space or end
        let mut j = i;
        while j < n && bytes[j] != b' ' { j += 1; }
        let end = j;
        // extract token string using byte offsets (valid UTF-8 guaranteed)
        let tok = canonical_text[start..end].to_string();
        tokens.push(Token { text: tok, start, end });
        i = end + 1; // move past space (or end)
    }

    // 6) SHA-256 checksum of canonical_text
    let mut hasher = Sha256::new();
    hasher.update(canonical_text.as_bytes());
    let sha256_hex = hex::encode(hasher.finalize());

    CanonicalizedDocument { canonical_text, tokens, sha256_hex }
}

// -----------------------------
// Unit tests
// -----------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_canonicalize_default() {
        let input = "  Héllo\nWORLD!  This is   UCFP. ";
        let cfg = CanonicalizeConfig::default();
        let out = canonicalize(input, &cfg);
        // default cfg lowercases but does not strip punctuation
        assert_eq!(out.canonical_text, "héllo world! this is ucfp.");
        // tokens should be split on single spaces
        let token_texts: Vec<String> = out.tokens.iter().map(|t| t.text.clone()).collect();
        assert_eq!(token_texts, vec!["héllo", "world!", "this", "is", "ucfp."]);
        // checksum should be stable
        let expected = hex::encode(Sha256::digest(out.canonical_text.as_bytes()));
        assert_eq!(out.sha256_hex, expected);
    }

    #[test]
    fn test_strip_punctuation() {
        let input = "Hello, world! It's UCFP: 100% fun.";
        let mut cfg = CanonicalizeConfig::default();
        cfg.strip_punctuation = true;
        let out = canonicalize(input, &cfg);
        assert_eq!(out.canonical_text, "hello world it s ucfp 100 fun");
        let token_texts: Vec<String> = out.tokens.iter().map(|t| t.text.clone()).collect();
        assert_eq!(token_texts, vec!["hello", "world", "it", "s", "ucfp", "100", "fun"]);
    }

    #[test]
    fn test_empty_input() {
        let input = "   \n \t  ";
        let cfg = CanonicalizeConfig::default();
        let out = canonicalize(input, &cfg);
        assert_eq!(out.canonical_text, "");
        assert!(out.tokens.is_empty());
        let expected = hex::encode(Sha256::digest("".as_bytes()));
        assert_eq!(out.sha256_hex, expected);
    }
}


