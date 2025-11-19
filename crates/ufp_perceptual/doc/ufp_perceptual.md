# UCFP Perceptual Fingerprinting

## Purpose

`ufp_perceptual` transforms canonicalized token streams into perceptual fingerprints that remain stable across small textual edits. The pipeline is deterministic and consists of three stages:

1. rolling-hash shingles built from contiguous token windows
2. winnowing to retain rightmost minimum hashes per window
3. MinHash signatures that enable locality-sensitive similarity search

All behavior is configured at runtime through `PerceptualConfig` (no Cargo features required).

## Key Types

```rust
pub struct PerceptualConfig {
    pub version: u32,
    pub k: usize,
    pub w: usize,
    pub minhash_bands: usize,
    pub minhash_rows_per_band: usize,
    pub seed: u64,
    pub use_parallel: bool,
}

pub struct PerceptualFingerprint {
    pub shingles: Vec<u64>,
    pub winnowed: Vec<WinnowedShingle>,
    pub minhash: Vec<u64>,
    pub meta: PerceptualMeta,
}

pub struct WinnowedShingle {
    pub hash: u64,
    pub start_idx: usize,
}

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
```

`PerceptualError` captures invalid configuration (zero/overflowing parameters, unsupported version) and situations where the token stream is too short (`NotEnoughTokens`).

## Public API

```rust
pub fn perceptualize_tokens<T: AsRef<str>>(
    tokens: &[T],
    cfg: &PerceptualConfig,
) -> Result<PerceptualFingerprint, PerceptualError>;

pub fn make_shingles_rolling<T: AsRef<str>>(tokens: &[T], k: usize, seed: u64) -> Vec<u64>;
pub fn winnow_minq(shingles: &[u64], w: usize) -> Vec<WinnowedShingle>;
pub fn minhash_signature(unique_shingles: &[u64], m: usize, cfg: &PerceptualConfig) -> Vec<u64>;
```

- `perceptualize_tokens` drives the full shingle -> winnow -> MinHash pipeline.
- `make_shingles_rolling` exposes the deterministic rolling hash, allowing custom winnowing strategies.
- `winnow_minq` implements a monotonic deque with rightmost tie-breaking for consistent minima. The deque stores candidate indices in non-decreasing hash order, evicts stale entries as the window slides, and inspects the **front** element so we truly pick the minimum hash (older revisions accidentally peeked at the back, selecting maxima). When hashes tie, newer candidates replace older ones to enforce deterministic rightmost minima.
- `minhash_signature` supports optional Rayon-backed parallelism when `cfg.use_parallel` is `true`.

### Winnowing behavior

`winnow_minq` guarantees coverage even when `w` is larger than the shingle count by clamping to at least one window. Each step:

1. Drops indices that fell left of the current window.
2. Pops trailing candidates whose hash is greater than or equal to the new entrant.
3. Emits the front index if it differs from the previously published shingle.

This mirrors the classic winnowing algorithm and produces deterministic fingerprints for deduplication and similarity scoring.

### Configuration Fields

- `version` - Semantic version of the configuration; must be >= 1.
- `k` - Tokens per shingle window. Larger values capture longer phrases while reducing the number of shingles; defaults to 9.
- `w` - Winnowing window size (in shingles). Smaller windows retain more fingerprints; defaults to 4. When `w` exceeds the number of shingles we treat the entire document as a single window so at least one fingerprint is emitted.
- `minhash_bands` - Number of MinHash bands. Together with `minhash_rows_per_band` it defines signature length; defaults to 16.
- `minhash_rows_per_band` - Rows per band. Defaults to 8, producing 128 MinHash values with the default band count.
- `seed` - Master seed feeding both rolling hash and MinHash permutations for deterministic output.
- `use_parallel` - Enables Rayon-backed parallel MinHash computation when `true`.

## Example

```rust
use ufp_canonical::{canonicalize, CanonicalizeConfig};
use ufp_perceptual::{perceptualize_tokens, PerceptualConfig};

let canonical = canonicalize(
    "demo-doc",
    "Hello   perceptual world",
    &CanonicalizeConfig::default(),
).expect("canonicalization succeeds");
let tokens: Vec<String> = canonical.tokens.iter().map(|t| t.text.clone()).collect();

let mut cfg = PerceptualConfig::default();
cfg.k = 2; // ensure enough tokens for the example
cfg.use_parallel = false;

let fingerprint = perceptualize_tokens(&tokens, &cfg)?;
assert_eq!(fingerprint.meta.k, 2);
assert_eq!(fingerprint.meta.use_parallel, false);
```

### Examples

- `cargo run --package ufp_perceptual --example fingerprint_demo` - prints shingles, winnowed selections, and MinHash output for a sample sentence.

## Testing

```bash
cargo test -p ufp_perceptual
```

Unit tests cover determinism, parallel vs sequential parity, invalid configuration guards, and rolling-hash arithmetic.

## Integration

`PerceptualFingerprint` is the third step in the UCFP pipeline. After ingest normalization (`ufp_ingest`) and canonicalization (`ufp_canonical`), pass canonical tokens into `perceptualize_tokens` to obtain similarity-aware fingerprints for clustering, deduplication, or indexing.




