# UCFP Canonical Layer (`ufp_canonical`)

## Purpose

`ufp_canonical` turns trusted ingest text into a deterministic, versioned
representation that downstream layers can hash, tokenize, fingerprint, and
embed. It is **pure** and **side-effect free**:

- no I/O
- no network
- no dependence on wall-clock time, locale, or hardware

For a fixed configuration and input text, `ufp_canonical` must always produce
the same output on any machine.

Invariant:

> Same input text + same `CanonicalizeConfig` → identical `CanonicalizedDocument`, forever.

Typical callers obtain text from `ufp_ingest::CanonicalIngestRecord` and then
run this crate directly or indirectly via `ucfp::process_record`.

## Core Types

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
    pub token_hashes: Vec<String>,
    pub sha256_hex: String,
    pub canonical_version: u32,
    pub config: CanonicalizeConfig,
}

pub struct Token {
    pub text: String,
    pub start: usize,
    pub end: usize,
}
```

### Configuration (`CanonicalizeConfig`)

- `version` – semantic version for canonical behavior. Any change that can
  affect canonical text, tokenization, or hashes must be accompanied by a
  config version bump wherever configs are persisted.
- `normalize_unicode` – if `true`, apply Unicode NFKC before any other
  transforms. This collapses composed/decomposed forms such as
  `"Caf\u{00E9}"` and `"Cafe\u{0301}"`.
- `strip_punctuation` – if `true`, treat punctuation as delimiters and remove
  them from the canonical text.
- `lowercase` – if `true`, lowercase via Unicode case folding (locale-free).

`CanonicalizeConfig::default()` enables NFKC + lowercasing, leaves punctuation
intact, and starts at `version = 1`.

### Output (`CanonicalizedDocument`)

The canonical document captures everything downstream needs:

- `doc_id` – application-level identifier (copied from ingest); required.
- `canonical_text` – normalized, whitespace-collapsed text.
- `tokens` – sequence of `Token { text, start, end }` where `start`/`end` are
  UTF‑8 byte offsets into `canonical_text`.
- `token_hashes` – stable per-token hashes aligned with `tokens`, computed as
  `SHA-256( canonical_version.to_be_bytes() || 0x01 || token_text_bytes )`.
- `sha256_hex` – **canonical identity hash** computed as:

  ```text
  SHA-256( canonical_version.to_be_bytes() || 0x00 || canonical_text_bytes )
  ```

- `canonical_version` – copy of `config.version` used for canonicalization.
- `config` – a snapshot of the `CanonicalizeConfig` (normalization/profile)
  used to produce this document.

Downstream components should treat `sha256_hex` as the stable identity of the
canonical content for a given `canonical_version`.

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

pub fn hash_canonical_bytes(version: u32, bytes: &[u8]) -> String;
```

### Canonicalization

`canonicalize` is the only function that constructs a full
`CanonicalizedDocument`. It enforces:

- `cfg.version != 0` (reserved and rejected)
- non-empty, trimmed `doc_id`
- non-empty canonical text after normalization (otherwise `CanonicalError::EmptyInput`)

The pipeline is linear-time and deterministic:

1. **Unicode normalization** – optional NFKC via `input.nfkc()`.
2. **Case folding** – optional, locale-free `to_lowercase()`.
3. **Delimiter detection** – any Unicode whitespace, plus punctuation when
   `strip_punctuation = true`.
4. **Whitespace collapsing** – internal state machine inserts a single ASCII
   space between tokens and never at the ends.
5. **Tokenization** – tokens are created as contiguous non-delimiter spans with
   `start`/`end` byte offsets into `canonical_text`.
6. **Canonical hash** – `sha256_hex` is computed from the canonical text bytes
   and `canonical_version` as shown above.

### Utilities

- `collapse_whitespace` – deterministic whitespace collapsing for callers that
  only need a normalized string.
- `tokenize` – offset-aware tokenization of already-canonical text.
- `hash_text` – simple SHA-256 hex digest of arbitrary text (version-agnostic).
- `hash_canonical_bytes` – low-level helper that mirrors the identity hash
  used in `CanonicalizedDocument`.

## Error Semantics

```rust
pub enum CanonicalError {
    InvalidConfig(String),
    MissingDocId,
    EmptyInput,
}
```

- `InvalidConfig` – currently used when `version == 0`; future config
  validation problems should return this variant with a clear message.
- `MissingDocId` – `doc_id` is empty or only whitespace after trimming.
- `EmptyInput` – canonical text is empty after normalization and collapsing.

These errors are surfaced by the workspace root crate as
`PipelineError::Canonical(_)`.

## Determinism & Testing

Unit tests in `src/lib.rs` cover:

- canonicalization under the default config
- Unicode equivalence (composed vs decomposed forms)
- non-BMP token offsets (e.g., `\u{10348}`)
- version-aware canonical hashing
- configuration validation and error paths

Run tests for this crate with:

```bash
cargo test -p ufp_canonical
```

When you intentionally change canonical behavior (e.g., adding a new
normalization rule), bump `CanonicalizeConfig::version`, update any golden
outputs in downstream crates, and ensure the new version is stored wherever
`CanonicalizeConfig` instances are persisted.

## Integration

`CanonicalizedDocument` is consumed by the workspace root crate `ucfp`:

- `ucfp::process_record` runs ingest + canonical and returns a
  `CanonicalizedDocument`.
- `ucfp::process_record_with_perceptual` and
  `ucfp::process_record_with_semantic` extend that pipeline with perceptual
  fingerprints and semantic embeddings.

Indexing layers (see `ufp_index`) should persist `sha256_hex` as the canonical
hash along with any perceptual and semantic representations.