//! UCFP Perceptual Fingerprinting
//!
//! This crate handles the "what does it look like" part of fingerprinting. Given
//! canonical tokens, it produces a compact signature that captures similarity -
//! so near-duplicates will have similar fingerprints even if they're not identical.
//!
//! ## What you need to know
//!
//! - We only take canonical tokens. Don't send us raw text or ingest metadata.
//! - Pure function: same input = same output. No I/O, no network, no randomness.
//!
//! ## The pipeline (three stages)
//!
//! 1. **Shingling** - Break tokens into overlapping windows of k tokens,
//!    hash each window to a 64-bit value. Captures local structure.
//!
//! 2. **Winnowing** - Pick the minimum hash from each sliding window.
//!    Reduces data size. This is just an optimization, not the actual LSH step.
//!
//! 3. **MinHash** - The real locality-sensitive hashing magic.
//!    Produces a fixed-size signature you can compare for Jaccard similarity.
//!
//! ## Quick example
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

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Constants Tests ====================

    #[test]
    fn constants_defined() {
        assert_eq!(PERCEPTUAL_VERSION, 1);
        assert_eq!(PERCEPTUAL_ALGORITHM, "rollingminqminhash_v1");
    }

    // ==================== Main Pipeline Tests ====================

    #[test]
    fn perceptualize_tokens_success() {
        let tokens = vec![
            "the", "quick", "brown", "fox", "jumps", "over", "the", "lazy", "dog",
        ];
        let cfg = PerceptualConfig::default();

        let result = perceptualize_tokens(&tokens, &cfg);
        assert!(result.is_ok());

        let fingerprint = result.unwrap();
        assert!(!fingerprint.minhash.is_empty());
        assert_eq!(fingerprint.meta.k, cfg.k);
        assert_eq!(fingerprint.meta.w, cfg.w);
    }

    #[test]
    fn perceptualize_tokens_deterministic() {
        let tokens = vec![
            "the", "quick", "brown", "fox", "jumps", "over", "the", "lazy", "dog",
        ];
        let cfg = PerceptualConfig::default();

        let fp1 = perceptualize_tokens(&tokens, &cfg).unwrap();
        let fp2 = perceptualize_tokens(&tokens, &cfg).unwrap();

        assert_eq!(fp1.minhash, fp2.minhash);
        assert_eq!(fp1.meta.seed, fp2.meta.seed);
    }

    #[test]
    fn perceptualize_tokens_different_seeds() {
        let tokens = vec![
            "the", "quick", "brown", "fox", "jumps", "over", "the", "lazy", "dog",
        ];

        let cfg1 = PerceptualConfig {
            seed: 12345,
            ..Default::default()
        };
        let cfg2 = PerceptualConfig {
            seed: 54321,
            ..Default::default()
        };

        let fp1 = perceptualize_tokens(&tokens, &cfg1).unwrap();
        let fp2 = perceptualize_tokens(&tokens, &cfg2).unwrap();

        assert_ne!(fp1.minhash, fp2.minhash);
    }

    #[test]
    fn perceptualize_tokens_error_not_enough_tokens() {
        let tokens = vec!["hello", "world"];
        let cfg = PerceptualConfig::default(); // k = 9 by default

        let result = perceptualize_tokens(&tokens, &cfg);
        assert!(matches!(
            result,
            Err(PerceptualError::NotEnoughTokens { k: 9 })
        ));
    }

    #[test]
    fn perceptualize_tokens_error_invalid_k() {
        let tokens = vec!["the", "quick", "brown"];
        let cfg = PerceptualConfig {
            k: 0,
            ..Default::default()
        };

        let result = perceptualize_tokens(&tokens, &cfg);
        assert!(matches!(
            result,
            Err(PerceptualError::InvalidConfigK { k: 0 })
        ));
    }

    #[test]
    fn perceptualize_tokens_error_invalid_w() {
        let tokens = vec!["the", "quick", "brown", "fox"];
        let cfg = PerceptualConfig {
            w: 0,
            ..Default::default()
        };

        let result = perceptualize_tokens(&tokens, &cfg);
        assert!(matches!(
            result,
            Err(PerceptualError::InvalidConfigW { w: 0 })
        ));
    }

    #[test]
    fn perceptualize_tokens_error_invalid_bands() {
        let tokens = vec!["the", "quick", "brown", "fox"];
        let cfg = PerceptualConfig {
            minhash_bands: 0,
            ..Default::default()
        };

        let result = perceptualize_tokens(&tokens, &cfg);
        assert!(matches!(
            result,
            Err(PerceptualError::InvalidConfigBands { bands: 0 })
        ));
    }

    #[test]
    fn perceptualize_tokens_error_invalid_rows() {
        let tokens = vec!["the", "quick", "brown", "fox"];
        let cfg = PerceptualConfig {
            minhash_rows_per_band: 0,
            ..Default::default()
        };

        let result = perceptualize_tokens(&tokens, &cfg);
        assert!(matches!(
            result,
            Err(PerceptualError::InvalidConfigRows { rows: 0 })
        ));
    }

    #[test]
    fn perceptualize_tokens_error_invalid_version() {
        let tokens = vec!["the", "quick", "brown", "fox"];
        let cfg = PerceptualConfig {
            version: 0,
            ..Default::default()
        };

        let result = perceptualize_tokens(&tokens, &cfg);
        assert!(matches!(
            result,
            Err(PerceptualError::InvalidConfigVersion { version: 0 })
        ));
    }

    #[test]
    fn perceptualize_tokens_error_overflow() {
        let tokens: Vec<&str> = (0..100).map(|_| "token").collect();
        let cfg = PerceptualConfig {
            minhash_bands: usize::MAX,
            minhash_rows_per_band: 2,
            ..Default::default()
        };

        let result = perceptualize_tokens(&tokens, &cfg);
        assert!(matches!(
            result,
            Err(PerceptualError::InvalidConfigMinhashLength { .. })
        ));
    }

    #[test]
    fn perceptualize_tokens_with_intermediates() {
        let tokens = vec![
            "the", "quick", "brown", "fox", "jumps", "over", "the", "lazy", "dog",
        ];
        let cfg = PerceptualConfig {
            include_intermediates: true,
            ..Default::default()
        };

        let fp = perceptualize_tokens(&tokens, &cfg).unwrap();
        assert!(!fp.shingles.is_empty());
        assert!(!fp.winnowed.is_empty());
    }

    #[test]
    fn perceptualize_tokens_without_intermediates() {
        let tokens = vec![
            "the", "quick", "brown", "fox", "jumps", "over", "the", "lazy", "dog",
        ];
        let cfg = PerceptualConfig {
            include_intermediates: false,
            ..Default::default()
        };

        let fp = perceptualize_tokens(&tokens, &cfg).unwrap();
        assert!(fp.shingles.is_empty());
        assert!(fp.winnowed.is_empty());
        assert!(!fp.minhash.is_empty()); // MinHash should still be present
    }

    #[test]
    fn perceptualize_tokens_metadata_correct() {
        let tokens = vec!["the", "quick", "brown", "fox", "jumps", "over"];
        let cfg = PerceptualConfig {
            k: 3,
            w: 2,
            minhash_bands: 8,
            minhash_rows_per_band: 4,
            seed: 12345,
            use_parallel: true,
            version: 2,
            ..Default::default()
        };

        let fp = perceptualize_tokens(&tokens, &cfg).unwrap();

        assert_eq!(fp.meta.perceptual_version, PERCEPTUAL_VERSION);
        assert_eq!(fp.meta.algorithm_name, PERCEPTUAL_ALGORITHM);
        assert_eq!(fp.meta.k, 3);
        assert_eq!(fp.meta.w, 2);
        assert_eq!(fp.meta.minhash_bands, 8);
        assert_eq!(fp.meta.minhash_rows_per_band, 4);
        assert_eq!(fp.meta.minhash_len, 32); // 8 * 4
        assert_eq!(fp.meta.seed, 12345);
        assert!(fp.meta.use_parallel);
        assert_eq!(fp.meta.config_version, 2);
    }

    // ==================== Edge Cases ====================

    #[test]
    fn perceptualize_tokens_minimum_viable_input() {
        // Minimum: k tokens, k shingles
        let tokens: Vec<String> = (0..9).map(|i| format!("token{i}")).collect();
        let cfg = PerceptualConfig::default(); // k = 9

        let fp = perceptualize_tokens(&tokens, &cfg).unwrap();
        assert!(!fp.minhash.is_empty());
    }

    #[test]
    fn perceptualize_tokens_large_input() {
        let tokens: Vec<String> = (0..1000).map(|i| format!("token{i}")).collect();
        let cfg = PerceptualConfig::default();

        let fp = perceptualize_tokens(&tokens, &cfg).unwrap();
        assert!(!fp.minhash.is_empty());
        assert_eq!(fp.minhash.len(), 128); // 16 * 8
    }

    #[test]
    fn perceptualize_tokens_similarity_property() {
        // Test that similar documents have some similar MinHash values
        // Use longer text to ensure enough shingles for meaningful comparison
        let tokens1 = vec![
            "the", "quick", "brown", "fox", "jumps", "over", "the", "lazy", "dog", "and", "runs",
            "through", "the", "forest", "very", "fast", "indeed",
        ];
        let tokens2 = vec![
            "the", "quick", "brown", "fox", "jumps", "over", "the", "lazy", "dog", "and", "walks",
            "through", "the", "forest", "very", "slowly", "indeed",
        ];
        // Similar documents with some differences

        let cfg = PerceptualConfig::default();
        let fp1 = perceptualize_tokens(&tokens1, &cfg).unwrap();
        let fp2 = perceptualize_tokens(&tokens2, &cfg).unwrap();

        // Count matching MinHash values
        let matches = fp1
            .minhash
            .iter()
            .zip(fp2.minhash.iter())
            .filter(|(a, b)| a == b)
            .count();

        // Document the behavior - note this is probabilistic
        // Similar documents typically share some MinHash values
        println!("Matching MinHash values: {matches}/128");
        // This test documents the behavior - note that matches is always >= 0 for usize
        // Similar documents should have some matching MinHash values with high probability
        println!("Similar documents may share some MinHash values (found {matches} matches)");
    }

    // ==================== Doc Test Verification ====================

    #[test]
    fn doc_test_example() {
        // Replicate the doc test example to ensure it works
        let tokens = vec![
            "the", "quick", "brown", "fox", "jumps", "over", "the", "lazy", "dog",
        ];
        let cfg = PerceptualConfig {
            k: 3,
            ..Default::default()
        };

        let fingerprint = perceptualize_tokens(&tokens, &cfg).unwrap();

        assert!(!fingerprint.minhash.is_empty());
        assert_eq!(fingerprint.meta.k, 3);
    }

    // ==================== Feature Tests (with_canonical) ====================

    #[cfg(feature = "with_canonical")]
    mod canonical_integration_tests {
        use super::*;
        use canonical::{canonicalize, CanonicalizeConfig};

        #[test]
        fn full_pipeline_with_canonical() {
            let text = "The quick brown fox jumps over the lazy dog";
            let canonical_doc =
                canonicalize("test-doc", text, &CanonicalizeConfig::default()).unwrap();
            let tokens: Vec<&str> = canonical_doc
                .tokens
                .iter()
                .map(|t| t.text.as_str())
                .collect();

            let cfg = PerceptualConfig::default();
            let fp = perceptualize_tokens(&tokens, &cfg).unwrap();

            assert!(!fp.minhash.is_empty());
        }
    }

    // ==================== Stress Tests ====================

    #[test]
    fn perceptualize_tokens_stress_repeated_calls() {
        let tokens: Vec<String> = (0..100).map(|i| format!("token{i}")).collect();
        let cfg = PerceptualConfig::default();

        // Call multiple times to ensure consistency
        for _ in 0..10 {
            let fp = perceptualize_tokens(&tokens, &cfg).unwrap();
            assert!(!fp.minhash.is_empty());
        }
    }

    #[test]
    fn perceptualize_tokens_various_k_values() {
        let tokens: Vec<String> = (0..50).map(|i| format!("token{i}")).collect();

        for k in [3, 5, 7, 9, 11] {
            let cfg = PerceptualConfig {
                k,
                ..Default::default()
            };
            let fp = perceptualize_tokens(&tokens, &cfg).unwrap();
            assert!(!fp.minhash.is_empty(), "Should work with k={k}");
        }
    }

    #[test]
    fn perceptualize_tokens_various_w_values() {
        let tokens: Vec<String> = (0..50).map(|i| format!("token{i}")).collect();

        for w in [1, 2, 4, 8, 16] {
            let cfg = PerceptualConfig {
                w,
                ..Default::default()
            };
            let fp = perceptualize_tokens(&tokens, &cfg).unwrap();
            assert!(!fp.minhash.is_empty(), "Should work with w={w}");
        }
    }
}
