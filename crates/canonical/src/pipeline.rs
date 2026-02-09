use std::borrow::Cow;

use unicode_categories::UnicodeCategories;
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

use crate::config::CanonicalizeConfig;
use crate::document::CanonicalizedDocument;
use crate::error::CanonicalError;
use crate::hash::{hash_canonical_bytes, hash_token_bytes};
use crate::token::Token;

/// Main entry point. Takes an input payload and config and returns a
/// canonicalized document.
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
    let doc_id: String = doc_id.into();
    let trimmed = doc_id.trim();
    if trimmed.is_empty() {
        return Err(CanonicalError::MissingDocId);
    }
    let doc_id = if doc_id.len() == trimmed.len() {
        // No trimming needed, reuse the original String
        doc_id
    } else {
        // Only allocate a new String if trimming actually removed something
        trimmed.to_string()
    };

    // Unicode normalization is the first step, as it can affect character boundaries.
    // Use Cow to avoid allocation when normalization is disabled.
    let normalized_text: Cow<str> = if cfg.normalize_unicode {
        Cow::Owned(input.nfkc().collect::<String>())
    } else {
        Cow::Borrowed(input)
    };

    // Pre-allocate for efficiency, assuming canonical text is roughly the same size as normalized input.
    let mut canonical_text = String::with_capacity(normalized_text.len());
    let mut tokens: Vec<Token> = Vec::with_capacity((normalized_text.len() / 4).saturating_add(1));
    // State machine for tokenization and whitespace collapsing.
    let mut pending_space = false;
    let mut current_token_start: Option<usize> = None;

    process_chars(
        normalized_text.as_ref(),
        cfg,
        &mut canonical_text,
        &mut tokens,
        &mut pending_space,
        &mut current_token_start,
    );

    // The last token needs to be finalized after the loop.
    finalize_token(&mut tokens, &canonical_text, &mut current_token_start);

    // After all transformations, if the text is empty, it's an error.
    if canonical_text.is_empty() {
        return Err(CanonicalError::EmptyInput);
    }

    let canonical_version = cfg.version;
    let mut token_hashes: Vec<String> = Vec::with_capacity(tokens.len());
    token_hashes.extend(
        tokens
            .iter()
            .map(|t| hash_token_bytes(canonical_version, t.text.as_bytes())),
    );
    let sha256_hex = hash_canonical_bytes(canonical_version, canonical_text.as_bytes());

    Ok(CanonicalizedDocument {
        doc_id,
        canonical_text,
        tokens,
        token_hashes,
        sha256_hex,
        canonical_version,
        config: cfg.clone(),
    })
}

/// Helper to iterate over characters and dispatch them for processing.
fn process_chars(
    text: &str,
    cfg: &CanonicalizeConfig,
    canonical_text: &mut String,
    tokens: &mut Vec<Token>,
    pending_space: &mut bool,
    current_token_start: &mut Option<usize>,
) {
    // Process Unicode grapheme clusters properly to handle multi-character emojis and complex scripts
    for grapheme in text.graphemes(true) {
        // Lowercasing can expand a single character into multiple (e.g., German ß -> ss).
        if cfg.lowercase {
            for lower in grapheme.to_lowercase().chars() {
                dispatch_char(
                    lower,
                    cfg,
                    canonical_text,
                    tokens,
                    pending_space,
                    current_token_start,
                );
            }
        } else {
            for ch in grapheme.chars() {
                dispatch_char(
                    ch,
                    cfg,
                    canonical_text,
                    tokens,
                    pending_space,
                    current_token_start,
                );
            }
        }
    }
}

/// Decides whether a character is part of a token or a delimiter.
fn dispatch_char(
    ch: char,
    cfg: &CanonicalizeConfig,
    canonical_text: &mut String,
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
        append_char(ch, canonical_text, current_token_start, pending_space);
    }
}

/// Appends a character to the canonical text.
fn append_char(
    ch: char,
    canonical_text: &mut String,
    current_token_start: &mut Option<usize>,
    pending_space: &mut bool,
) {
    // If a space is pending, add it and mark the start of a new token.
    if *pending_space {
        canonical_text.push(' ');
        *pending_space = false;
        *current_token_start = Some(canonical_text.len());
    } else if current_token_start.is_none() {
        // If there's no token in progress, start a new one.
        *current_token_start = Some(canonical_text.len());
    }

    // Append the character to the canonical text.
    canonical_text.push(ch);
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
