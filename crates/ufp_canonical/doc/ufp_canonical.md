# UCFP Canonicalizer

## Purpose

`ufp_canonical` converts raw text into a deterministic representation that downstream layers can
hash, tokenize, and fingerprint. The canonicalizer:

1. normalizes Unicode to NFKC (configurable)
2. optionally lowercases and strips punctuation
3. collapses whitespace to single spaces
4. emits UTF-8 byte offsets for each token
5. computes a SHA-256 digest of the canonical text

## Key Types

```rust
pub struct CanonicalizeConfig {
    pub version: u32,
    pub normalize_unicode: bool,
    pub strip_punctuation: bool,
    pub lowercase: bool,
}

pub struct CanonicalizedDocument {
    pub doc_id: String,
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

- `canonicalize(doc_id, input, cfg)` - full pipeline with error reporting
- `collapse_whitespace(text)` - deterministic whitespace collapsing
- `tokenize(text)` - convert normalized text into offset-aware tokens
- `hash_text(text)` - SHA-256 helper used by the pipeline

## Public API

```rust
pub fn canonicalize(
    doc_id: impl Into<String>,
    input: &str,
    cfg: &CanonicalizeConfig,
) -> Result<CanonicalizedDocument, CanonicalError>;
pub fn collapse_whitespace(text: &str) -> String;
pub fn tokenize(text: &str) -> Vec<Token>;
pub fn hash_text(text: &str) -> String;
```

`canonicalize` returns a `CanonicalizedDocument` that contains the normalized text, doc identifier,
byte-aware tokens, and a deterministic SHA-256 digest. Failures are surfaced via `CanonicalError`:
empty inputs, invalid configuration versions, or unsupported combinations return descriptive
variants. `collapse_whitespace` enforces the one-space rule that keeps token boundaries stable,
`tokenize` exposes the offset calculation logic for downstream consumers, and `hash_text` produces
the hex-encoded checksum used for deduplication and integrity checks.

## Example

```rust
use ufp_canonical::{canonicalize, CanonicalizeConfig};

let cfg = CanonicalizeConfig {
    strip_punctuation: true,
    ..Default::default()
};

let doc = canonicalize("doc-demo", "Hello, WORLD!  This   is a test.", &cfg)
    .expect("canonicalization succeeds");
assert_eq!(doc.canonical_text, "hello world this is a test");
assert_eq!(
    doc.tokens.iter().map(|t| &t.text).collect::<Vec<_>>(),
    vec!["hello", "world", "this", "is", "a", "test"]
);
assert_eq!(doc.doc_id, "doc-demo");
```

### Examples

- `cargo run --package ufp_canonical --example demo` - canonicalizes a larger document sourced from disk.
- `cargo run --package ufp_canonical --example helpers` - demonstrates helper utilities (`collapse_whitespace`, `tokenize`, `hash_text`) alongside custom configuration.

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
