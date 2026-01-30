# UCFP Canonical Layer (`ufp_canonical`)

## Purpose

`ufp_canonical` turns trusted ingest text into a deterministic, versioned representation that downstream layers can hash, tokenize, fingerprint, and embed. It is **pure** and **side-effect free**:

- No I/O
- No network
- No dependence on wall-clock time, locale, or hardware

**Invariant:**

> Same input text + same `CanonicalizeConfig` → identical `CanonicalizedDocument`, forever.

This is the **second stage** in the UCFP linear pipeline:
```
ufp_ingest → ufp_canonical → ufp_perceptual/semantic → ufp_index → ufp_match
```

Typical callers obtain text from `ufp_ingest::CanonicalIngestRecord` and then run this crate directly or indirectly via the individual pipeline orchestration.

## Core Types

### CanonicalizeConfig

```rust
pub struct CanonicalizeConfig {
    pub version: u32,
    pub normalize_unicode: bool,
    pub strip_punctuation: bool,
    pub lowercase: bool,
}
```

**Configuration Fields:**

- **`version`** (u32) - Semantic version for canonical behavior. Any change that can affect canonical text, tokenization, or hashes must be accompanied by a config version bump wherever configs are persisted. Start at 1. Version 0 is reserved and rejected.

- **`normalize_unicode`** (bool) - If `true`, apply Unicode NFKC normalization before any other transforms. This collapses composed/decomposed forms such as `"Caf\u{00E9}"` and `"Cafe\u{0301}"` (both become "Cafe" with the combining accent merged). This is **highly recommended** for most use cases as it ensures consistent canonicalization across different Unicode input forms.

- **`strip_punctuation`** (bool) - If `true`, treat punctuation as delimiters and remove them from the canonical text. This can be useful for text comparison but may change the meaning. Use carefully.

- **`lowercase`** (bool) - If `true`, lowercase via Unicode case folding (locale-free). This ensures consistent lowercase behavior regardless of system locale. Recommended for case-insensitive matching.

**Default Configuration:**
```rust
CanonicalizeConfig::default() // Enables NFKC + lowercasing, leaves punctuation intact, version = 1
```

### CanonicalizedDocument

```rust
pub struct CanonicalizedDocument {
    pub doc_id: String,
    pub canonical_text: String,
    pub tokens: Vec<Token>,
    pub token_hashes: Vec<String>,
    pub sha256_hex: String,
    pub canonical_version: u32,
    pub config: CanonicalizeConfig,
}
```

**Output Fields:**

- **`doc_id`** - Application-level identifier (copied from ingest); required.

- **`canonical_text`** - Normalized, whitespace-collapsed text. This is the "source of truth" text that downstream stages should use.

- **`tokens`** - Sequence of `Token { text, start, end }` where `start`/`end` are UTF-8 byte offsets into `canonical_text`. These tokens are used by the perceptual fingerprinting stage.

- **`token_hashes`** - Stable per-token hashes aligned with `tokens`, computed as:
  ```
  SHA-256(canonical_version.to_be_bytes() || 0x01 || token_text_bytes)
  ```

- **`sha256_hex`** - **Canonical identity hash** computed as:
  ```
  SHA-256(canonical_version.to_be_bytes() || 0x00 || canonical_text_bytes)
  ```
  This is the unique identifier for the canonical content. Downstream components should treat this as the stable identity.

- **`canonical_version`** - Copy of `config.version` used for canonicalization.

- **`config`** - A snapshot of the `CanonicalizeConfig` used to produce this document. This allows you to recreate the exact canonicalization if needed.

### Token

```rust
pub struct Token {
    pub text: String,
    pub start: usize,
    pub end: usize,
}
```

**Token Fields:**

- **`text`** - The token text content.
- **`start`** - Byte offset (inclusive) in the canonical text.
- **`end`** - Byte offset (exclusive) in the canonical text.

## Public API

### Main Function

```rust
/// Canonicalize text into a deterministic representation.
pub fn canonicalize(
    doc_id: impl Into<String>,
    input: &str,
    cfg: &CanonicalizeConfig,
) -> Result<CanonicalizedDocument, CanonicalError>;
```

**Parameters:**
- `doc_id` - Unique document identifier (required, non-empty)
- `input` - Raw input text to canonicalize
- `cfg` - Canonicalization configuration

**Returns:**
- `Ok(CanonicalizedDocument)` on success
- `Err(CanonicalError)` if validation fails

**Validation Rules:**
1. `cfg.version != 0` (reserved and rejected)
2. `doc_id` is non-empty after trimming
3. Canonical text is non-empty after normalization (otherwise `CanonicalError::EmptyInput`)

### Utility Functions

```rust
/// Deterministic whitespace collapsing for callers that only need a normalized string.
pub fn collapse_whitespace(text: &str) -> String;

/// Offset-aware tokenization of already-canonical text.
pub fn tokenize(text: &str) -> Vec<Token>;

/// Simple SHA-256 hex digest of arbitrary text (version-agnostic).
pub fn hash_text(text: &str) -> String;

/// Low-level helper that mirrors the identity hash used in CanonicalizedDocument.
pub fn hash_canonical_bytes(version: u32, bytes: &[u8]) -> String;
```

## Canonicalization Pipeline

The `canonicalize` function implements a linear-time, deterministic pipeline:

### Stage 1: Unicode Normalization (Optional)

If `cfg.normalize_unicode = true`:
- Apply NFKC normalization via `input.nfkc()`
- Merges composed characters: "é" (U+00E9) and "e" + combining accent (U+0301) both become the same form
- Normalizes compatibility characters

**Example:**
```rust
// Input: "Caf\u{00E9}" (precomposed) or "Cafe\u{0301}" (decomposed)
// After NFKC: "Café" (same canonical form)
```

### Stage 2: Case Folding (Optional)

If `cfg.lowercase = true`:
- Apply locale-free Unicode case folding via `to_lowercase()`
- Ensures consistent lowercase regardless of system locale

**Example:**
```rust
// Input: "Hello World"
// After lowercasing: "hello world"

// Input: "İstanbul" (Turkish dotted capital I)
// After lowercasing: "i̇stanbul" (correct Unicode handling)
```

### Stage 3: Delimiter Detection

The pipeline detects token boundaries using:
- Unicode whitespace characters
- Punctuation characters (if `cfg.strip_punctuation = true`)

Common delimiters: spaces, tabs, newlines, punctuation marks

### Stage 4: Whitespace Collapsing

An internal state machine:
- Inserts a single ASCII space between tokens
- Never inserts space at the beginning or end
- Collapses multiple consecutive delimiters to a single space

**Example:**
```rust
// Input: "  Hello   world  \n\t test  "
// After collapsing: "Hello world test"
```

### Stage 5: Tokenization

Creates tokens as contiguous non-delimiter spans:
- Records byte offsets in the canonical text
- Each token knows its exact position
- Tokens are stored in order

**Example:**
```rust
// Canonical text: "hello world"
// Tokens: [
//   Token { text: "hello", start: 0, end: 5 },
//   Token { text: "world", start: 6, end: 11 }
// ]
```

### Stage 6: Hashing

Computes the canonical identity hash:
```rust
let mut hasher = Sha256::new();
hasher.update(&canonical_version.to_be_bytes());
hasher.update(&[0x00]); // discriminator byte
hasher.update(canonical_text.as_bytes());
let sha256_hex = format!("{:x}", hasher.finalize());
```

Also computes per-token hashes using discriminator byte `0x01`.

## Error Semantics

```rust
pub enum CanonicalError {
    InvalidConfig(String),
    MissingDocId,
    EmptyInput,
}
```

**Error Variants:**

- **`InvalidConfig`** - Currently used when `version == 0`. Future config validation problems will return this variant with a clear message.

- **`MissingDocId`** - `doc_id` is empty or only whitespace after trimming. This is a required field.

- **`EmptyInput`** - Canonical text is empty after normalization and collapsing. This prevents processing of meaningless inputs.

These errors are surfaced by the workspace root crate as `PipelineError::Canonical(_)`.

## Examples

### Basic Canonicalization

```rust
use ufp_canonical::{canonicalize, CanonicalizeConfig};

let cfg = CanonicalizeConfig::default();
let doc = canonicalize("doc-1", "  Hello   WORLD  ", &cfg)?;

assert_eq!(doc.canonical_text, "hello world");
assert_eq!(doc.tokens.len(), 2);
assert_eq!(doc.tokens[0].text, "hello");
assert_eq!(doc.tokens[1].text, "world");
assert!(!doc.sha256_hex.is_empty());
```

### Unicode Normalization

```rust
use ufp_canonical::{canonicalize, CanonicalizeConfig};

let cfg = CanonicalizeConfig {
    normalize_unicode: true,
    ..Default::default()
};

// Both inputs should produce the same canonical text
let doc1 = canonicalize("doc-1", "Café", &cfg)?;           // Precomposed é (U+00E9)
let doc2 = canonicalize("doc-1", "Cafe\u{0301}", &cfg)?;   // Decomposed e + combining accent

assert_eq!(doc1.canonical_text, doc2.canonical_text);
```

### Without Lowercasing

```rust
use ufp_canonical::{canonicalize, CanonicalizeConfig};

let cfg = CanonicalizeConfig {
    lowercase: false,
    ..Default::default()
};

let doc = canonicalize("doc-1", "Hello World", &cfg)?;
assert_eq!(doc.canonical_text, "Hello World"); // Preserves case
```

### Punctuation Stripping

```rust
use ufp_canonical::{canonicalize, CanonicalizeConfig};

let cfg = CanonicalizeConfig {
    strip_punctuation: true,
    ..Default::default()
};

let doc = canonicalize("doc-1", "Hello, world! How are you?", &cfg)?;
assert_eq!(doc.canonical_text, "hello world how are you");
```

### Version-Aware Hashing

```rust
use ufp_canonical::{canonicalize, CanonicalizeConfig};

let cfg_v1 = CanonicalizeConfig { version: 1, ..Default::default() };
let cfg_v2 = CanonicalizeConfig { version: 2, ..Default::default() };

let doc_v1 = canonicalize("doc-1", "hello world", &cfg_v1)?;
let doc_v2 = canonicalize("doc-1", "hello world", &cfg_v2)?;

// Same text but different versions produce different hashes
assert_ne!(doc_v1.sha256_hex, doc_v2.sha256_hex);
```

### Using Utility Functions

```rust
use ufp_canonical::{collapse_whitespace, tokenize, hash_text};

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

## Determinism Guarantees

### Cross-Platform Consistency

`ufp_canonical` produces identical results on:
- Different operating systems (Linux, macOS, Windows)
- Different architectures (x86_64, ARM64)
- Different Rust versions (within reason)
- Different locales (always uses Unicode standard, not system locale)

### Version Stability

For a fixed `CanonicalizeConfig::version` and input text:
- `canonical_text` will always be identical
- `sha256_hex` will always be identical
- `tokens` will always have the same content and offsets
- `token_hashes` will always be identical

### Upgrade Path

When intentionally changing canonical behavior:
1. Bump `CanonicalizeConfig::version`
2. Update any golden outputs in downstream tests
3. Document the change
4. Consider backfilling existing data or running dual pipelines during migration

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

Store your `CanonicalizeConfig` alongside your pipeline configuration and reuse it:

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

## Testing

### Running Tests

```bash
# Run all tests
cargo test -p ufp_canonical

# Run with output
cargo test -p ufp_canonical -- --nocapture
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

## Performance Considerations

### Time Complexity

All operations are O(n) where n is the input length:
- Unicode normalization: O(n)
- Case folding: O(n)
- Whitespace collapsing: O(n)
- Tokenization: O(n)
- SHA-256 hashing: O(n)

### Memory Allocations

- `canonicalize`: Allocates for `canonical_text`, tokens vector, and hashes vector
- `collapse_whitespace`: Allocates one new String
- `tokenize`: Allocates vector of tokens
- `hash_text`: Minimal allocation for hex string

### Optimization Tips

1. **Reuse configurations** - Avoid recreating `CanonicalizeConfig`
2. **Batch processing** - Process multiple documents with the same config
3. **Avoid unnecessary canonicalization** - Cache results when possible
4. **Use `collapse_whitespace`** - If you only need normalized text, not full tokenization

## Integration

`CanonicalizedDocument` is consumed by downstream stages:

```rust
use ufp_ingest::{ingest, CanonicalPayload};
use ufp_canonical::canonicalize;
use ufp_perceptual::perceptualize_tokens;
use ufp_semantic::semanticize;

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

## Troubleshooting

### "EmptyInput" Error

**Cause:** Input became empty after processing

**Solutions:**
- Check if input is whitespace-only
- Check if all characters were stripped (punctuation + strip_punctuation=true)
- Skip empty documents or provide meaningful error messages

### Different Hashes for Same Text

**Cause:** Different `CanonicalizeConfig` versions or settings

**Solutions:**
- Ensure consistent configuration
- Log the config version with the hash
- Store config snapshot with results

### Token Offsets Don't Match

**Cause:** Unicode normalization changed byte positions

**Solutions:**
- Always enable `normalize_unicode` for consistent results
- Remember offsets are into the *canonical* text, not original input

## Advanced Topics

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

### Version Migration Strategy

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

## License

Licensed under the Apache License, Version 2.0.
