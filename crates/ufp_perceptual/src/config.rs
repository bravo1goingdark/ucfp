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

impl PerceptualConfig {
    /// Create a new configuration with sensible defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the shingle size (k). Typical values: 5-15.
    /// Larger k = more context, less noise tolerant. Smaller k = more noise tolerant.
    pub fn with_k(mut self, k: usize) -> Self {
        self.k = k;
        self
    }

    /// Set the winnowing window size (w). Typical values: 2-10.
    /// Larger w = fewer shingles, more selective. Smaller w = more shingles, less selective.
    pub fn with_w(mut self, w: usize) -> Self {
        self.w = w;
        self
    }

    /// Set the number of MinHash bands. Typical values: 8-32.
    /// More bands = higher recall, lower precision. Fewer bands = lower recall, higher precision.
    pub fn with_minhash_bands(mut self, bands: usize) -> Self {
        self.minhash_bands = bands;
        self
    }

    /// Set the number of rows per MinHash band. Typical values: 4-16.
    /// Affects false positive probability in LSH.
    pub fn with_minhash_rows_per_band(mut self, rows: usize) -> Self {
        self.minhash_rows_per_band = rows;
        self
    }

    /// Set the random seed for reproducible results.
    /// Default uses a well-known seed for consistency.
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Enable or disable parallel processing.
    /// Parallel is faster for large datasets but uses more CPU.
    pub fn with_parallel(mut self, use_parallel: bool) -> Self {
        self.use_parallel = use_parallel;
        self
    }

    /// Include or exclude intermediate processing results.
    /// Useful for debugging and analysis of the fingerprinting process.
    pub fn with_intermediates(mut self, include_intermediates: bool) -> Self {
        self.include_intermediates = include_intermediates;
        self
    }

    /// Validate configuration parameters.
    pub fn validate(&self) -> Result<(), PerceptualError> {
        if self.k < 1 {
            return Err(PerceptualError::InvalidConfigK { k: self.k });
        }
        if self.w < 1 {
            return Err(PerceptualError::InvalidConfigW { w: self.w });
        }
        if self.minhash_bands < 1 {
            return Err(PerceptualError::InvalidConfigBands {
                bands: self.minhash_bands,
            });
        }
        if self.minhash_rows_per_band < 1 {
            return Err(PerceptualError::InvalidConfigRows {
                rows: self.minhash_rows_per_band,
            });
        }
        if self.version < 1 {
            return Err(PerceptualError::InvalidConfigVersion {
                version: self.version,
            });
        }

        // Check for minhash length overflow
        let expected_length = self.minhash_bands * self.minhash_rows_per_band;
        if expected_length < self.k {
            return Err(PerceptualError::InvalidConfigMinhashLength {
                bands: self.minhash_bands,
                rows: self.minhash_rows_per_band,
            });
        }

        Ok(())
    }
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
