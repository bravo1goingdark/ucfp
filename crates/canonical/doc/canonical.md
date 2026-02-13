# UCFP Canonical Crate

> **Deterministic text canonicalization and tokenization for the Universal Content Fingerprinting pipeline**

[![API Docs](https://img.shields.io/badge/docs-api-blue)](https://docs.rs/canonical)
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

The `canonical` crate is the **second stage** in the Universal Content Fingerprinting (UCFP) linear pipeline. It transforms trusted ingest text into a deterministic, versioned representation that downstream layers can hash, tokenize, fingerprint, and embed.

### What This Crate Does

| Function | Description |
|----------|-------------|
| **Normalization** | Unicode NFKC normalization, whitespace collapsing, case folding |
| **Tokenization** | Offset-aware token generation for perceptual fingerprinting |
| **Hashing** | Versioned SHA-256 identity hashes for content comparison |
| **Determinism** | Same input + config → identical output, forever |

### Key Properties

- **Pure**: No I/O, no network, no wall-clock time dependence
- **Deterministic**: Cross-platform consistent output
- **Versioned**: Config version enables tracking and migration
- **Observable**: Structured logging via `tracing`

### Pipeline Position

```
┌─────────┐     ┌──────────┐     ┌──────────────────┐     ┌───────┐     ┌───────┐
│  Ingest │────▶│Canonical │────▶│Perceptual/Semantic│────▶│ Index │────▶│ Match │
│         │     │ (this)   │     │                  │     │       │     │       │
└─────────┘     └──────────┘     └──────────────────┘     └───────┘     └───────┘
```

---

## Quick Start

### Basic Canonicalization

```rust
use canonical::{canonicalize, CanonicalizeConfig};

let cfg = CanonicalizeConfig::default();
let doc = canonicalize("doc-1", "  Hello   WORLD  ", &cfg)?;

assert_eq!(doc.canonical_text, "hello world");
assert_eq!(doc.tokens.len(), 2);
assert!(!doc.sha256_hex.is_empty());
```

### With Unicode Normalization

```rust
use canonical::{canonicalize, CanonicalizeConfig};

let cfg = CanonicalizeConfig {
    normalize_unicode: true,
    ..Default::default()
};

// Both inputs produce the same canonical text
let doc1 = canonicalize("doc-1", "Café", &cfg)?;           // Precomposed
let doc2 = canonicalize("doc-1", "Cafe\u{0301}", &cfg)?;   // Decomposed

assert_eq!(doc1.canonical_text, doc2.canonical_text);
```

---

## Architecture

### Data Flow

```
Input Text
    │
    ▼
┌─────────────────────────────────────────┐
│         Canonicalization Pipeline       │
├─────────────────────────────────────────┤
│  1. Unicode Normalization (Optional)    │
│     - NFKC normalization via input.nfkc()│
│     - Merges composed/decomposed forms   │
├─────────────────────────────────────────┤
│  2. Case Folding (Optional)             │
│     - Locale-free Unicode case folding  │
│     - Consistent across platforms       │
├─────────────────────────────────────────┤
│  3. Whitespace Collapsing               │
│     - Single ASCII space between tokens │
│     - No leading/trailing spaces        │
│     - Collapses multiple delimiters      │
├─────────────────────────────────────────┤
│  4. Tokenization                        │
│     - Contiguous non-delimiter spans    │
│     - Records UTF-8 byte offsets        │
├─────────────────────────────────────────┤
│  5. Hashing                             │
│     - Identity hash (discriminator 0x00)│
│     - Per-token hashes (discriminator)  │
└─────────────────────────────────────────┘
    │
    ▼
CanonicalizedDocument
```

### Key Design Principles

1. **Pure**: No side effects, deterministic output
2. **Versioned**: Config version ensures tracking of behavior changes
3. **Aligned Offsets**: Token offsets reference canonical text
4. **Fast**: O(n) time complexity for all operations

---

## Core Concepts

### Canonical Identity

The canonical identity is a **stable, versioned hash** of the normalized text:

```rust
let identity_hash = SHA-256(
    canonical_version.to_be_bytes() || 0x00 || canonical_text_bytes
);
```

This hash:
- Uniquely identifies content across versions
- Enables efficient deduplication
- Supports backward-compatible migrations

### Token Offsets

Tokens store UTF-8 byte offsets into the canonical text:

```rust
// Canonical text: "hello world"
// Tokens:
Token { text: "hello", start: 0, end: 5 },
Token { text: "world", start: 6, end: 11 }
```

### Version Stability

For a fixed `CanonicalizeConfig::version` and input:
- `canonical_text` is always identical
- `sha256_hex` is always identical
- `tokens` have same content and offsets

---

## Configuration

### CanonicalizeConfig

Central configuration struct controlling canonicalization behavior:

```rust
pub struct CanonicalizeConfig {
    /// Semantic version for canonical behavior (required, > 0)
    pub version: u32,
    
    /// Apply Unicode NFKC normalization
    pub normalize_unicode: bool,
    
    /// Treat punctuation as delimiters
    pub strip_punctuation: bool,
    
    /// Lowercase via Unicode case folding
    pub lowercase: bool,
}
```

**Configuration Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `version` | u32 | Semantic version. Must be > 0. Any behavior change requires a version bump. |
| `normalize_unicode` | bool | Apply NFKC normalization. Recommended: `true` |
| `strip_punctuation` | bool | Remove punctuation as delimiters. Use carefully. |
| `lowercase` | bool | Locale-free case folding. Recommended: `true` |

### Default Configuration

```rust
CanonicalizeConfig::default()
// Enables NFKC + lowercasing, preserves punctuation, version = 1
```

### Configuration Validation

Always validate configuration at startup:

```rust
fn main() {
    let cfg = load_canonical_config();
    if cfg.version == 0 {
        panic!("Invalid canonical config: version cannot be 0");
    }
}
```

---

## API Reference

### Main Function

#### `canonicalize()`

```rust
pub fn canonicalize(
    doc_id: impl Into<String>,
    input: &str,
    cfg: &CanonicalizeConfig,
) -> Result<CanonicalizedDocument, CanonicalError>
```

**Primary entry point** for canonicalization.

**Parameters:**
- `doc_id`: Unique document identifier (required, non-empty)
- `input`: Raw input text to canonicalize
- `cfg`: Canonicalization configuration

**Returns:**
- `Ok(CanonicalizedDocument)`: Canonicalized result
- `Err(CanonicalError)`: Specific error variant

**Validation Rules:**
1. `cfg.version != 0` (reserved and rejected)
2. `doc_id` is non-empty after trimming
3. Canonical text is non-empty after normalization

### Utility Functions

#### `collapse_whitespace()`

```rust
pub fn collapse_whitespace(text: &str) -> String
```

Collapses whitespace without full canonicalization:
- Trims leading/trailing whitespace
- Collapses multiple spaces/tabs/newlines into single spaces

#### `tokenize()`

```rust
pub fn tokenize(text: &str) -> Vec<Token>
```

Offset-aware tokenization of already-canonical text.

#### `hash_text()`

```rust
pub fn hash_text(text: &str) -> String
```

Simple SHA-256 hex digest (version-agnostic).

#### `hash_canonical_bytes()`

```rust
pub fn hash_canonical_bytes(version: u32, bytes: &[u8]) -> String
```

Low-level helper mirroring identity hash computation.

### Data Types

See the [types module](src/types.rs) for complete definitions of:
- `CanonicalizeConfig` - Configuration struct
- `CanonicalizedDocument` - Output structure
- `Token` - Token with text and offsets
- `CanonicalError` - Error variants

---

## Error Handling

### CanonicalError Variants

All errors are typed and cloneable:

| Error | Trigger | Recovery |
|-------|---------|----------|
| `InvalidConfig(msg)` | `version == 0` or future config issues | Fix configuration |
| `MissingDocId` | Empty or whitespace-only doc_id | Provide valid doc_id |
| `EmptyInput` | Text empty after normalization | Provide non-empty content |

### Error Handling Patterns

**Pattern 1: Structured Logging**

```rust
use canonical::{canonicalize, CanonicalError};

match canonicalize(doc_id, text, &cfg) {
    Ok(doc) => {
        tracing::info!(
            doc_id = %doc.doc_id,
            version = doc.canonical_version,
            hash = %doc.sha256_hex,
            "canonicalize_success"
        );
    }
    Err(CanonicalError::EmptyInput) => {
        tracing::warn!(doc_id = %doc_id, "empty_content_skipped");
    }
    Err(e) => {
        tracing::error!(error = %e, "canonicalize_failed");
    }
}
```

**Pattern 2: Graceful Degradation**

```rust
match canonicalize(doc_id, text, &cfg) {
    Ok(doc) => doc,
    Err(CanonicalError::EmptyInput) => {
        // Log and skip empty documents
        warn!("Skipping empty document: {}", doc_id);
        return Ok(());
    }
    Err(e) => return Err(e.into()),
}
```

---

## Examples

### Example 1: Basic Text Processing

```rust
use canonical::{canonicalize, CanonicalizeConfig};

let cfg = CanonicalizeConfig::default();
let doc = canonicalize("doc-1", "  Hello   WORLD  ", &cfg)?;

assert_eq!(doc.canonical_text, "hello world");
assert_eq!(doc.tokens.len(), 2);
assert_eq!(doc.tokens[0].text, "hello");
assert_eq!(doc.tokens[1].text, "world");
assert!(!doc.sha256_hex.is_empty());
```

### Example 2: Unicode Equivalence

```rust
use canonical::{canonicalize, CanonicalizeConfig};

let cfg = CanonicalizeConfig {
    normalize_unicode: true,
    ..Default::default()
};

// Both inputs produce identical canonical text
let doc1 = canonicalize("doc-1", "Café", &cfg)?;           // Precomposed é (U+00E9)
let doc2 = canonicalize("doc-1", "Cafe\u{0301}", &cfg)?;   // Decomposed e + combining accent

assert_eq!(doc1.canonical_text, doc2.canonical_text);
assert_eq!(doc1.sha256_hex, doc2.sha256_hex);
```

### Example 3: Without Lowercasing

```rust
use canonical::{canonicalize, CanonicalizeConfig};

let cfg = CanonicalizeConfig {
    lowercase: false,
    ..Default::default()
};

let doc = canonicalize("doc-1", "Hello World", &cfg)?;
assert_eq!(doc.canonical_text, "Hello World"); // Preserves case
```

### Example 4: Punctuation Stripping

```rust
use canonical::{canonicalize, CanonicalizeConfig};

let cfg = CanonicalizeConfig {
    strip_punctuation: true,
    ..Default::default()
};

let doc = canonicalize("doc-1", "Hello, world! How are you?", &cfg)?;
assert_eq!(doc.canonical_text, "hello world how are you");
```

### Example 5: Version-Aware Hashing

```rust
use canonical::{canonicalize, CanonicalizeConfig};

let cfg_v1 = CanonicalizeConfig { version: 1, ..Default::default() };
let cfg_v2 = CanonicalizeConfig { version: 2, ..Default::default() };

let doc_v1 = canonicalize("doc-1", "hello world", &cfg_v1)?;
let doc_v2 = canonicalize("doc-1", "hello world", &cfg_v2)?;

// Same text but different versions produce different hashes
assert_ne!(doc_v1.sha256_hex, doc_v2.sha256_hex);
```

### Example 6: Using Utility Functions

```rust
use canonical::{collapse_whitespace, tokenize, hash_text};

// Just normalize whitespace
let normalized = collapse_whitespace("  Hello   world  ");
assert_eq!(normalized, "Hello world");

// Tokenize already-canonical text
let tokens = tokenize("hello world test");
assert_eq!(tokens.len(), 3);
assert_eq!(tokens[0].text, "hello");

// Hash arbitrary text
let hash = hash_text("hello world");
```

---

## Best Practices

### 1. Always Enable Unicode Normalization

```rust
let cfg = CanonicalizeConfig {
    normalize_unicode: true,  // Recommended
    ..Default::default()
};
```

This prevents issues with composed vs decomposed characters causing different canonicalizations.

### 2. Use Consistent Configuration

```rust
lazy_static::lazy_static! {
    static ref CANONICAL_CONFIG: CanonicalizeConfig = CanonicalizeConfig {
        version: 1,
        normalize_unicode: true,
        strip_punctuation: false,
        lowercase: true,
    };
}
```

### 3. Validate at Startup

```rust
fn main() {
    let cfg = load_canonical_config();
    if cfg.version == 0 {
        panic!("Invalid canonical config: version cannot be 0");
    }
}
```

### 4. Document Your Version

Include the canonical version in logs, metrics, and stored data:

```rust
info!(
    "Canonicalized doc_id={} version={} hash={}",
    doc.doc_id,
    doc.canonical_version,
    doc.sha256_hex
);
```

### 5. Handle Empty Input Gracefully

```rust
match canonicalize(doc_id, text, &cfg) {
    Ok(doc) => doc,
    Err(CanonicalError::EmptyInput) => {
        // Log and skip empty documents
        warn!("Skipping empty document: {}", doc_id);
        return Ok(());
    }
    Err(e) => return Err(e.into()),
}
```

---

## Performance

### Benchmarks

Actual performance on modern hardware:

| Input Size | Latency | Throughput |
|------------|---------|------------|
| 64 bytes | ~2.5 µs | ~23 MiB/s |
| 512 bytes | ~24 µs | ~20 MiB/s |
| 4 KB | ~180 µs | ~22 MiB/s |
| 32 KB | ~1.4 ms | ~22 MiB/s |

### Time Complexity

All operations are O(n) where n is the input length:
- Unicode normalization: O(n)
- Case folding: O(n)
- Whitespace collapsing: O(n)
- Tokenization: O(n)
- SHA-256 hashing: O(n)

### Memory Usage

- `canonicalize`: Allocates for `canonical_text`, tokens vector, and hashes vector
- `collapse_whitespace`: Allocates one new String
- `tokenize`: Allocates vector of tokens
- `hash_text`: Minimal allocation for hex string

### Optimization Tips

1. **Reuse configurations** - Avoid recreating `CanonicalizeConfig`
2. **Batch processing** - Process multiple documents with the same config
3. **Avoid unnecessary canonicalization** - Cache results when possible
4. **Use `collapse_whitespace`** - If you only need normalized text, not full tokenization

---

## Troubleshooting

### Common Issues

#### "EmptyInput" Error

**Problem**: Input became empty after processing.

**Common Causes:**
- Input is whitespace-only
- All characters were stripped (punctuation + strip_punctuation=true)
- Input is empty string

**Solutions:**
```rust
// Check before canonicalization
if text.trim().is_empty() {
    return Err("Content cannot be empty");
}

// Or catch the error
match canonicalize(doc_id, text, &cfg) {
    Err(CanonicalError::EmptyInput) => {
        eprintln!("Please provide non-empty content");
    }
    // ...
}
```

#### Different Hashes for Same Text

**Problem**: Different canonical hashes for apparently same text.

**Common Causes:**
- Different `CanonicalizeConfig` versions or settings
- Unicode normalization differences

**Solutions:**
```rust
// Ensure consistent configuration
let cfg = CanonicalizeConfig {
    version: 1,
    normalize_unicode: true,
    strip_punctuation: false,
    lowercase: true,
};

// Log config with hash
info!(
    version = cfg.version,
    normalize_unicode = cfg.normalize_unicode,
    lowercase = cfg.lowercase,
    hash = doc.sha256_hex,
    "canonicalized"
);
```

#### Token Offsets Don't Match

**Problem**: Token offsets don't align with expected positions.

**Common Causes:**
- Unicode normalization changed byte positions
- Using offsets on original text instead of canonical text

**Solutions:**
- Always enable `normalize_unicode` for consistent results
- Remember offsets are into the *canonical* text, not original input

---

## Integration Guide

### With the UCFP Pipeline

```rust
use ingest::{ingest, CanonicalPayload};
use canonical::canonicalize;
use perceptual::perceptualize_tokens;
use semantic::semanticize;

// 1. Ingest
let canonical_record = ingest(raw_record, &ingest_cfg)?;

// 2. Extract text
if let Some(CanonicalPayload::Text(text)) = canonical_record.normalized_payload {
    // 3. Canonicalize
    let doc = canonicalize(&canonical_record.doc_id, &text, &canonical_cfg)?;
    
    // 4. Get tokens for perceptual
    let token_refs: Vec<&str> = doc.tokens.iter().map(|t| t.text.as_str()).collect();
    let fingerprint = perceptualize_tokens(&token_refs, &perceptual_cfg)?;
    
    // 5. Get text for semantic
    let embedding = semanticize(&doc.doc_id, &doc.canonical_text, &semantic_cfg)?;
    
    // 6. Index results...
}
```

### Custom Tokenization

If you need different tokenization:

```rust
// Canonicalize first
let doc = canonicalize(id, text, &cfg)?;

// Then apply custom tokenization to canonical_text
let custom_tokens = my_tokenizer(&doc.canonical_text);
```

### Partial Canonicalization

For streaming or chunked processing:

```rust
// Process chunks and concatenate
let mut full_text = String::new();
for chunk in chunks {
    full_text.push_str(&collapse_whitespace(chunk));
}
let doc = canonicalize(id, &full_text, &cfg)?;
```

### Version Migration

When upgrading canonical behavior:

```rust
// Old config
const V1_CONFIG: CanonicalizeConfig = CanonicalizeConfig {
    version: 1,
    normalize_unicode: true,
    strip_punctuation: false,
    lowercase: true,
};

// New config
const V2_CONFIG: CanonicalizeConfig = CanonicalizeConfig {
    version: 2,
    normalize_unicode: true,
    strip_punctuation: true,  // New: strip punctuation
    lowercase: true,
};

// Support both during migration
fn canonicalize_with_version(
    id: &str,
    text: &str,
    target_version: u32,
) -> Result<CanonicalizedDocument, CanonicalError> {
    match target_version {
        1 => canonicalize(id, text, &V1_CONFIG),
        2 => canonicalize(id, text, &V2_CONFIG),
        _ => Err(CanonicalError::InvalidConfig(
            format!("Unknown version: {}", target_version)
        )),
    }
}
```

---

## Testing

### Running Tests

```bash
# Run all tests
cargo test -p canonical

# Run with output
cargo test -p canonical -- --nocapture

# Run specific test
cargo test -p canonical golden_corpus_test
```

### Test Coverage

Unit tests in `src/lib.rs` cover:
- Canonicalization under default config
- Unicode equivalence (composed vs decomposed forms)
- Non-BMP token offsets (e.g., `\u{10348}`)
- Version-aware canonical hashing
- Configuration validation and error paths
- Punctuation stripping behavior
- Case folding behavior

### Example: Golden Corpus Testing

```rust
#[test]
fn golden_corpus_test() {
    let test_cases = vec![
        ("hello world", "hello world"),
        ("  Hello   WORLD  ", "hello world"),
        ("Caf\u{00E9}", "café"),
    ];
    
    let cfg = CanonicalizeConfig::default();
    
    for (input, expected) in test_cases {
        let doc = canonicalize("test", input, &cfg).unwrap();
        assert_eq!(doc.canonical_text, expected);
    }
}
```

---

## Examples

See the `examples/` directory for complete working examples:
- `demo.rs`: Basic canonicalization examples

---

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](../../LICENSE) for details.

---

## Contributing

Contributions are welcome! Please ensure:
- All tests pass: `cargo test -p canonical`
- Documentation is updated
- Examples are provided for new features
- Determinism is maintained

---

## Support

For issues and questions:
- GitHub Issues: [github.com/bravo1goingdark/ufcp/issues](https://github.com/bravo1goingdark/ufcp/issues)
- Documentation: [docs.rs/canonical](https://docs.rs/canonical)
