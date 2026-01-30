//! Fingerprint and metadata types for UCFP perceptual layer.
//!
//! This module defines the public fingerprint representation produced by the
//! perceptual stage. The fingerprint schema and metadata are part of the
//! public contract: any incompatible change must result in a new
//! `perceptual_version`.

use serde::{Deserialize, Serialize};

/// Selected winnowed shingle with its originating position.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WinnowedShingle {
    /// Shingle hash value.
    pub hash: u64,
    /// Index of the first token in the original token stream that
    /// contributed to this shingle.
    pub start_idx: usize,
}

/// Final perceptual fingerprint artifact.
///
/// The fingerprint is produced **only** from canonical token streams and a
/// [`crate::config::PerceptualConfig`]. No ingest metadata or raw payload
/// state is consulted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerceptualFingerprint {
    /// All rolling‑hash shingles over the input token stream.
    ///
    /// In production deployments these may be omitted (emptied) depending on
    /// configuration to reduce memory/storage footprint.
    pub shingles: Vec<u64>,
    /// Winnowed shingles selected via the monotonic‑deque algorithm.
    pub winnowed: Vec<WinnowedShingle>,
    /// Fixed‑length MinHash signature derived from the unique shingles.
    pub minhash: Vec<u64>,
    /// Metadata describing how and with which configuration the fingerprint
    /// was produced.
    pub meta: PerceptualMeta,
}

/// Metadata for traceability and determinism.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerceptualMeta {
    /// Perceptual algorithm version.
    ///
    /// This value is owned by the perceptual crate and must be bumped whenever
    /// the effective algorithm (shingling, winnowing, MinHash mapping) changes
    /// in a way that can affect fingerprints.
    pub perceptual_version: u16,
    /// Human‑readable algorithm identifier (e.g. "rolling+minq+minhash_v1").
    /// Stored as an owned string to keep serialization simple.
    pub algorithm_name: String,
    /// Shingle length in tokens.
    pub k: usize,
    /// Winnowing window size.
    pub w: usize,
    /// Total MinHash length (bands × rows).
    pub minhash_len: usize,
    /// Number of MinHash bands.
    pub minhash_bands: usize,
    /// Number of rows per band.
    pub minhash_rows_per_band: usize,
    /// Hash seed used for token hashing and MinHash permutations.
    pub seed: u64,
    /// Whether MinHash was computed using the parallel implementation.
    pub use_parallel: bool,
    /// Configuration schema version that was supplied when computing this
    /// fingerprint.
    pub config_version: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn winnowed_shingle_creation() {
        let shingle = WinnowedShingle {
            hash: 123456789u64,
            start_idx: 42,
        };
        assert_eq!(shingle.hash, 123456789u64);
        assert_eq!(shingle.start_idx, 42);
    }

    #[test]
    fn winnowed_shingle_clone() {
        let shingle = WinnowedShingle {
            hash: 123456789u64,
            start_idx: 42,
        };
        let cloned = shingle.clone();
        assert_eq!(shingle, cloned);
    }

    #[test]
    fn winnowed_shingle_serde_roundtrip() {
        let shingle = WinnowedShingle {
            hash: 987654321u64,
            start_idx: 100,
        };

        let serialized = serde_json::to_string(&shingle).unwrap();
        let deserialized: WinnowedShingle = serde_json::from_str(&serialized).unwrap();

        assert_eq!(shingle, deserialized);
    }

    #[test]
    fn perceptual_fingerprint_creation() {
        let fingerprint = PerceptualFingerprint {
            shingles: vec![1u64, 2u64, 3u64],
            winnowed: vec![
                WinnowedShingle {
                    hash: 1u64,
                    start_idx: 0,
                },
                WinnowedShingle {
                    hash: 2u64,
                    start_idx: 1,
                },
            ],
            minhash: vec![100u64, 200u64, 300u64],
            meta: PerceptualMeta {
                perceptual_version: 1,
                algorithm_name: "test".to_string(),
                k: 3,
                w: 2,
                minhash_len: 3,
                minhash_bands: 1,
                minhash_rows_per_band: 3,
                seed: 42,
                use_parallel: false,
                config_version: 1,
            },
        };

        assert_eq!(fingerprint.shingles.len(), 3);
        assert_eq!(fingerprint.winnowed.len(), 2);
        assert_eq!(fingerprint.minhash.len(), 3);
        assert_eq!(fingerprint.meta.k, 3);
    }

    #[test]
    fn perceptual_fingerprint_clone() {
        let fingerprint = PerceptualFingerprint {
            shingles: vec![1u64],
            winnowed: vec![WinnowedShingle {
                hash: 1u64,
                start_idx: 0,
            }],
            minhash: vec![100u64],
            meta: PerceptualMeta {
                perceptual_version: 1,
                algorithm_name: "test".to_string(),
                k: 1,
                w: 1,
                minhash_len: 1,
                minhash_bands: 1,
                minhash_rows_per_band: 1,
                seed: 42,
                use_parallel: false,
                config_version: 1,
            },
        };

        let cloned = fingerprint.clone();
        assert_eq!(fingerprint, cloned);
    }

    #[test]
    fn perceptual_fingerprint_serde_roundtrip() {
        let fingerprint = PerceptualFingerprint {
            shingles: vec![1u64, 2u64, 3u64],
            winnowed: vec![WinnowedShingle {
                hash: 2u64,
                start_idx: 1,
            }],
            minhash: vec![100u64, 200u64],
            meta: PerceptualMeta {
                perceptual_version: 1,
                algorithm_name: "test_algo".to_string(),
                k: 3,
                w: 2,
                minhash_len: 2,
                minhash_bands: 1,
                minhash_rows_per_band: 2,
                seed: 12345,
                use_parallel: true,
                config_version: 2,
            },
        };

        let serialized = serde_json::to_string(&fingerprint).unwrap();
        let deserialized: PerceptualFingerprint = serde_json::from_str(&serialized).unwrap();

        assert_eq!(fingerprint, deserialized);
    }

    #[test]
    fn perceptual_meta_creation() {
        let meta = PerceptualMeta {
            perceptual_version: 2,
            algorithm_name: "rolling+minq+minhash_v2".to_string(),
            k: 5,
            w: 4,
            minhash_len: 128,
            minhash_bands: 16,
            minhash_rows_per_band: 8,
            seed: 0xDEADBEEF,
            use_parallel: true,
            config_version: 3,
        };

        assert_eq!(meta.perceptual_version, 2);
        assert_eq!(meta.algorithm_name, "rolling+minq+minhash_v2");
        assert_eq!(meta.k, 5);
        assert_eq!(meta.w, 4);
        assert_eq!(meta.minhash_len, 128);
        assert_eq!(meta.minhash_bands, 16);
        assert_eq!(meta.minhash_rows_per_band, 8);
        assert_eq!(meta.seed, 0xDEADBEEF);
        assert!(meta.use_parallel);
        assert_eq!(meta.config_version, 3);
    }

    #[test]
    fn perceptual_meta_clone() {
        let meta = PerceptualMeta {
            perceptual_version: 1,
            algorithm_name: "test".to_string(),
            k: 3,
            w: 2,
            minhash_len: 16,
            minhash_bands: 2,
            minhash_rows_per_band: 8,
            seed: 42,
            use_parallel: false,
            config_version: 1,
        };

        let cloned = meta.clone();
        assert_eq!(meta, cloned);
    }

    #[test]
    fn perceptual_meta_serde_roundtrip() {
        let meta = PerceptualMeta {
            perceptual_version: 1,
            algorithm_name: "test".to_string(),
            k: 9,
            w: 4,
            minhash_len: 128,
            minhash_bands: 16,
            minhash_rows_per_band: 8,
            seed: 0xF00D_BAAD_F00D_BAAD,
            use_parallel: false,
            config_version: 1,
        };

        let serialized = serde_json::to_string(&meta).unwrap();
        let deserialized: PerceptualMeta = serde_json::from_str(&serialized).unwrap();

        assert_eq!(meta, deserialized);
    }

    #[test]
    fn partial_equality_winnowed_shingle() {
        let s1 = WinnowedShingle {
            hash: 100u64,
            start_idx: 0,
        };
        let s2 = WinnowedShingle {
            hash: 100u64,
            start_idx: 0,
        };
        let s3 = WinnowedShingle {
            hash: 200u64,
            start_idx: 0,
        };

        assert_eq!(s1, s2);
        assert_ne!(s1, s3);
    }

    #[test]
    fn debug_formatting() {
        let shingle = WinnowedShingle {
            hash: 12345u64,
            start_idx: 5,
        };
        let debug_str = format!("{shingle:?}");
        assert!(debug_str.contains("12345"));
        assert!(debug_str.contains("5"));
    }
}
