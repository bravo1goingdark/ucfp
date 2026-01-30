# UCFP Perceptual Fingerprinting

## Purpose

`perceptual` transforms canonicalized token streams into perceptual fingerprints that remain stable across small textual edits. The pipeline is **perceptual-only**:

- it *only* consumes canonical tokens and never sees raw payloads or ingest metadata;
- it performs no normalization or tokenization; and
- for a given canonical token stream and `PerceptualConfig`, it is a pure, deterministic function with no I/O.

The pipeline consists of three stages:

1. rolling-hash shingles built from contiguous token windows
2. winnowing to retain rightmost minimum hashes per window
3. MinHash signatures that enable locality-sensitive similarity search

All behavior is configured at runtime through `PerceptualConfig` (no Cargo features required), and the resulting `PerceptualFingerprint` records its `perceptual_version` and algorithm identifier so that changes remain auditable.

## Key Types

```rust
pub struct PerceptualConfig {
    /// Configuration schema version (>= 1). Any behavior change that can alter
    /// fingerprints must bump this value.
    pub version: u32,
    /// Tokens per shingle window.
    pub k: usize,
    /// Winnowing window size in shingles.
    pub w: usize,
    /// Number of MinHash bands.
    pub minhash_bands: usize,
    /// Number of rows per band.
    pub minhash_rows_per_band: usize,
    /// Master seed for rolling hash and MinHash permutations.
    pub seed: u64,
    /// Enable Rayon-backed parallel MinHash when true.
    pub use_parallel: bool,
    /// When false, shingles and winnowed shingles are computed internally but
    /// cleared from the returned fingerprint to reduce storage and bandwidth.
    pub include_intermediates: bool,
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
    /// Perceptual algorithm version managed by this crate.
    pub perceptual_version: u16,
    /// Human-readable algorithm identifier (e.g. "rolling+minq+minhash_v1").
    /// Stored as an owned `String` for straightforward serialization.
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
    /// Hash seed used for both shingling and MinHash.
    pub seed: u64,
    /// Whether MinHash was computed using the parallel implementation.
    pub use_parallel: bool,
    /// Schema/configuration version supplied at computation time.
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

// From the `shingles` module:
pub fn make_shingles_rolling<T: AsRef<str>>(tokens: &[T], k: usize, seed: u64) -> Vec<u64>;
pub fn winnow_minq(shingles: &[u64], w: usize) -> Vec<WinnowedShingle>;

// From the `minhash` module:
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

- `version` - Semantic version of the configuration; must be >= 1. Any change that affects fingerprints requires a bump.
- `k` - Tokens per shingle window. Larger values capture longer phrases while reducing the number of shingles; defaults to 9.
- `w` - Winnowing window size (in shingles). Smaller windows retain more fingerprints; defaults to 4. When `w` exceeds the number of shingles we treat the entire document as a single window so at least one fingerprint is emitted.
- `minhash_bands` - Number of MinHash bands. Together with `minhash_rows_per_band` it defines signature length; defaults to 16.
- `minhash_rows_per_band` - Rows per band. Defaults to 8, producing 128 MinHash values with the default band count.
- `seed` - Master seed feeding both rolling hash and MinHash permutations for deterministic output.
- `use_parallel` - Enables Rayon-backed parallel MinHash computation when `true`.
- `include_intermediates` - When `false`, the returned `PerceptualFingerprint` omits `shingles` and `winnowed` content (they are computed internally but cleared before return) to reduce memory and storage. This does not change the MinHash result.

## Example

```rust
use canonical::{canonicalize, CanonicalizeConfig};
use perceptual::{perceptualize_tokens, PerceptualConfig};

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

- `cargo run --package perceptual --example fingerprint_demo` - prints shingles, winnowed selections, and MinHash output for a sample sentence.

## Testing

```bash
cargo test -p perceptual
```

Unit tests cover determinism, parallel vs sequential parity, invalid configuration guards, and rolling-hash arithmetic.

## Integration

`PerceptualFingerprint` is the third step in the UCFP pipeline. After ingest normalization (`ingest`) and canonicalization (`canonical`), pass canonical tokens into `perceptualize_tokens` to obtain similarity-aware fingerprints for clustering, deduplication, or indexing.




