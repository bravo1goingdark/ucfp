//! ufp_perceptual: Perceptual fingerprinting for canonicalized text.
//!
//! This module implements deterministic shingling, winnowing, and MinHash
//! for textual data. It is configuration-driven (no Cargo features needed)
//! and supports optional parallelism controlled at runtime.

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use thiserror::Error;
use xxhash_rust::xxh3::xxh3_64_with_seed;

/// Configuration for perceptual fingerprinting.
/// Everything is runtime-configurable (no feature flags).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerceptualConfig {
    /// Semantic version for config evolution.
    pub version: u32,
    /// Number of tokens per shingle (default = 9)
    pub k: usize,
    /// Window size for winnowing (default = 4)
    pub w: usize,
    /// Number of bands and rows-per-band for MinHash (default 16×8 = 128)
    pub minhash_bands: usize,
    pub minhash_rows_per_band: usize,
    /// Random seed for deterministic hashing
    pub seed: u64,
    /// Enable parallel MinHash computation (default false)
    pub use_parallel: bool,
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
        }
    }
}

/// Selected winnowed shingle with its originating position.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WinnowedShingle {
    pub hash: u64,
    pub start_idx: usize,
}

/// Final perceptual fingerprint artifact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerceptualFingerprint {
    pub shingles: Vec<u64>,
    pub winnowed: Vec<WinnowedShingle>,
    pub minhash: Vec<u64>,
    pub meta: PerceptualMeta,
}

/// Metadata for traceability and determinism.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct PerceptualMeta {
    pub k: usize,
    pub w: usize,
    pub minhash_len: usize,
    pub minhash_bands: usize,
    pub minhash_rows_per_band: usize,
    pub seed: u64,
    pub use_parallel: bool,
    pub config_version: u32,
}

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

/// Compute a perceptual fingerprint (shingles → winnow → MinHash).
pub fn perceptualize_tokens<S>(
    tokens: &[S],
    cfg: &PerceptualConfig,
) -> Result<PerceptualFingerprint, PerceptualError>
where
    S: AsRef<str>,
{
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

    let minhash_len = cfg
        .minhash_bands
        .checked_mul(cfg.minhash_rows_per_band)
        .ok_or(PerceptualError::InvalidConfigMinhashLength {
            bands: cfg.minhash_bands,
            rows: cfg.minhash_rows_per_band,
        })?;

    // Step 1: Rolling-hash shingles (O(n))
    let shingles = make_shingles_rolling(tokens, cfg.k, cfg.seed);

    // Step 2: Winnowing (O(n))
    let winnowed = winnow_minq(&shingles, cfg.w);

    // Step 3: Deduplicate and compute MinHash
    let uniq: Vec<u64> = if winnowed.is_empty() {
        shingles.clone()
    } else {
        let mut hashes: Vec<u64> = winnowed.iter().map(|w| w.hash).collect();
        hashes.sort_unstable();
        hashes.dedup();
        hashes
    };

    let minhash = minhash_signature(&uniq, minhash_len, cfg);

    Ok(PerceptualFingerprint {
        shingles,
        winnowed,
        minhash,
        meta: PerceptualMeta {
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

/// Compute rolling-hash shingles deterministically in O(n).
pub fn make_shingles_rolling<S: AsRef<str>>(tokens: &[S], k: usize, seed: u64) -> Vec<u64> {
    let n = tokens.len();
    let th: Vec<u64> = tokens
        .iter()
        .map(|t| xxh3_64_with_seed(t.as_ref().as_bytes(), seed))
        .collect();

    const BASE: u64 = 1_000_003;
    let base = BASE ^ splitmix64(seed);

    let mut base_km1 = 1u64;
    for _ in 1..k {
        base_km1 = base_km1.wrapping_mul(base);
    }

    let mut out = Vec::with_capacity(n - k + 1);
    let mut h = 0u64;
    for &val in th.iter().take(k) {
        h = h.wrapping_mul(base).wrapping_add(val);
    }
    out.push(h);

    for (&old, &new) in th.iter().zip(th.iter().skip(k)) {
        h = h.wrapping_sub(old.wrapping_mul(base_km1));
        h = h.wrapping_mul(base).wrapping_add(new);
        out.push(h);
    }
    out
}

/// Winnowing via monotonic deque, O(n).
pub fn winnow_minq(shingles: &[u64], w: usize) -> Vec<WinnowedShingle> {
    let n = shingles.len();
    if n == 0 {
        return Vec::new();
    }

    let w = w.max(1);
    let mut out = Vec::new();
    let mut dq: VecDeque<usize> = VecDeque::new();
    let mut last_picked: Option<usize> = None;

    let push = |dq: &mut VecDeque<usize>, i: usize, vals: &[u64]| {
        while let Some(&j) = dq.back() {
            if vals[i] <= vals[j] {
                dq.pop_back();
            } else {
                break;
            }
        }
        dq.push_back(i);
    };

    for i in 0..w.min(n) {
        push(&mut dq, i, shingles);
    }

    let emit = |dq: &VecDeque<usize>,
                out: &mut Vec<WinnowedShingle>,
                last: &mut Option<usize>,
                vals: &[u64]| {
        // Rightmost tie-breaking keeps winnowing deterministic when minima repeat.
        if let Some(&idx) = dq.back() {
            if *last != Some(idx) {
                out.push(WinnowedShingle {
                    hash: vals[idx],
                    start_idx: idx,
                });
                *last = Some(idx);
            }
        }
    };

    emit(&dq, &mut out, &mut last_picked, shingles);

    for i in w..n {
        let left = i - w + 1;
        while let Some(&j) = dq.front() {
            if j < left {
                dq.pop_front();
            } else {
                break;
            }
        }
        push(&mut dq, i, shingles);
        emit(&dq, &mut out, &mut last_picked, shingles);
    }

    out
}

/// Compute a MinHash signature (parallel if cfg.use_parallel = true).
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

#[inline]
fn compute_slot(unique_shingles: &[u64], j: usize, seed: u64) -> u64 {
    let step = (j as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let key = splitmix64(seed.wrapping_add(step));
    let mut minv = u64::MAX;
    for &val in unique_shingles {
        let h = mix_u64(val, key);
        if h < minv {
            minv = h;
        }
    }
    minv
}

#[inline]
fn mix_u64(x: u64, key: u64) -> u64 {
    let mut h = xxh3_64_with_seed(&x.to_le_bytes(), key);
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51afd7ed558ccd);
    h ^= h >> 33;
    h = h.wrapping_mul(0xc4ceb9fe1a85ec53);
    h ^ (h >> 33)
}

#[inline]
fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

// --------------------------- Tests ---------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(s: &str) -> Vec<String> {
        s.split_whitespace().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_determinism() {
        let cfg = PerceptualConfig {
            k: 2,
            ..Default::default()
        };
        let t1 = toks("hello world this is test text");
        let t2 = toks("hello   world this   is test text");
        let fp1 = perceptualize_tokens(&t1, &cfg).unwrap();
        let fp2 = perceptualize_tokens(&t2, &cfg).unwrap();
        assert_eq!(fp1.minhash, fp2.minhash);
        assert_eq!(fp1.meta.config_version, cfg.version);
        assert_eq!(
            fp1.meta.minhash_len,
            cfg.minhash_bands * cfg.minhash_rows_per_band
        );
    }

    #[test]
    fn test_parallel_equivalence() {
        let cfg_parallel = PerceptualConfig {
            use_parallel: true,
            ..Default::default()
        };
        let tokens = toks("the quick brown fox jumps over the lazy dog");
        let fp1 = perceptualize_tokens(&tokens, &cfg_parallel).unwrap();
        let cfg_serial = PerceptualConfig::default();
        let fp2 = perceptualize_tokens(&tokens, &cfg_serial).unwrap();
        assert_eq!(fp1.minhash, fp2.minhash);
    }

    #[test]
    fn test_invalid_k_rejected() {
        let cfg = PerceptualConfig {
            k: 0,
            ..Default::default()
        };
        let tokens = toks("a b c");
        let res = perceptualize_tokens(&tokens, &cfg);
        assert!(matches!(res, Err(PerceptualError::InvalidConfigK { k: 0 })));
    }

    #[test]
    fn test_invalid_w_rejected() {
        let cfg = PerceptualConfig {
            k: 2,
            w: 0,
            ..Default::default()
        };
        let tokens = toks("a b c d");
        let res = perceptualize_tokens(&tokens, &cfg);
        assert!(matches!(res, Err(PerceptualError::InvalidConfigW { w: 0 })));
    }

    #[test]
    fn test_invalid_bands_rejected() {
        let cfg = PerceptualConfig {
            k: 2,
            minhash_bands: 0,
            ..Default::default()
        };
        let tokens = toks("a b c d e");
        let res = perceptualize_tokens(&tokens, &cfg);
        assert!(matches!(
            res,
            Err(PerceptualError::InvalidConfigBands { bands: 0 })
        ));
    }

    #[test]
    fn test_invalid_rows_rejected() {
        let cfg = PerceptualConfig {
            k: 2,
            minhash_rows_per_band: 0,
            ..Default::default()
        };
        let tokens = toks("a b c d e f");
        let res = perceptualize_tokens(&tokens, &cfg);
        assert!(matches!(
            res,
            Err(PerceptualError::InvalidConfigRows { rows: 0 })
        ));
    }

    #[test]
    fn test_invalid_version_rejected() {
        let cfg = PerceptualConfig {
            version: 0,
            k: 2,
            ..Default::default()
        };
        let tokens = toks("a b c d e f g");
        let res = perceptualize_tokens(&tokens, &cfg);
        assert!(matches!(
            res,
            Err(PerceptualError::InvalidConfigVersion { version: 0 })
        ));
    }

    #[test]
    fn test_minhash_length_overflow_rejected() {
        let cfg = PerceptualConfig {
            k: 2,
            minhash_bands: usize::MAX,
            minhash_rows_per_band: 2,
            ..Default::default()
        };
        let tokens = toks(
            "overflow check requires enough tokens to skip early not-enough check and trigger overflow",
        );
        let res = perceptualize_tokens(&tokens, &cfg);
        assert!(matches!(
            res,
            Err(PerceptualError::InvalidConfigMinhashLength { .. })
        ));
    }

    #[test]
    fn test_not_enough_tokens_error() {
        let cfg = PerceptualConfig::default();
        let tokens = toks("short");
        let res = perceptualize_tokens(&tokens, &cfg);
        assert!(matches!(res, Err(PerceptualError::NotEnoughTokens { .. })));
    }
}
