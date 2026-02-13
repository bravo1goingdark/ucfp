//! Whitespace normalization utilities.
//!
//! This module provides functions for normalizing whitespace in text,
//! primarily the [`collapse_whitespace`] function which collapses consecutive
//! whitespace characters into single spaces.
//!
//! # Whitespace Definition
//!
//! This crate uses Unicode's definition of whitespace, which includes:
//! - ASCII space (U+0020)
//! - ASCII tab (U+0009)
//! - ASCII newline (U+000A)
//! - ASCII carriage return (U+000D)
//! - And many other Unicode whitespace characters
//!
//! # Algorithm
//!
//! The whitespace collapsing algorithm:
//! 1. Split text on any Unicode whitespace sequence
//! 2. Join segments with single ASCII spaces
//! 3. Result has no leading/trailing whitespace
//!
//! # Examples
//!
//! ```rust
//! use canonical::collapse_whitespace;
//!
//! let normalized = collapse_whitespace("  hello   world  ");
//! assert_eq!(normalized, "hello world");
//! ```

/// Collapses repeated whitespace, trims edges, and normalizes newlines to
/// single spaces.
///
/// This utility is deterministic and primarily useful for callers that need
/// whitespace-normalized text without running the full canonical pipeline.
///
/// # Algorithm
///
/// 1. Split the text on any Unicode whitespace (using `split_whitespace()`)
/// 2. Join the resulting segments with single ASCII spaces
/// 3. The result has no leading or trailing whitespace
///
/// # Arguments
///
/// * `text` - The text to normalize
///
/// # Returns
///
/// A new `String` with whitespace collapsed. Returns empty string if input
/// is empty or whitespace-only.
///
/// # Whitespace Characters
///
/// All Unicode whitespace characters are treated as delimiters:
/// - Space (U+0020)
/// - Tab (U+0009)
/// - Newline (U+000A)
/// - Carriage return (U+000D)
/// - Non-breaking space (U+00A0)
/// - And other Unicode whitespace
///
/// # Examples
///
/// ## Basic Usage
///
/// ```rust
/// use canonical::collapse_whitespace;
///
/// // Multiple spaces
/// let result = collapse_whitespace("hello   world");
/// assert_eq!(result, "hello world");
///
/// // Leading/trailing whitespace
/// let result = collapse_whitespace("  hello world  ");
/// assert_eq!(result, "hello world");
///
/// // Mixed whitespace
/// let result = collapse_whitespace("hello\t\t\tworld");
/// assert_eq!(result, "hello world");
/// ```
///
/// ## Newlines and Complex Whitespace
///
/// ```rust
/// use canonical::collapse_whitespace;
///
/// // Newlines
/// let result = collapse_whitespace("hello\n\n\nworld");
/// assert_eq!(result, "hello world");
///
/// // Mixed tabs and spaces
/// let result = collapse_whitespace("hello \t \t world");
/// assert_eq!(result, "hello world");
///
/// // Carriage returns (Windows line endings)
/// let result = collapse_whitespace("hello\r\nworld");
/// assert_eq!(result, "hello world");
/// ```
///
/// ## Edge Cases
///
/// ```rust
/// use canonical::collapse_whitespace;
///
/// // Empty string
/// let result = collapse_whitespace("");
/// assert_eq!(result, "");
///
/// // Whitespace-only
/// let result = collapse_whitespace("   \n\t   ");
/// assert_eq!(result, "");
///
/// // Single word
/// let result = collapse_whitespace("hello");
/// assert_eq!(result, "hello");
///
/// // Already normalized
/// let result = collapse_whitespace("hello world");
/// assert_eq!(result, "hello world");
/// ```
///
/// ## Unicode Whitespace
///
/// ```rust
/// use canonical::collapse_whitespace;
///
/// // Various Unicode whitespace characters
/// let result = collapse_whitespace("hello\u{00A0}world"); // Non-breaking space
/// assert_eq!(result, "hello world");
/// ```
///
/// # Performance
///
/// - Time complexity: O(n) where n is text length
/// - Space complexity: O(n) for the output string
/// - Pre-allocates capacity equal to input length
///
/// # When to Use
///
/// Use this function when you only need whitespace normalization without
/// full canonicalization (Unicode normalization, case folding, etc.).
///
/// # When Not to Use
///
/// For complete text canonicalization, use [`canonicalize()`](crate::canonicalize)
/// instead, which also performs Unicode normalization, case folding, and
/// produces token-level information.
///
/// # See Also
///
/// - [`canonicalize()`](crate::canonicalize) for full canonicalization
/// - [`tokenize()`](crate::tokenize) for splitting text into tokens
/// - `split_whitespace()` in the standard library
pub fn collapse_whitespace(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    for segment in text.split_whitespace() {
        if !normalized.is_empty() {
            normalized.push(' ');
        }
        normalized.push_str(segment);
    }
    normalized
}
