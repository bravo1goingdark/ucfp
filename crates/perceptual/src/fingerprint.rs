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
