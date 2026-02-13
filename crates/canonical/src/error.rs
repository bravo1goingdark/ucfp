//! Error types for the canonical text pipeline.
//!
//! This module defines [`CanonicalError`], the error type for all canonicalization
//! failures. All errors are typed, cloneable, and comparable for precise error
//! handling and testing.
//!
//! # Error Philosophy
//!
//! The canonical crate uses typed errors (not generic strings) to allow callers to:
//! - Handle specific error cases differently
//! - Provide user-friendly error messages
//! - Log structured error information
//! - Test error conditions precisely
//!
//! # Error Categories
//!
//! | Error | Category | Description |
//! |-------|----------|-------------|
//! | [`InvalidConfig`](CanonicalError::InvalidConfig) | Configuration | Invalid configuration parameters |
//! | [`MissingDocId`](CanonicalError::MissingDocId) | Validation | Document ID is empty or whitespace-only |
//! | [`EmptyInput`](CanonicalError::EmptyInput) | Validation | Input is empty after normalization |
//!
//! # Examples
//!
//! ## Basic Error Handling
//!
//! ```rust
//! use canonical::{canonicalize, CanonicalizeConfig, CanonicalError};
//!
//! let config = CanonicalizeConfig::default();
//!
//! match canonicalize("doc", "   ", &config) {
//!     Ok(doc) => println!("Success: {}", doc.canonical_text),
//!     Err(CanonicalError::EmptyInput) => {
//!         println!("Document is empty after normalization");
//!     }
//!     Err(e) => println!("Error: {}", e),
//! }
//! ```

use thiserror::Error;

/// Errors that can occur during text canonicalization.
///
/// These errors represent validation or processing failures that prevent
/// successful canonicalization. All variants are:
///
/// - **Cloneable**: Can be copied for error propagation
/// - **Comparable**: Support equality checks for testing
/// - **Displayable**: Implement `std::fmt::Display` for user messages
/// - **Debuggable**: Implement `std::fmt::Debug` for development
///
/// # Error Recovery
///
/// - `InvalidConfig`: Fix the configuration and retry
/// - `MissingDocId`: Provide a non-empty document ID
/// - `EmptyInput`: Skip the document or provide meaningful content
///
/// # Examples
///
/// ```rust
/// use canonical::CanonicalError;
///
/// // Error messages
/// let err = CanonicalError::MissingDocId;
/// assert_eq!(err.to_string(), "canonical document requires a non-empty doc_id");
///
/// let err = CanonicalError::InvalidConfig("version must be >= 1".to_string());
/// assert!(err.to_string().contains("version"));
/// ```
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CanonicalError {
    /// Invalid configuration parameter.
    ///
    /// This error occurs when the [`CanonicalizeConfig`](crate::CanonicalizeConfig)
    /// contains invalid or unsupported values.
    ///
    /// # Common Causes
    ///
    /// - `version == 0` (reserved, must be >= 1)
    /// - Future: unsupported feature combinations
    /// - Future: invalid parameter combinations
    ///
    /// # Solutions
    ///
    /// - Check that `version >= 1`
    /// - Validate configuration before canonicalization
    /// - Review configuration documentation
    ///
    /// # Example
    ///
    /// ```rust
    /// use canonical::{canonicalize, CanonicalizeConfig, CanonicalError};
    ///
    /// let bad_config = CanonicalizeConfig {
    ///     version: 0,  // Invalid!
    ///     ..Default::default()
    /// };
    ///
    /// match canonicalize("doc", "hello", &bad_config) {
    ///     Err(CanonicalError::InvalidConfig(msg)) => {
    ///         println!("Configuration error: {}", msg);
    ///     }
    ///     _ => println!("Other result"),
    /// }
    /// ```
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    /// Document ID is missing or empty.
    ///
    /// This error occurs when the `doc_id` parameter is empty or contains
    /// only whitespace after trimming.
    ///
    /// # Why Required
    ///
    /// The document ID is essential for:
    /// - Traceability in logs and metrics
    /// - Referencing canonicalized content
    /// - Debugging and audit trails
    /// - Downstream processing
    ///
    /// # Solutions
    ///
    /// - Provide a non-empty document identifier
    /// - Use a UUID if no natural ID exists
    /// - Ensure ID is trimmed before passing
    ///
    /// # Example
    ///
    /// ```rust
    /// use canonical::{canonicalize, CanonicalizeConfig, CanonicalError};
    ///
    /// let config = CanonicalizeConfig::default();
    ///
    /// // Empty ID
    /// match canonicalize("", "hello", &config) {
    ///     Err(CanonicalError::MissingDocId) => println!("ID is required"),
    ///     _ => println!("Other result"),
    /// }
    ///
    /// // Whitespace-only ID
    /// match canonicalize("   ", "hello", &config) {
    ///     Err(CanonicalError::MissingDocId) => println!("ID cannot be whitespace"),
    ///     _ => println!("Other result"),
    /// }
    /// ```
    #[error("canonical document requires a non-empty doc_id")]
    MissingDocId,

    /// Input text is empty after normalization.
    ///
    /// This error occurs when the input text becomes empty after applying
    /// all canonicalization transformations (normalization, stripping, etc.).
    ///
    /// # Common Causes
    ///
    /// - Input is whitespace-only
    /// - All characters were stripped (e.g., only punctuation with `strip_punctuation: true`)
    /// - Input was empty to begin with
    ///
    /// # Solutions
    ///
    /// - Skip empty documents
    /// - Provide meaningful error messages to users
    /// - Check input before canonicalization
    /// - Consider whether `strip_punctuation` should be disabled
    ///
    /// # Example
    ///
    /// ```rust
    /// use canonical::{canonicalize, CanonicalizeConfig, CanonicalError};
    ///
    /// let config = CanonicalizeConfig::default();
    ///
    /// // Whitespace-only
    /// match canonicalize("doc", "   \n\t   ", &config) {
    ///     Err(CanonicalError::EmptyInput) => println!("Document is empty"),
    ///     _ => println!("Other result"),
    /// }
    ///
    /// // Only punctuation with stripping enabled
    /// let strip_config = CanonicalizeConfig {
    ///     strip_punctuation: true,
    ///     ..Default::default()
    /// };
    /// match canonicalize("doc", "!@#$%", &strip_config) {
    ///     Err(CanonicalError::EmptyInput) => println!("Only punctuation was removed"),
    ///     _ => println!("Other result"),
    /// }
    /// ```
    #[error("input text empty after normalization")]
    EmptyInput,
}

impl CanonicalError {
    /// Returns true if this error indicates a client-side issue.
    ///
    /// All canonical errors are client-side issues (invalid input), so this
    /// always returns true.
    ///
    /// # Example
    ///
    /// ```rust
    /// use canonical::CanonicalError;
    ///
    /// let err = CanonicalError::EmptyInput;
    /// assert!(err.is_client_error());
    /// ```
    pub fn is_client_error(&self) -> bool {
        true
    }
}
