# UCFP Canonicalizer

## Purpose

`ufp_canonical` converts raw text into a deterministic representation that downstream layers can
hash, tokenize, and fingerprint. The canonicalizer:

1. normalizes Unicode to NFKC
2. optionally lowercases and strips punctuation
3. collapses whitespace to single spaces
4. emits UTF-8 byte offsets for each token
5. computes a SHA-256 digest of the canonical text

## Key Types

```rust
pub struct CanonicalizeConfig {
    pub strip_punctuation: bool,
    pub lowercase: bool,
}

pub struct CanonicalizedDocument {
    pub canonical_text: String,
    pub tokens: Vec<Token>,
    pub sha256_hex: String,
}

pub struct Token {
    pub text: String,
    pub start: usize,
    pub end: usize,
}
```

Helper functions are exposed for composition:

- `canonicalize(input, cfg)` - full pipeline
- `collapse_whitespace(text)` - deterministic whitespace collapsing
- `tokenize(text)` - convert normalized text into offset-aware tokens
- `hash_text(text)` - SHA-256 helper used by the pipeline

## Example

```rust
use ufp_canonical::{canonicalize, CanonicalizeConfig};

let cfg = CanonicalizeConfig {
    strip_punctuation: true,
    ..Default::default()
};

let doc = canonicalize("Hello, WORLD!  This   is a test.", &cfg);
assert_eq!(doc.canonical_text, "hello world this is a test");
assert_eq!(
    doc.tokens.iter().map(|t| &t.text).collect::<Vec<_>>(),
    vec!["hello", "world", "this", "is", "a", "test"]
);
```

## Testing

Run unit tests with:

```bash
cargo test -p ufp_canonical
```

Tests cover Unicode equivalence (`"Caf\u{00E9}"` vs `"Cafe\u{0301}"`), offset stability for extended glyphs,
and deterministic hashing for ASCII and Unicode payloads.

## Integration

`CanonicalizedDocument` is consumed by `ucfp::process_record`, which orchestrates ingest validation
and canonicalization in a single call.
