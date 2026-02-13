# UCFP Perceptual Crate

> **Perceptual fingerprinting for the Universal Content Fingerprinting pipeline**

[![API Docs](https://img.shields.io/badge/docs-api-blue)](https://docs.rs/perceptual)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)

## Table of Contents

- [Overview](#overview)
- [Quick Start](#quick-start)
- [Architecture](#architecture)
- [Core Concepts](#core-concepts)
- [Configuration](#configuration)
- [API Reference](#api-reference)
- [Error Handling](#error-handling)
- [Examples](#examples)
- [Best Practices](#best-practices)
- [Performance](#performance)
- [Troubleshooting](#troubleshooting)
- [Integration Guide](#integration-guide)
- [Testing](#testing)

---

## Overview

The `perceptual` crate is the **third stage** in the Universal Content Fingerprinting (UCFP) pipeline. It transforms canonicalized token streams into perceptual fingerprints that remain stable across small textual edits.

### What This Crate Does

| Function | Description |
|----------|-------------|
| **Shingling** | Build contiguous token windows using polynomial rolling hash |
| **Winnowing** | Data reduction to select minimum hashes per window |
| **MinHash LSH** | Locality-Sensitive Hashing for approximate Jaccard similarity |
| **Determinism** | Same input + config → identical output |

### Key Properties

- **Pure**: No I/O, no network, deterministic output
- **Perceptual-Only**: Only consumes canonical tokens, never raw payloads
- **Versioned**: Config version tracks algorithm changes
- **Configurable**: Runtime configuration via `PerceptualConfig`

### Pipeline Position

```
┌─────────┐     ┌──────────┐     ┌──────────────────┐     ┌───────┐     ┌───────┐
│  Ingest │────▶│Canonical │────▶│Perceptual/Semantic│────▶│ Index │────▶│ Match │
│         │     │          │     │     (this)       │     │       │     │       │
└─────────┘     └──────────┘     └──────────────────┘     └───────┘     └───────┘
```

---

## Quick Start

### Basic Fingerprinting

```rust
use canonical::{canonicalize, CanonicalizeConfig};
use perceptual::{perceptualize_tokens, PerceptualConfig};

let canonical = canonicalize(
    "demo-doc",
    "Hello perceptual world",
    &CanonicalizeConfig::default(),
).expect("canonicalization succeeds");

let tokens: Vec<String> = canonical.tokens.iter().map(|t| t.text.clone()).collect();
let cfg = PerceptualConfig::default();

let fingerprint = perceptualize_tokens(&tokens, &cfg)?;
assert!(!fingerprint.minhash.is_empty());
```

### Production Configuration

```rust
use perceptual::PerceptualConfig;

let cfg = PerceptualConfig {
    version: 1,
    k: 9,                    // Tokens per shingle
    w: 4,                    // Winnowing window size
    minhash_bands: 16,       // LSH bands (recall vs precision)
    minhash_rows_per_band: 8,
    seed: 42,
    use_parallel: true,
    include_intermediates: false,  // Save memory in production
};

cfg.validate()?;
```

---

## Architecture

### Pipeline Overview

```
Canonical Tokens
      │
      ▼
┌─────────────────────────────────────────┐
│         Perceptual Pipeline              │
├─────────────────────────────────────────┤
│  1. Rolling-Hash Shingling             │
│     - k-token contiguous windows        │
│     - Polynomial rolling hash           │
│     - Deterministic output              │
├─────────────────────────────────────────┤
│  2. Winnowing (Data Reduction)         │
│     - MinQ algorithm (deque-based)     │
│     - Selects minimum hashes           │
│     - Reduces shingle count            │
├─────────────────────────────────────────┤
│  3. MinHash LSH                        │
│     - bands × rows signature           │
│     - Approximate Jaccard similarity   │
│     - Fixed-size output                │
└─────────────────────────────────────────┘
      │
      ▼
PerceptualFingerprint
```

### Three Stages

1. **Rolling-hash shingles**: Build shingles from contiguous token windows using a polynomial rolling hash.
2. **Winnowing**: Data reduction step that selects minimum hashes per window to reduce computation time.
3. **MinHash signatures**: The actual LSH implementation producing fixed-size signatures for similarity search.

---

## Core Concepts

### Shingling

Tokens are grouped into overlapping windows of size `k`:

```
Tokens: [A, B, C, D, E]
k = 3

Shingles: [A,B,C], [B,C,D], [C,D,E]
```

### Winnowing

Selects minimum hashes from sliding windows to reduce data:

- Uses a monotonic deque with rightmost tie-breaking
- Guarantees at least one shingle even when window exceeds count
- Trade-off: larger window = fewer shingles = faster, but potentially reduced accuracy

### MinHash LSH

**This is the actual Locality-Sensitive Hashing implementation**, not winnowing.

How it works:
- Takes unique winnowed shingle hashes as input
- Computes `m = bands × rows` independent hash functions
- Each signature slot is the minimum hash value for that permutation
- The bands×rows structure enables efficient similarity search

---

## Configuration

### PerceptualConfig

Central configuration struct controlling perceptual fingerprinting:

```rust
pub struct PerceptualConfig {
    /// Schema version (>= 1). Any behavior change must bump this.
    pub version: u32,
    /// Tokens per shingle window.
    pub k: usize,
    /// Winnowing window size in shingles.
    pub w: usize,
    /// Number of MinHash bands.
    pub minhash_bands: usize,
    /// Number of rows per band.
    pub minhash_rows_per_band: usize,
    /// Master seed for rolling hash and MinHash.
    pub seed: u64,
    /// Enable Rayon-backed parallel MinHash.
    pub use_parallel: bool,
    /// Include intermediate results (shingles, winnowed).
    pub include_intermediates: bool,
}
```

### Configuration Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `version` | u32 | 1 | Schema version, must be >= 1 |
| `k` | usize | 9 | Tokens per shingle window |
| `w` | usize | 4 | Winnowing window size |
| `minhash_bands` | usize | 16 | LSH bands (recall vs precision) |
| `minhash_rows_per_band` | usize | 8 | Rows per band |
| `seed` | u64 | 42 | Master seed for determinism |
| `use_parallel` | bool | false | Enable parallel MinHash |
| `include_intermediates` | bool | true | Include shingles/winnowed in output |

### Default Configuration

```rust
PerceptualConfig::default()
// k=9, w=4, bands=16, rows=8, seed=42
// Produces 128-element MinHash signature
```

### Configuration Validation

```rust
fn main() {
    let cfg = load_perceptual_config();
    if let Err(e) = cfg.validate() {
        eprintln!("Invalid perceptual configuration: {}", e);
        std::process::exit(1);
    }
}
```

---

## API Reference

### Main Function

#### `perceptualize_tokens()`

```rust
pub fn perceptualize_tokens<T: AsRef<str>>(
    tokens: &[T],
    cfg: &PerceptualConfig,
) -> Result<PerceptualFingerprint, PerceptualError>
```

**Primary entry point** for perceptual fingerprinting.

**Parameters:**
- `tokens`: Slice of canonical tokens
- `cfg`: Configuration struct

**Returns:**
- `Ok(PerceptualFingerprint)`: Contains MinHash signature
- `Err(PerceptualError)`: Invalid config or not enough tokens

### Utility Functions

#### `make_shingles_rolling()`

```rust
pub fn make_shingles_rolling<T: AsRef<str>>(
    tokens: &[T], 
    k: usize, 
    seed: u64
) -> Vec<u64>
```

Generate rolling-hash shingles from tokens.

#### `winnow_minq()`

```rust
pub fn winnow_minq(shingles: &[u64], w: usize) -> Vec<WinnowedShingle>
```

Winnowing with MinQ algorithm (monotonic deque).

#### `minhash_signature()`

```rust
pub fn minhash_signature(
    unique_shingles: &[u64], 
    m: usize, 
    cfg: &PerceptualConfig
) -> Vec<u64>
```

Compute MinHash signature with optional parallelism.

### Data Types

See the [types module](src/types.rs) for complete definitions of:
- `PerceptualConfig` - Configuration struct
- `PerceptualFingerprint` - Output structure
- `WinnowedShingle` - Winnowed shingle with position
- `PerceptualMeta` - Metadata about computation
- `PerceptualError` - Error variants

---

## Error Handling

### PerceptualError Variants

| Error | Trigger | Recovery |
|-------|---------|----------|
| `InvalidConfig(msg)` | Zero/overflowing parameters, bad version | Fix configuration |
| `NotEnoughTokens` | Fewer tokens than needed for shingling | Provide more tokens |

### Error Handling Patterns

**Pattern 1: Graceful Handling**

```rust
use perceptual::{perceptualize_tokens, PerceptualError};

match perceptualize_tokens(&tokens, &cfg) {
    Ok(fingerprint) => {
        tracing::info!(
            doc_id = %doc_id,
            minhash_len = fingerprint.minhash.len(),
            "perceptual_success"
        );
    }
    Err(PerceptualError::NotEnoughTokens) => {
        tracing::warn!(doc_id = %doc_id, "not_enough_tokens_for_fingerprint");
    }
    Err(e) => {
        tracing::error!(error = %e, "perceptual_failed");
    }
}
```

**Pattern 2: Validate First**

```rust
let cfg = PerceptualConfig::default();
if let Err(e) = cfg.validate() {
    return Err(format!("Invalid config: {}", e));
}
let fingerprint = perceptualize_tokens(&tokens, &cfg)?;
```

---

## Examples

### Example 1: Basic Fingerprinting

```rust
use canonical::{canonicalize, CanonicalizeConfig};
use perceptual::{perceptualize_tokens, PerceptualConfig};

let canonical = canonicalize(
    "demo-doc",
    "Hello perceptual world",
    &CanonicalizeConfig::default(),
).expect("canonicalization succeeds");

let tokens: Vec<String> = canonical.tokens.iter().map(|t| t.text.clone()).collect();
let cfg = PerceptualConfig::default();

let fingerprint = perceptualize_tokens(&tokens, &cfg)?;
assert!(!fingerprint.minhash.is_empty());
```

### Example 2: Custom Shingle Size

```rust
use perceptual::{perceptualize_tokens, PerceptualConfig};

let mut cfg = PerceptualConfig::default();
cfg.k = 3;  // 3-token shingles
cfg.use_parallel = false;

let tokens = vec!["hello", "world", "test", "foo", "bar"];
let fingerprint = perceptualize_tokens(&tokens, &cfg)?;
assert_eq!(fingerprint.meta.k, 3);
```

### Example 3: Production Configuration

```rust
use perceptual::PerceptualConfig;

let cfg = PerceptualConfig {
    version: 1,
    k: 9,
    w: 4,
    minhash_bands: 16,
    minhash_rows_per_band: 8,
    seed: 12345,
    use_parallel: true,
    include_intermediates: false,  // Don't store intermediates
};

// Validate first
cfg.validate()?;

let fingerprint = perceptualize_tokens(&tokens, &cfg)?;
// Only minhash is populated, not shingles/winnowed
```

### Example 4: Custom Winnowing

```rust
use perceptual::{make_shingles_rolling, winnow_minq};

let tokens = vec!["a", "b", "c", "d", "e", "f", "g"];
let k = 3;
let w = 2;

// Custom shingling
let shingles = make_shingles_rolling(&tokens, k, 42);
assert_eq!(shingles.len(), 5);  // 7 - 3 + 1

// Custom winnowing
let winnowed = winnow_minq(&shingles, w);
```

### Example 5: Debugging Intermediates

```rust
use perceptual::PerceptualConfig;

let mut cfg = PerceptualConfig::default();
cfg.include_intermediates = true;  // Include all intermediate data

let fingerprint = perceptualize_tokens(&tokens, &cfg)?;

// Access all stages
println!("Shingles: {:?}", fingerprint.shingles);
println!("Winnowed: {:?}", fingerprint.winnowed);
println!("MinHash: {:?}", fingerprint.minhash);
```

---

## Best Practices

### 1. Use Production Configuration

```rust
let cfg = PerceptualConfig {
    version: 1,
    k: 9,
    w: 4,
    minhash_bands: 16,
    minhash_rows_per_band: 8,
    seed: 42,
    use_parallel: true,  // Enable parallelism in production
    include_intermediates: false,  // Save memory
};
```

### 2. Store Only MinHash

```rust
// Only store minhash in your index
struct PerceptualIndexEntry {
    doc_id: String,
    minhash: Vec<u64>,  // This is what matters for similarity
    // Don't store shingles or winnowed
}
```

### 3. Validate Configuration at Startup

```rust
fn main() {
    let cfg = load_perceptual_config();
    if let Err(e) = cfg.validate() {
        eprintln!("Invalid perceptual config: {}", e);
        std::process::exit(1);
    }
}
```

### 4. Document Your Version

```rust
info!(
    "Fingerprinted doc_id={} version={} k={} w={}",
    doc_id,
    fingerprint.meta.config_version,
    fingerprint.meta.k,
    fingerprint.meta.w
);
```

### 5. Handle Short Documents

```rust
match perceptualize_tokens(&tokens, &cfg) {
    Ok(fp) => process(fp),
    Err(PerceptualError::NotEnoughTokens) => {
        // Document too short for fingerprinting
        // Fall back to exact matching or skip
    }
}
```

---

## Performance

### Benchmarks

Typical performance on modern hardware:

| Operation | Latency (μs) | Throughput |
|-----------|--------------|------------|
| 10 tokens | ~5 | 200K ops/sec |
| 100 tokens | ~20 | 50K ops/sec |
| 1000 tokens | ~150 | 6K ops/sec |
| 10000 tokens | ~1.5ms | 660 ops/sec |

### Parallel Performance

With `use_parallel: true`:
- 1000 tokens: ~80μs (vs ~150μs sequential)
- 10000 tokens: ~600μs (vs ~1.5ms sequential)

### Memory Usage

| Configuration | Memory per fingerprint |
|---------------|----------------------|
| include_intermediates=false | ~1KB (MinHash only) |
| include_intermediates=true | ~10-50KB |

### Optimization Tips

1. **Enable parallelism** for large documents
2. **Disable intermediates** in production
3. **Use appropriate k** - larger k = fewer shingles = faster
4. **Tune w** - larger w = more reduction but potentially less accurate

---

## Troubleshooting

### Common Issues

#### "NotEnoughTokens" Error

**Problem**: Not enough tokens to create shingles.

**Common Causes:**
- Document too short (fewer than k tokens)
- Empty token list

**Solutions:**
```rust
// Check token count before fingerprinting
if tokens.len() < cfg.k {
    // Use alternative matching strategy
    return Ok(None);
}

// Or handle the error gracefully
match perceptualize_tokens(&tokens, &cfg) {
    Err(PerceptualError::NotEnoughTokens) => {
        // Document too short
    }
    Ok(fp) => process(fp),
}
```

#### Different Fingerprints for Same Text

**Problem**: Different MinHash signatures for identical content.

**Common Causes:**
- Different configuration (k, w, bands, rows, seed)
- Different tokenization

**Solutions:**
```rust
// Ensure consistent configuration
let cfg = PerceptualConfig {
    version: 1,
    k: 9,
    w: 4,
    minhash_bands: 16,
    minhash_rows_per_band: 8,
    seed: 42,
    ..Default::default()
};

// Log config with fingerprint
info!(
    k = cfg.k,
    w = cfg.w,
    seed = cfg.seed,
    minhash_len = fingerprint.minhash.len(),
    "fingerprinted"
);
```

#### Performance Issues

**Problem**: Fingerprinting is slow.

**Solutions:**
1. Enable parallel processing: `cfg.use_parallel = true`
2. Reduce shingle count: increase `k` or `w`
3. Disable intermediates: `cfg.include_intermediates = false`

---

## Integration Guide

### With the UCFP Pipeline

```rust
use ingest::{ingest, CanonicalPayload};
use canonical::canonicalize;
use perceptual::perceptualize_tokens;

// 1. Ingest
let canonical_record = ingest(raw_record, &ingest_cfg)?;

// 2. Extract text and canonicalize
if let Some(CanonicalPayload::Text(text)) = canonical_record.normalized_payload {
    let canonical = canonicalize(&canonical_record.doc_id, &text, &canonical_cfg)?;
    
    // 3. Perceptual fingerprint
    let tokens: Vec<&str> = canonical.tokens.iter().map(|t| t.text.as_str()).collect();
    let fingerprint = perceptualize_tokens(&tokens, &perceptual_cfg)?;
    
    // 4. Index...
}
```

### With Custom Shingling

```rust
use perceptual::{make_shingles_rolling, winnow_minq, minhash_signature, PerceptualConfig};

// Custom pipeline
let shingles = make_shingles_rolling(&tokens, custom_k, seed);
let winnowed = winnow_minq(&shingles, custom_w);

// Extract unique hashes for MinHash
let unique: Vec<u64> = winnowed.iter().map(|s| s.hash).collect();
let minhash = minhash_signature(&unique, expected_len, &cfg)?;
```

### With Existing MinHash

```rust
// If you have pre-computed shingles from another source
use perceptual::minhash_signature;

let unique_shingles = vec![/* pre-computed hashes */];
let cfg = PerceptualConfig::default();
let signature = minhash_signature(&unique_shingles, 128, &cfg)?;
```

---

## Testing

### Running Tests

```bash
# Run all tests
cargo test -p perceptual

# Run with output
cargo test -p perceptual -- --nocapture

# Run specific test
cargo test -p perceptual test_determinism
```

### Test Coverage

Unit tests cover:
- Determinism (same input = same output)
- Parallel vs sequential parity
- Invalid configuration guards
- Rolling-hash arithmetic
- Winnowing behavior
- MinHash signature generation

### Example Programs

```bash
# Fingerprint demo
cargo run --package perceptual --example fingerprint_demo
```

---

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](../../LICENSE) for details.

---

## Contributing

Contributions are welcome! Please ensure:
- All tests pass: `cargo test -p perceptual`
- Documentation is updated
- Examples are provided for new features
- Determinism is maintained

---

## Support

For issues and questions:
- GitHub Issues: [github.com/bravo1goingdark/ufcp/issues](https://github.com/bravo1goingdark/ufcp/issues)
- Documentation: [docs.rs/perceptual](https://docs.rs/perceptual)
