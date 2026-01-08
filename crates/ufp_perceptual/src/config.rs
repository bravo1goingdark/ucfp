//! Configuration and error types for UCFP perceptual fingerprinting.
//!
//! This module defines the public configuration surface for the perceptual
//! layer. It is intentionally free of any I/O or environment-dependent
//! behavior so that the perceptual pipeline is a pure function of
//! `(canonical_tokens, config)`.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Semantic configuration for the perceptual fingerprinting pipeline.
///
/// The perceptual layer **only** works over canonical token streams produced
/// by upstream components. It must never perform normalization, tokenization,
/// or read ingest metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerceptualConfig {
    /// Configuration schema version.
    ///
    /// Any algorithmic change that can affect the fingerprint must bump this
    /// version, so that old fingerprints remain replayable and comparable.
    pub version: u32,
    /// Number of tokens per shingle (k‑shingling).
    ///
    /// This controls local context sensitivity. Larger values are more robust
    /// to noise but less tolerant to reordering.
    pub k: usize,
    /// Window size for winnowing.
    ///
    /// Determines how aggressively shingle hashes are subsampled. Larger
    /// windows keep fewer shingles.
    pub w: usize,
    /// Number of bands for MinHash.
    pub minhash_bands: usize,
    /// Number of rows per band for MinHash.
    pub minhash_rows_per_band: usize,
    /// Random seed for deterministic hashing.
    ///
    /// When two configs share the same seed, and all other parameters and
    /// canonical tokens are equal, the resulting fingerprints will be
    /// bit‑identical.
    pub seed: u64,
    /// Enable parallel MinHash computation.
    pub use_parallel: bool,
    /// Control whether intermediate artifacts (full shingle stream and
    /// winnowed shingles) are included in the fingerprint.
    ///
    /// When `false`, these are still computed internally but cleared from the
    /// returned struct to minimize memory and storage overhead in production.
    pub include_intermediates: bool,
}

impl Default for PerceptualConfig {
    fn default() -> Self {
        Self {
            version: 1,
            k: 9,
            w: 4,
            minhash_bands: 16,
            minhash_rows_per_band: 8,
            seed: 0xF00D_BAAD_F00D_BAAD,
            use_parallel: false,
            include_intermediates: true,
        }
    }
}

/// Errors returned by the perceptual fingerprinting pipeline.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PerceptualError {
    #[error("not enough tokens for k={k}")]
    NotEnoughTokens { k: usize },

    #[error("invalid config: k must be >= 1 (got {k})")]
    InvalidConfigK { k: usize },

    #[error("invalid config: w must be >= 1 (got {w})")]
    InvalidConfigW { w: usize },

    #[error("invalid config: minhash_bands must be >= 1 (got {bands})")]
    InvalidConfigBands { bands: usize },

    #[error("invalid config: minhash_rows_per_band must be >= 1 (got {rows})")]
    InvalidConfigRows { rows: usize },

    #[error("invalid config version {version}; expected >= 1")]
    InvalidConfigVersion { version: u32 },

    #[error("invalid config: minhash length overflow for bands={bands} rows={rows}")]
    InvalidConfigMinhashLength { bands: usize, rows: usize },
}
