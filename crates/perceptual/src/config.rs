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
/// by upstream components.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_default_values() {
        let cfg = PerceptualConfig::default();
        assert_eq!(cfg.version, 1);
        assert_eq!(cfg.k, 9);
        assert_eq!(cfg.w, 4);
        assert_eq!(cfg.minhash_bands, 16);
        assert_eq!(cfg.minhash_rows_per_band, 8);
        assert_eq!(cfg.seed, 0xF00D_BAAD_F00D_BAAD);
        assert!(!cfg.use_parallel);
        assert!(cfg.include_intermediates);
    }

    #[test]
    fn config_new_creates_default() {
        let cfg_new = PerceptualConfig::new();
        let cfg_default = PerceptualConfig::default();
        assert_eq!(cfg_new, cfg_default);
    }

    #[test]
    fn config_builder_with_k() {
        let cfg = PerceptualConfig::new().with_k(5);
        assert_eq!(cfg.k, 5);
    }

    #[test]
    fn config_builder_with_w() {
        let cfg = PerceptualConfig::new().with_w(8);
        assert_eq!(cfg.w, 8);
    }

    #[test]
    fn config_builder_with_minhash_bands() {
        let cfg = PerceptualConfig::new().with_minhash_bands(32);
        assert_eq!(cfg.minhash_bands, 32);
    }

    #[test]
    fn config_builder_with_minhash_rows_per_band() {
        let cfg = PerceptualConfig::new().with_minhash_rows_per_band(16);
        assert_eq!(cfg.minhash_rows_per_band, 16);
    }

    #[test]
    fn config_builder_with_seed() {
        let cfg = PerceptualConfig::new().with_seed(12345);
        assert_eq!(cfg.seed, 12345);
    }

    #[test]
    fn config_builder_with_parallel() {
        let cfg = PerceptualConfig::new().with_parallel(true);
        assert!(cfg.use_parallel);
    }

    #[test]
    fn config_builder_with_intermediates() {
        let cfg = PerceptualConfig::new().with_intermediates(false);
        assert!(!cfg.include_intermediates);
    }

    #[test]
    fn config_builder_chain() {
        let cfg = PerceptualConfig::new()
            .with_k(3)
            .with_w(2)
            .with_minhash_bands(8)
            .with_minhash_rows_per_band(4)
            .with_seed(42)
            .with_parallel(true)
            .with_intermediates(false);

        assert_eq!(cfg.k, 3);
        assert_eq!(cfg.w, 2);
        assert_eq!(cfg.minhash_bands, 8);
        assert_eq!(cfg.minhash_rows_per_band, 4);
        assert_eq!(cfg.seed, 42);
        assert!(cfg.use_parallel);
        assert!(!cfg.include_intermediates);
    }

    #[test]
    fn config_validate_valid() {
        let cfg = PerceptualConfig::default();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn config_validate_invalid_k_zero() {
        let cfg = PerceptualConfig::new().with_k(0);
        assert!(matches!(
            cfg.validate(),
            Err(PerceptualError::InvalidConfigK { k: 0 })
        ));
    }

    #[test]
    fn config_validate_invalid_w_zero() {
        let cfg = PerceptualConfig::new().with_w(0);
        assert!(matches!(
            cfg.validate(),
            Err(PerceptualError::InvalidConfigW { w: 0 })
        ));
    }

    #[test]
    fn config_validate_invalid_bands_zero() {
        let cfg = PerceptualConfig::new().with_minhash_bands(0);
        assert!(matches!(
            cfg.validate(),
            Err(PerceptualError::InvalidConfigBands { bands: 0 })
        ));
    }

    #[test]
    fn config_validate_invalid_rows_zero() {
        let cfg = PerceptualConfig::new().with_minhash_rows_per_band(0);
        assert!(matches!(
            cfg.validate(),
            Err(PerceptualError::InvalidConfigRows { rows: 0 })
        ));
    }

    #[test]
    fn config_validate_invalid_version_zero() {
        let cfg = PerceptualConfig {
            version: 0,
            ..Default::default()
        };
        assert!(matches!(
            cfg.validate(),
            Err(PerceptualError::InvalidConfigVersion { version: 0 })
        ));
    }

    #[test]
    fn config_clone() {
        let cfg = PerceptualConfig::default();
        let cloned = cfg.clone();
        assert_eq!(cfg, cloned);
    }

    #[test]
    fn config_serde_roundtrip() {
        let cfg = PerceptualConfig::new()
            .with_k(5)
            .with_w(3)
            .with_seed(12345)
            .with_parallel(true);

        let serialized = serde_json::to_string(&cfg).unwrap();
        let deserialized: PerceptualConfig = serde_json::from_str(&serialized).unwrap();

        assert_eq!(cfg, deserialized);
    }

    #[test]
    fn error_display_not_enough_tokens() {
        let err = PerceptualError::NotEnoughTokens { k: 9 };
        assert!(err.to_string().contains("not enough tokens"));
        assert!(err.to_string().contains("k=9"));
    }

    #[test]
    fn error_display_invalid_config_k() {
        let err = PerceptualError::InvalidConfigK { k: 0 };
        assert!(err.to_string().contains("invalid config"));
        assert!(err.to_string().contains("k must be >= 1"));
    }

    #[test]
    fn error_clone() {
        let err = PerceptualError::NotEnoughTokens { k: 5 };
        let cloned = err.clone();
        assert_eq!(format!("{err}"), format!("{}", cloned));
    }

    #[test]
    fn error_partial_eq() {
        let err1 = PerceptualError::InvalidConfigK { k: 0 };
        let err2 = PerceptualError::InvalidConfigK { k: 0 };
        let err3 = PerceptualError::InvalidConfigW { w: 0 };

        assert_eq!(err1, err2);
        assert_ne!(err1, err3);
    }
}
