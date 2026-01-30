use serde::{Deserialize, Serialize};

/// A token with its UTF-8 byte offsets in the canonical text.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Token {
    /// The token text content.
    pub text: String,
    /// Byte offset (inclusive) in the canonical text.
    pub start: usize,
    /// Byte offset (exclusive) in the canonical text.
    pub end: usize,
}

impl AsRef<str> for Token {
    fn as_ref(&self) -> &str {
        self.text.as_str()
    }
}

/// Tokenizes canonical text and produces byte offsets.
///
/// This helper assumes that `text` has already been canonicalized and that
/// tokens are separated by Unicode whitespace. It is deterministic and
/// cross-platform.
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
