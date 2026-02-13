//! Tokenization utilities for the canonical text pipeline.
//!
//! This module defines [`Token`] and provides the [`tokenize`] function for
//! splitting canonical text into tokens with byte offsets.
//!
//! # Token Structure
//!
//! A `Token` represents a contiguous sequence of non-whitespace characters
//! in the canonical text, with byte offsets for precise positioning.
//!
//! ```text
//! Token {
//!     text: String,  // The token text content
//!     start: usize,  // Byte offset (inclusive)
//!     end: usize,    // Byte offset (exclusive)
//! }
//! ```
//!
//! # Byte Offsets
//!
//! Offsets are **byte positions**, not character positions. This is important
//! for multi-byte UTF-8 characters. For example:
//!
//! ```text
//! Text: "hello café"
//! Bytes: h e l l o   c a f é
//!        01234567890123456789
//!
//! Token "café" has start=6, end=11 (5 bytes for 4 characters)
//! ```
//!
//! # Examples
//!
//! ```rust
//! use canonical::tokenize;
//!
//! let tokens = tokenize("hello world");
//! assert_eq!(tokens.len(), 2);
//! assert_eq!(tokens[0].text, "hello");
//! assert_eq!(tokens[1].text, "world");
//! ```

use serde::{Deserialize, Serialize};

/// A token with its UTF-8 byte offsets in the canonical text.
///
/// `Token` represents a single token extracted from canonical text. It contains
/// the token text and byte offsets indicating its position in the original
/// canonical text string.
///
/// # Structure
///
/// - `text`: The token's text content
/// - `start`: Byte offset (inclusive) in the canonical text
/// - `end`: Byte offset (exclusive) in the canonical text
///
/// # Byte vs Character Offsets
///
/// Offsets are byte positions, not character positions. This is crucial for
/// correct handling of multi-byte UTF-8 characters:
///
/// ```text
/// Text: "café" (4 characters, 5 bytes in UTF-8)
/// Bytes: c a f é
///        01234
///
/// Token "café" has start=0, end=5
/// ```
///
/// # Examples
///
/// ```rust
/// use canonical::tokenize;
///
/// let tokens = tokenize("hello world");
/// let token = &tokens[0];
///
/// assert_eq!(token.text, "hello");
/// assert_eq!(token.start, 0);
/// assert_eq!(token.end, 5);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Token {
    /// The token text content.
    ///
    /// This is the actual text of the token, extracted from the canonical
    /// text using the byte offsets.
    ///
    /// # Example
    ///
    /// ```rust
    /// use canonical::tokenize;
    ///
    /// let tokens = tokenize("hello world");
    /// assert_eq!(tokens[0].text, "hello");
    /// assert_eq!(tokens[1].text, "world");
    /// ```
    pub text: String,

    /// Byte offset (inclusive) in the canonical text.
    ///
    /// This is the starting byte position of the token in the original
    /// canonical text string. It is measured in bytes, not characters.
    ///
    /// # Example
    ///
    /// ```rust
    /// use canonical::tokenize;
    ///
    /// let tokens = tokenize("hello world");
    /// assert_eq!(tokens[0].start, 0);   // "hello" starts at byte 0
    /// assert_eq!(tokens[1].start, 6);   // "world" starts at byte 6 (after "hello ")
    /// ```
    pub start: usize,

    /// Byte offset (exclusive) in the canonical text.
    ///
    /// This is the ending byte position of the token in the original
    /// canonical text string. It points to the byte immediately after
    /// the last byte of the token.
    ///
    /// # Invariant
    ///
    /// `end > start` for all valid tokens (non-empty text).
    /// `end - start == text.len()` (byte length matches text).
    ///
    /// # Example
    ///
    /// ```rust
    /// use canonical::tokenize;
    ///
    /// let tokens = tokenize("hello world");
    /// assert_eq!(tokens[0].end, 5);   // "hello" ends at byte 5
    /// assert_eq!(tokens[1].end, 11);  // "world" ends at byte 11
    ///
    /// // Verify: end - start == text length
    /// assert_eq!(tokens[0].end - tokens[0].start, tokens[0].text.len());
    /// ```
    pub end: usize,
}

impl AsRef<str> for Token {
    /// Returns the token text as a string slice.
    ///
    /// # Example
    ///
    /// ```rust
    /// use canonical::tokenize;
    ///
    /// let tokens = tokenize("hello");
    /// let text: &str = tokens[0].as_ref();
    /// assert_eq!(text, "hello");
    /// ```
    fn as_ref(&self) -> &str {
        self.text.as_str()
    }
}

impl Token {
    /// Returns the length of the token text in bytes.
    ///
    /// # Example
    ///
    /// ```rust
    /// use canonical::tokenize;
    ///
    /// let tokens = tokenize("hello café");
    /// assert_eq!(tokens[0].len(), 5);  // "hello" = 5 bytes
    /// assert_eq!(tokens[1].len(), 5);  // "café" = 5 bytes (4 chars + 1 for é)
    /// ```
    pub fn len(&self) -> usize {
        self.text.len()
    }

    /// Returns true if the token text is empty.
    ///
    /// Note: Valid tokens from the canonicalization pipeline should never be
    /// empty, but this method is provided for completeness.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Returns the byte range `[start, end)` as a tuple.
    ///
    /// # Example
    ///
    /// ```rust
    /// use canonical::tokenize;
    ///
    /// let tokens = tokenize("hello world");
    /// assert_eq!(tokens[0].range(), (0, 5));
    /// ```
    pub fn range(&self) -> (usize, usize) {
        (self.start, self.end)
    }
}

/// Tokenizes canonical text and produces byte offsets.
///
/// This function splits text into tokens separated by Unicode whitespace.
/// It is deterministic and produces consistent results across platforms.
///
/// # Algorithm
///
/// 1. Scan the text character by character
/// 2. On non-whitespace: begin or continue a token
/// 3. On whitespace: finalize current token (if any)
/// 4. After scan: finalize any remaining token
///
/// # Arguments
///
/// * `text` - The canonical text to tokenize
///
/// # Returns
///
/// A vector of `Token` structs with byte offsets.
///
/// # Examples
///
/// ```rust
/// use canonical::tokenize;
///
/// let tokens = tokenize("hello world");
/// assert_eq!(tokens.len(), 2);
/// assert_eq!(tokens[0].text, "hello");
/// assert_eq!(tokens[1].text, "world");
/// ```
///
/// ## Multi-byte Characters
///
/// ```rust
/// use canonical::tokenize;
///
/// let tokens = tokenize("café naïve");
/// assert_eq!(tokens.len(), 2);
///
/// // "café" is 5 bytes (c-a-f-é where é is 2 bytes)
/// assert_eq!(tokens[0].text, "café");
/// assert_eq!(tokens[0].start, 0);
/// assert_eq!(tokens[0].end, 5);
///
/// // "naïve" is 6 bytes (n-a-ï-v-e where ï is 2 bytes)
/// assert_eq!(tokens[1].text, "naïve");
/// assert_eq!(tokens[1].start, 6);  // After "café " (6 bytes)
/// ```
///
/// ## Empty Input
///
/// ```rust
/// use canonical::tokenize;
///
/// let tokens = tokenize("");
/// assert!(tokens.is_empty());
///
/// let tokens = tokenize("   ");  // Whitespace only
/// assert!(tokens.is_empty());
/// ```
///
/// # Performance
///
/// - Time complexity: O(n) where n is text length
/// - Space complexity: O(n) for the tokens vector
/// - Pre-allocates vector capacity based on text length estimate
///
/// # See Also
///
/// - [`CanonicalizedDocument`](crate::CanonicalizedDocument) which uses this internally
/// - [`collapse_whitespace`](crate::collapse_whitespace) for whitespace normalization
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
