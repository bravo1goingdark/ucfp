# UCFP Perceptual Fingerprinting

## Purpose

`ufp_perceptual` converts canonicalized token streams into perceptual fingerprints that remain
stable across minor text edits. The library provides three deterministic stages:

1. rolling-hash shingles from contiguous token windows
2. winnowing to keep the rightmost minimum hash per window
3. MinHash signatures for locality-sensitive similarity

All behavior is driven by `PerceptualConfig`, including shingle length, winnow window size, seed,
and whether to use parallel MinHash computation.

## Key Types

```rust
pub struct PerceptualConfig {
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
    pub seed: u64,
    pub use_parallel: bool,
}
```

Errors surface through `PerceptualError`, which guards invalid configuration (`k == 0`) and ensures
an adequate number of tokens.

## Public API

```rust
pub fn perceptualize_tokens(
    tokens: &[String],
    cfg: &PerceptualConfig,
) -> Result<PerceptualFingerprint, PerceptualError>;

pub fn make_shingles_rolling(tokens: &[String], k: usize, seed: u64) -> Vec<u64>;
pub fn winnow_minq(shingles: &[u64], w: usize) -> Vec<WinnowedShingle>;
pub fn minhash_signature(unique_shingles: &[u64], m: usize, cfg: &PerceptualConfig) -> Vec<u64>;
```

- `perceptualize_tokens` drives the full shingle → winnow → MinHash pipeline.
- `make_shingles_rolling` exposes the deterministic rolling hash builder, enabling callers to plug
  in alternate winnowing strategies.
- `winnow_minq` implements a monotonic deque with rightmost tie-breaking, ensuring consistent minima.
- `minhash_signature` supports runtime parallelism via Rayon when `cfg.use_parallel` is true.

### Configuration Fields

- `k` — Number of tokens per shingle window. Larger values capture longer phrases while reducing the
  number of shingles; defaults to 9.
- `w` — Winnowing window size used to select representative shingles. Smaller windows retain more
  fingerprints; defaults to 4.
- `minhash_bands` — Number of MinHash bands (groups) in the signature. Together with
  `minhash_rows_per_band` this defines signature length and collision behavior; defaults to 16.
- `minhash_rows_per_band` — Rows per band. The final MinHash length is `minhash_bands *
  minhash_rows_per_band`; defaults to 8 (128 total values).
- `seed` — Master seed feeding the rolling hash and MinHash permutations for determinism. Changing
  the seed yields different but stable fingerprints.
- `use_parallel` — Enables Rayon-backed parallel MinHash computation when `true`. Keep it `false`
  for single-threaded environments or deterministic ordering preferences.

## Example

```rust
use ufp_canonical::{canonicalize, CanonicalizeConfig};
use ufp_perceptual::{perceptualize_tokens, PerceptualConfig};

let canonical = canonicalize("Hello   perceptual world", &CanonicalizeConfig::default());
let tokens: Vec<String> = canonical.tokens.iter().map(|t| t.text.clone()).collect();

let cfg = PerceptualConfig {
    k: 5,
    w: 4,
    minhash_bands: 16,
    minhash_rows_per_band: 8,
    seed: 0x5EED,
    use_parallel: false,
};

let fp = perceptualize_tokens(&tokens, &cfg)?;
assert_eq!(fp.meta.k, 5);
assert_eq!(fp.meta.use_parallel, false);
```

### Examples

- `cargo run --package ufp_perceptual --example fingerprint_demo` - prints shingles, winnowed
  selections, and MinHash for a sample sentence.

## Testing

Run unit tests with:

```bash
cargo test -p ufp_perceptual
```

Tests assert deterministic signatures, parity between parallel and sequential MinHash execution, and
the rolling hash arithmetic.

## Integration

`PerceptualFingerprint` is the third step in the UCFP pipeline. After ingest normalization
(`ufp_ingest`) and canonicalization (`ufp_canonical`), pass the canonical tokens into
`perceptualize_tokens` to generate similarity-aware fingerprints suitable for clustering,
deduplication, or indexing.
