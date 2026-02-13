//! Core canonicalization pipeline implementation.
//!
//! This module contains the main [`canonicalize`] function and supporting
//! internal functions that implement the full canonicalization pipeline.
//!
//! # Pipeline Stages
//!
//! The canonicalization process follows these stages in order:
//!
//! 1. **Configuration Validation**: Ensure version >= 1, doc_id non-empty
//! 2. **Unicode Normalization** (optional): Apply NFKC normalization
//! 3. **Character Processing**: Process grapheme clusters with case folding
//! 4. **Delimiter Handling**: Detect whitespace and punctuation boundaries
//! 5. **Token Assembly**: Build tokens with byte offsets
//! 6. **Hash Computation**: Compute version-aware identity hashes
//!
//! # Implementation Details
//!
//! The pipeline uses a state machine approach for efficiency:
//! - `pending_space`: Tracks whether a space should be inserted before next token
//! - `current_token_start`: Tracks the start byte offset of the current token
//! - Processes text in a single pass where possible
//! - Uses `Cow<str>` to avoid allocations when normalization is disabled
//!
//! # Performance
//!
//! - Single-pass processing for most operations
//! - Pre-allocated vectors based on text length estimates
//! - Minimal allocations when Unicode normalization is disabled
//! - Linear time complexity O(n) where n is input length
//!
//! # Examples
//!
//! ```rust
//! use canonical::{canonicalize, CanonicalizeConfig};
//!
//! let config = CanonicalizeConfig::default();
//! let doc = canonicalize("doc-001", "  Hello   WORLD  ", &config).unwrap();
//!
//! assert_eq!(doc.canonical_text, "hello world");
//! assert_eq!(doc.tokens.len(), 2);
//! ```

use std::borrow::Cow;

use unicode_categories::UnicodeCategories;
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

use crate::config::CanonicalizeConfig;
use crate::document::CanonicalizedDocument;
use crate::error::CanonicalError;
use crate::hash::{hash_canonical_bytes, hash_token_bytes_fast};
use crate::token::Token;

/// Canonicalize text into a deterministic, versioned representation.
///
/// This is the primary entry point for the canonical text pipeline. It takes
/// raw input text and transforms it into a canonical form suitable for
/// fingerprinting, indexing, and comparison.
///
/// # Pipeline
///
/// The canonicalization follows these stages:
/// 1. Validate configuration (version >= 1)
/// 2. Validate document ID (non-empty)
/// 3. Apply Unicode normalization (if enabled)
/// 4. Process characters (case folding, delimiter detection)
/// 5. Collapse whitespace
/// 6. Extract tokens with byte offsets
/// 7. Compute version-aware hashes
///
/// # Arguments
///
/// * `doc_id` - Unique document identifier (required, non-empty)
/// * `input` - Raw input text to canonicalize
/// * `cfg` - Canonicalization configuration
///
/// # Returns
///
/// - `Ok(CanonicalizedDocument)` - Successfully canonicalized document
/// - `Err(CanonicalError)` - Validation or processing error
///
/// # Errors
///
/// - [`CanonicalError::InvalidConfig`] - If `cfg.version == 0`
/// - [`CanonicalError::MissingDocId`] - If `doc_id` is empty or whitespace-only
/// - [`CanonicalError::EmptyInput`] - If canonical text is empty after processing
///
/// # Guarantees
///
/// For the same input text and configuration:
/// - The output is deterministic across all platforms
/// - The output is stable across Rust versions (within reason)
/// - The identity hash uniquely identifies the content
///
/// # Examples
///
/// ## Basic Usage
///
/// ```rust
/// use canonical::{canonicalize, CanonicalizeConfig};
///
/// let config = CanonicalizeConfig::default();
/// let doc = canonicalize("doc-001", "Hello World", &config).unwrap();
///
/// assert_eq!(doc.canonical_text, "hello world");
/// assert_eq!(doc.tokens.len(), 2);
/// assert!(!doc.sha256_hex.is_empty());
/// ```
///
/// ## With Punctuation Stripping
///
/// ```rust
/// use canonical::{canonicalize, CanonicalizeConfig};
///
/// let config = CanonicalizeConfig {
///     strip_punctuation: true,
///     ..Default::default()
/// };
///
/// let doc = canonicalize("doc", "Hello, world!", &config).unwrap();
/// assert_eq!(doc.canonical_text, "hello world");
/// ```
///
/// ## Unicode Normalization
///
/// ```rust
/// use canonical::{canonicalize, CanonicalizeConfig};
///
/// let config = CanonicalizeConfig::default();
///
/// // Different Unicode forms produce same canonical text
/// let doc1 = canonicalize("doc", "Café", &config).unwrap();        // U+00E9
/// let doc2 = canonicalize("doc", "Cafe\u{0301}", &config).unwrap(); // e + accent
///
/// assert_eq!(doc1.canonical_text, doc2.canonical_text);
/// ```
///
/// ## Error Handling
///
/// ```rust
/// use canonical::{canonicalize, CanonicalizeConfig, CanonicalError};
///
/// let config = CanonicalizeConfig::default();
///
/// // Empty document ID
/// match canonicalize("", "hello", &config) {
///     Err(CanonicalError::MissingDocId) => println!("ID required"),
///     _ => {}
/// }
///
/// // Empty input after normalization
/// match canonicalize("doc", "   ", &config) {
///     Err(CanonicalError::EmptyInput) => println!("Empty content"),
///     _ => {}
/// }
/// ```
///
/// # Performance
///
/// - Time: O(n) where n is input length
/// - Space: O(n) for output structures
/// - Minimal allocations when Unicode normalization is disabled
///
/// # See Also
///
/// - [`CanonicalizeConfig`] for configuration options
/// - [`CanonicalizedDocument`] for the output structure
/// - [`CanonicalError`] for error types
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
            .map(|t| hash_token_bytes_fast(canonical_version, t.text.as_bytes())),
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
///
/// This function processes Unicode grapheme clusters to handle multi-character
/// emojis and complex scripts correctly. It applies case folding if configured.
fn process_chars(
    text: &str,
    cfg: &CanonicalizeConfig,
    canonical_text: &mut String,
    tokens: &mut Vec<Token>,
    pending_space: &mut bool,
    current_token_start: &mut Option<usize>,
) {
    // Fast path: ASCII-only text can use simpler processing
    if text.is_ascii() {
        let processed = if cfg.lowercase {
            text.to_ascii_lowercase()
        } else {
            text.to_string()
        };

        for ch in processed.chars() {
            dispatch_char(
                ch,
                cfg,
                canonical_text,
                tokens,
                pending_space,
                current_token_start,
            );
        }
        return;
    }

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
///
/// Delimiters (whitespace and optionally punctuation) trigger token finalization
/// and set the pending space flag. Non-delimiter characters are appended to the
/// current token.
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
///
/// If a space is pending (from a previous delimiter), it is inserted first
/// and a new token is started. Otherwise, the character is appended to the
/// current token.
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
///
/// This finalizes the current token by extracting the text from the current
/// start position to the end of the canonical text. The start position is
/// then cleared.
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
