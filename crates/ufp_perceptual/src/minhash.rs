//! MinHash computation for UCFP perceptual fingerprinting.
//!
//! This module implements fixed-length MinHash signatures over sets of
//! shingle hashes. The implementation is deterministic and uses a family of
//! hash functions derived from a single 64‑bit seed.

use rayon::prelude::*;
use xxhash_rust::xxh3::xxh3_64_with_seed;

use crate::config::PerceptualConfig;

/// Compute a MinHash signature (parallel if `cfg.use_parallel = true`).
///
/// This creates `m` hash values, where each value is the minimum of the hashes
/// of the unique shingles after being permuted by a different hash function.
pub fn minhash_signature(unique_shingles: &[u64], m: usize, cfg: &PerceptualConfig) -> Vec<u64> {
    if m == 0 {
        return Vec::new();
    }

    if cfg.use_parallel {
        (0..m)
            .into_par_iter()
            .map(|j| compute_slot(unique_shingles, j, cfg.seed))
            .collect()
    } else {
        (0..m)
            .map(|j| compute_slot(unique_shingles, j, cfg.seed))
            .collect()
    }
}

/// Computes a single slot in the MinHash signature.
#[inline]
pub(crate) fn compute_slot(unique_shingles: &[u64], j: usize, seed: u64) -> u64 {
    // Each slot uses a different key for the hash function to simulate a different permutation.
    let step = (j as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let key = splitmix64(seed.wrapping_add(step));
    let mut minv = u64::MAX;
    // Find the minimum hash value among all shingles for this permutation.
    for &val in unique_shingles {
        let h = mix_u64(val, key);
        if h < minv {
            minv = h;
        }
    }
    minv
}

/// A mixing function to create a new hash from an existing one.
#[inline]
pub(crate) fn mix_u64(x: u64, key: u64) -> u64 {
    // This uses a combination of multiplication and XOR shifts to create a well-distributed hash.
    let mut h = xxh3_64_with_seed(&x.to_le_bytes(), key);
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51afd7ed558ccd);
    h ^= h >> 33;
    h = h.wrapping_mul(0xc4ceb9fe1a85ec53);
    h ^ (h >> 33)
}

/// A 64-bit hash function that is fast and has good distribution.
#[inline]
pub(crate) fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}
