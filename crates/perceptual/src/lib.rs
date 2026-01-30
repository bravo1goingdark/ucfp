//! # UCFP Perceptual Fingerprinting
//!
//! This crate provides perceptual fingerprinting capabilities for the Universal
//! Content Fingerprinting (UCFP) framework. It takes a stream of canonicalized
//! tokens and generates a compact, similarity-preserving signature that is robust
//! to minor content modifications.
//!
//! ## Contract
//!
//! - The perceptual layer **only** consumes canonical tokens produced by the
//!   upstream canonicalization pipeline.
//! - It never performs normalization, tokenization, or reads ingest metadata.
//! - The API is a pure function of `(canonical_tokens, config)` with no I/O,
//!   no network, and no reliance on clocks or global process state.
//!
//! Invariant: for the same canonical token sequence and the same
//! [`PerceptualConfig`], the perceptual output is a bit identical.
//!
//! ## Core Pipeline
//!
//! The perceptual fingerprinting process consists of three main stages:
//!
//! 1.  **Shingling**: The token stream is converted into a sequence of
//!     overlapping k‑shingles (contiguous subsequences of `k` tokens). Each
//!     shingle is hashed into a 64‑bit integer using a deterministic rolling
//!     hash algorithm. This captures the local structure of the text.
//!
//! 2.  **Winnowing**: To reduce the number of fingerprints while preserving a
//!     guarantee on matching, a winnowing algorithm is applied to the shingle
//!     hashes. It selects a subset of shingles by choosing the minimum hash
//!     value within a sliding window. This significantly reduces the data size
//!     without sacrificing the ability to detect similarities.
//!
//! 3.  **MinHashing**: The set of winnowed shingle hashes is used to generate a
//!     fixed‑size MinHash signature. This signature is a compact representation
//!     of the document's content that can be efficiently compared with other
//!     signatures to estimate Jaccard similarity. The implementation supports
//!     optional parallelism via Rayon for improved performance on large
//!     documents.
//!
//! ## Example Usage
//!
//! ```
//! use perceptual::{perceptualize_tokens, PerceptualConfig};
//!
//! let tokens = vec!["the", "quick", "brown", "fox", "jumps", "over", "the", "lazy", "dog"];
//! let config = PerceptualConfig {
//!     k: 3,
//!     ..Default::default()
//! };
//!
//! let fingerprint = perceptualize_tokens(&tokens, &config).unwrap();
//!
//! assert!(!fingerprint.minhash.is_empty());
//! assert_eq!(fingerprint.meta.k, 3);
//! ```
//!
pub mod config;
pub mod fingerprint;
mod minhash;
mod shingles;

pub use crate::config::{PerceptualConfig, PerceptualError};
pub use crate::fingerprint::{PerceptualFingerprint, PerceptualMeta, WinnowedShingle};
use crate::minhash::minhash_signature;
use crate::shingles::{make_shingles_rolling, winnow_minq};

/// Current perceptual algorithm version for this crate.
pub const PERCEPTUAL_VERSION: u16 = 1;

/// Human‑readable algorithm identifier.
pub const PERCEPTUAL_ALGORITHM: &str = "rollingminqminhash_v1";

/// Compute a perceptual fingerprint (shingles → winnow → MinHash).
///
/// The `tokens` slice must contain canonical tokens in their original order.
/// This function does not perform any normalization or tokenization.
pub fn perceptualize_tokens<S>(
    tokens: &[S],
    cfg: &PerceptualConfig,
) -> Result<PerceptualFingerprint, PerceptualError>
where
    S: AsRef<str>,
{
    // --- Configuration validation ---
    if cfg.version == 0 {
        return Err(PerceptualError::InvalidConfigVersion {
            version: cfg.version,
        });
    }
    if cfg.k == 0 {
        return Err(PerceptualError::InvalidConfigK { k: cfg.k });
    }
    if cfg.w == 0 {
        return Err(PerceptualError::InvalidConfigW { w: cfg.w });
    }
    if cfg.minhash_bands == 0 {
        return Err(PerceptualError::InvalidConfigBands {
            bands: cfg.minhash_bands,
        });
    }
    if cfg.minhash_rows_per_band == 0 {
        return Err(PerceptualError::InvalidConfigRows {
            rows: cfg.minhash_rows_per_band,
        });
    }
    if tokens.len() < cfg.k {
        return Err(PerceptualError::NotEnoughTokens { k: cfg.k });
    }

    // Prevent overflow when calculating total MinHash length.
    let minhash_len = cfg
        .minhash_bands
        .checked_mul(cfg.minhash_rows_per_band)
        .ok_or(PerceptualError::InvalidConfigMinhashLength {
            bands: cfg.minhash_bands,
            rows: cfg.minhash_rows_per_band,
        })?;

    // --- Pipeline ---

    // Step 1: Create rolling-hash shingles from the token stream.
    // This is an O(n) operation that produces a hash for each k-token window.
    let mut shingles = make_shingles_rolling(tokens, cfg.k, cfg.seed);

    // Step 2: Winnow the shingles to select a smaller, representative set of fingerprints.
    // This is also O(n) and helps to reduce the data size while preserving similarity.
    let mut winnowed = winnow_minq(&shingles, cfg.w);

    // Step 3: Deduplicate the selected shingles to get a set of unique hashes for MinHash.
    // If winnowing produced no results (e.g., text was too short), use all shingles.
    let uniq: Vec<u64> = if winnowed.is_empty() {
        let mut hashes = Vec::with_capacity(shingles.len());
        hashes.extend(shingles.iter().copied());
        hashes
    } else {
        let mut hashes: Vec<u64> = Vec::with_capacity(winnowed.len());
        hashes.extend(winnowed.iter().map(|w| w.hash));
        hashes.sort_unstable();
        hashes.dedup();
        hashes
    };

    // Step 4: Compute the MinHash signature from the unique shingles.
    // This produces a fixed-size signature that can be used for LSH-based similarity search.
    let minhash = minhash_signature(&uniq, minhash_len, cfg);

    // Optionally drop intermediate artifacts for cost efficiency.
    if !cfg.include_intermediates {
        shingles.clear();
        winnowed.clear();
    }

    Ok(PerceptualFingerprint {
        shingles,
        winnowed,
        minhash,
        meta: PerceptualMeta {
            perceptual_version: PERCEPTUAL_VERSION,
            algorithm_name: PERCEPTUAL_ALGORITHM.to_string(),
            k: cfg.k,
            w: cfg.w,
            minhash_len,
            minhash_bands: cfg.minhash_bands,
            minhash_rows_per_band: cfg.minhash_rows_per_band,
            seed: cfg.seed,
            use_parallel: cfg.use_parallel,
            config_version: cfg.version,
        },
    })
}
