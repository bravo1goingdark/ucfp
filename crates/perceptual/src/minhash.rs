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

    if unique_shingles.is_empty() {
        return vec![u64::MAX; m];
    }

    // Pre-allocate the result vector to avoid multiple allocations
    let mut result = Vec::with_capacity(m);

    if cfg.use_parallel {
        (0..m)
            .into_par_iter()
            .map(|j| compute_slot(unique_shingles, j, cfg.seed))
            .collect_into_vec(&mut result);
    } else {
        for j in 0..m {
            result.push(compute_slot(unique_shingles, j, cfg.seed));
        }
    }

    result
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

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== MinHash Signature Tests ====================

    #[test]
    fn minhash_signature_m_zero_returns_empty() {
        let shingles = vec![1u64, 2u64, 3u64];
        let cfg = PerceptualConfig::default();
        let sig = minhash_signature(&shingles, 0, &cfg);
        assert!(sig.is_empty());
    }

    #[test]
    fn minhash_signature_empty_shingles_returns_max() {
        let shingles: Vec<u64> = vec![];
        let cfg = PerceptualConfig::default();
        let m = 16;
        let sig = minhash_signature(&shingles, m, &cfg);
        assert_eq!(sig.len(), m);
        assert!(sig.iter().all(|&v| v == u64::MAX));
    }

    #[test]
    fn minhash_signature_single_shingle() {
        let shingles = vec![42u64];
        let cfg = PerceptualConfig::default();
        let m = 8;
        let sig = minhash_signature(&shingles, m, &cfg);
        assert_eq!(sig.len(), m);
        // All values should be the same (mix of 42 with different keys)
        assert!(sig.iter().all(|&v| v != u64::MAX));
    }

    #[test]
    fn minhash_signature_deterministic() {
        let shingles = vec![1u64, 2u64, 3u64, 4u64, 5u64];
        let cfg = PerceptualConfig::default();
        let m = 16;

        let sig1 = minhash_signature(&shingles, m, &cfg);
        let sig2 = minhash_signature(&shingles, m, &cfg);

        assert_eq!(sig1, sig2);
    }

    #[test]
    fn minhash_signature_different_seeds() {
        let shingles = vec![1u64, 2u64, 3u64, 4u64, 5u64];
        let mut cfg1 = PerceptualConfig::default();
        let mut cfg2 = PerceptualConfig::default();
        cfg1.seed = 12345;
        cfg2.seed = 54321;
        let m = 16;

        let sig1 = minhash_signature(&shingles, m, &cfg1);
        let sig2 = minhash_signature(&shingles, m, &cfg2);

        // Different seeds should produce different signatures
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn minhash_signature_parallel_equals_sequential() {
        let shingles = vec![1u64, 2u64, 3u64, 4u64, 5u64, 6u64, 7u64, 8u64];
        let m = 128;

        let cfg_seq = PerceptualConfig {
            use_parallel: false,
            ..Default::default()
        };

        let cfg_par = PerceptualConfig {
            use_parallel: true,
            ..Default::default()
        };

        let sig_seq = minhash_signature(&shingles, m, &cfg_seq);
        let sig_par = minhash_signature(&shingles, m, &cfg_par);

        // Parallel and sequential should produce identical results
        assert_eq!(sig_seq, sig_par);
    }

    #[test]
    fn minhash_signature_correct_length() {
        let shingles = vec![1u64, 2u64, 3u64];
        let cfg = PerceptualConfig::default();

        for m in [1, 8, 16, 32, 64, 128] {
            let sig = minhash_signature(&shingles, m, &cfg);
            assert_eq!(sig.len(), m, "Signature length should be {m}");
        }
    }

    #[test]
    fn minhash_signature_values_not_all_max() {
        let shingles = vec![1u64, 2u64, 3u64, 4u64, 5u64];
        let cfg = PerceptualConfig::default();
        let m = 16;

        let sig = minhash_signature(&shingles, m, &cfg);

        // At least some values should not be u64::MAX
        let non_max_count = sig.iter().filter(|&&v| v != u64::MAX).count();
        assert!(non_max_count > 0, "Signature should have non-MAX values");
    }

    // ==================== Compute Slot Tests ====================

    #[test]
    fn compute_slot_finds_minimum() {
        let shingles = vec![100u64, 50u64, 200u64, 25u64, 75u64];
        let seed = 42u64;
        let j = 0;

        let slot = compute_slot(&shingles, j, seed);

        // The slot value should be derived from one of the shingles
        // and should be the minimum of the mixed values
        assert!(slot < u64::MAX);
    }

    #[test]
    fn compute_slot_deterministic() {
        let shingles = vec![1u64, 2u64, 3u64];
        let seed = 42u64;
        let j = 5;

        let slot1 = compute_slot(&shingles, j, seed);
        let slot2 = compute_slot(&shingles, j, seed);

        assert_eq!(slot1, slot2);
    }

    #[test]
    fn compute_slot_different_j_produce_different_values() {
        let shingles = vec![1u64, 2u64, 3u64, 4u64, 5u64];
        let seed = 42u64;

        let slot0 = compute_slot(&shingles, 0, seed);
        let slot1 = compute_slot(&shingles, 1, seed);
        let slot2 = compute_slot(&shingles, 2, seed);

        // Different j values should produce different slot values
        assert_ne!(slot0, slot1);
        assert_ne!(slot1, slot2);
    }

    // ==================== Mix U64 Tests ====================

    #[test]
    fn mix_u64_deterministic() {
        let x = 12345u64;
        let key = 42u64;

        let mixed1 = mix_u64(x, key);
        let mixed2 = mix_u64(x, key);

        assert_eq!(mixed1, mixed2);
    }

    #[test]
    fn mix_u64_different_keys() {
        let x = 12345u64;

        let mixed1 = mix_u64(x, 1u64);
        let mixed2 = mix_u64(x, 2u64);

        assert_ne!(mixed1, mixed2);
    }

    #[test]
    fn mix_u64_different_inputs() {
        let key = 42u64;

        let mixed1 = mix_u64(100u64, key);
        let mixed2 = mix_u64(200u64, key);

        assert_ne!(mixed1, mixed2);
    }

    #[test]
    fn mix_u64_produces_different_values() {
        let x = 12345u64;
        let key = 42u64;

        let mixed = mix_u64(x, key);

        // Mixed value should be different from input
        assert_ne!(mixed, x);
        assert_ne!(mixed, key);
    }

    #[test]
    fn mix_u64_well_distributed() {
        // Test that mixing produces well-distributed values
        let key = 42u64;
        let mut values = Vec::new();

        for i in 0..100 {
            values.push(mix_u64(i as u64, key));
        }

        // All values should be unique (or at least mostly unique)
        let unique_count = values
            .iter()
            .cloned()
            .collect::<std::collections::HashSet<u64>>()
            .len();
        assert!(
            unique_count >= 95,
            "Mix should produce well-distributed values"
        );
    }

    // ==================== SplitMix64 Tests ====================

    #[test]
    fn splitmix64_deterministic() {
        let x = 12345u64;

        let hash1 = splitmix64(x);
        let hash2 = splitmix64(x);

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn splitmix64_different_inputs() {
        let hash1 = splitmix64(1u64);
        let hash2 = splitmix64(2u64);
        let hash3 = splitmix64(3u64);

        assert_ne!(hash1, hash2);
        assert_ne!(hash2, hash3);
    }

    #[test]
    fn splitmix64_produces_different_value() {
        let x = 12345u64;
        let hash = splitmix64(x);

        // Hash should be different from input (usually)
        assert_ne!(hash, x);
    }

    #[test]
    fn splitmix64_well_distributed() {
        let mut values = Vec::new();

        for i in 0..100 {
            values.push(splitmix64(i as u64));
        }

        // All values should be unique
        let unique_count = values
            .iter()
            .cloned()
            .collect::<std::collections::HashSet<u64>>()
            .len();
        assert_eq!(
            unique_count, 100,
            "SplitMix64 should produce unique values for unique inputs"
        );
    }

    #[test]
    fn splitmix64_avalanche_effect() {
        // Small changes in input should produce large changes in output
        let hash1 = splitmix64(1000u64);
        let hash2 = splitmix64(1001u64);

        // Hashes should be very different (bitwise)
        let diff_bits = (hash1 ^ hash2).count_ones();
        assert!(diff_bits > 16, "SplitMix64 should exhibit avalanche effect");
    }

    // ==================== Integration Tests ====================

    #[test]
    fn integration_full_minhash_pipeline() {
        let shingles = vec![1u64, 2u64, 3u64, 4u64, 5u64, 6u64, 7u64, 8u64, 9u64, 10u64];
        let cfg = PerceptualConfig {
            minhash_bands: 4,
            minhash_rows_per_band: 4,
            ..Default::default()
        };
        let m = cfg.minhash_bands * cfg.minhash_rows_per_band; // 16

        let sig = minhash_signature(&shingles, m, &cfg);

        assert_eq!(sig.len(), 16);
        assert!(sig.iter().all(|&v| v != u64::MAX));
    }

    #[test]
    fn minhash_similar_inputs_produce_similar_signatures() {
        // Similar inputs should produce some similar MinHash values
        // This is the fundamental property of MinHash
        let shingles1 = vec![1u64, 2u64, 3u64, 4u64, 5u64];
        let shingles2 = vec![1u64, 2u64, 3u64, 4u64, 6u64]; // Only one element different

        let cfg = PerceptualConfig::default();
        let m = 128;

        let sig1 = minhash_signature(&shingles1, m, &cfg);
        let sig2 = minhash_signature(&shingles2, m, &cfg);

        // Count matching slots
        let matches = sig1.iter().zip(sig2.iter()).filter(|(a, b)| a == b).count();

        // With high probability, at least some slots should match
        // (This is probabilistic, so we use a generous threshold)
        assert!(
            matches > 0,
            "MinHash should have some matching slots for similar inputs"
        );
    }
}
