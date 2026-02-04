//! UCFP canonical text layer.
//!
//! This module normalizes text into a deterministic, versioned format. Downstream
//! stages (perceptual, semantic, index) can rely on this for stable identity.
//!
//! ## What we do
//!
//! - Unicode normalization (NFKC by default, configurable)
//! - Casing and punctuation handling (lowercase, optional stripping)
//! - Whitespace normalization (collapses to single spaces)
//! - Tokenization with byte offsets for downstream accuracy
//! - Versioned hashes so you can tell which canonicalization was used
//!
//! ## Pure function guarantee
//!
//! No I/O, no clock calls, no OS/locale dependence. Give us the same text
//! and config, you get the same result on any machine.
//!
//! ## Invariants worth knowing
//!
//! - Input should be trusted UTF-8 (usually from ingest stage)
//! - We don't re-validate ingest constraints here
//! - Output depends only on text + config
//! - Hash = SHA-256(version || 0x00 || canonical_text)
//!
//! Bottom line: same input + same config = same output forever.

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
