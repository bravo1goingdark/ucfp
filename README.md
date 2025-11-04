# Universal Content Fingerprinting (UCFP)

Universal Content Fingerprinting is a Rust workspace for building deterministic fingerprints of text
and binary payloads. It gives you the ingest validation, canonical text processing, and perceptual
similarity layers needed to prepare data for large-scale indexing or deduplication systems.

## Why UCFP?

- **Deterministic ingest** – strict metadata validation, canonical IDs, and consistent whitespace
  normalization keep upstream feeds clean.
- **Reproducible canonical text** – Unicode NFKC, lowercasing, punctuation stripping, token offsets,
  and SHA-256 digests are exposed as standalone helpers.
- **Perceptual fingerprints** – rolling-hash shingles, winnowing, and MinHash signatures make
  similarity search and near-duplicate detection straightforward.
- **Single entry point** – the root `ucfp` crate wires every stage into `process_record` and
  `process_record_with_perceptual`, so applications can adopt the full pipeline one call at a time.
- **Built-in observability** – plug in a `PipelineMetrics` recorder to capture latency and results for
  ingest, canonical, and perceptual stages.

## Quickstart

### Prerequisites

- Rust 1.76+ (`rustup toolchain install stable`)
- `cargo` available on your `PATH`

### Build, lint, and test

```bash
cargo fmt --all
cargo clippy --all --all-targets -- -D warnings
cargo test --all
```

### Explore the examples

```bash
cargo run --package ufp_ingest --example ingest_demo
cargo run --package ufp_ingest --example batch_ingest
cargo run --package ufp_canonical --example demo
cargo run --package ufp_canonical --example helpers
cargo run --package ufp_perceptual --example fingerprint_demo
cargo run                              # end-to-end demo on big_text.txt
cargo run --example pipeline_metrics   # observe metrics events
```

## Architecture Overview

UCFP is a layered pipeline:

1. **Ingest (`ufp_ingest`)** – validates metadata, derives deterministic IDs, normalizes text/binary
   payloads, and emits `CanonicalIngestRecord`.
2. **Canonical (`ufp_canonical`)** – converts normalized text into lowercase NFKC strings, token
   streams with byte offsets, and SHA-256 hashes.
3. **Perceptual (`ufp_perceptual`)** – shingles canonical tokens, applies winnowing, and produces
   MinHash fingerprints tuned by `PerceptualConfig`.

The root `ucfp` crate re-exports all public types and orchestrates the stages through:

- `process_record` (ingest + canonicalize),
- `process_record_with_perceptual` (full ingest → canonical → perceptual),
- `process_record_with_*_configs` helpers when explicit configuration objects are needed,
- `big_text_demo` for the bundled integration example,
- `set_pipeline_metrics` / `PipelineMetrics` for observability hooks.

### Layer responsibilities

| Layer           | Responsibilities                                                                                                       | Key types                                     |
|-----------------|------------------------------------------------------------------------------------------------------------------------|-----------------------------------------------|
| `ufp_ingest`    | Required metadata enforcement, timestamp defaulting, control-character stripping, whitespace normalization, UTF-8 decode | `IngestConfig`, `RawIngestRecord`, `CanonicalIngestRecord`, `CanonicalPayload` |
| `ufp_canonical` | Unicode normalization, casing/punctuation policies, tokenization with byte offsets, SHA-256 hashing                     | `CanonicalizeConfig`, `CanonicalizedDocument`, `Token` |
| `ufp_perceptual`| Rolling-hash shingles, winnowing, MinHash signatures with deterministic seeding and optional parallelism                | `PerceptualConfig`, `PerceptualFingerprint`, `WinnowedShingle`, `PerceptualMeta` |

### Pipeline in code

```rust
use chrono::{Duration, Utc};
use ucfp::{
    CanonicalizeConfig, IngestMetadata, IngestPayload, IngestSource, PerceptualConfig,
    RawIngestRecord, process_record_with_perceptual,
};

let canonical_cfg = CanonicalizeConfig::default();
let perceptual_cfg = PerceptualConfig { k: 5, ..Default::default() };
let record = RawIngestRecord {
    id: "ingest-1".into(),
    source: IngestSource::RawText,
    metadata: IngestMetadata {
        tenant_id: "tenant".into(),
        doc_id: "doc".into(),
        received_at: Utc::UNIX_EPOCH + Duration::seconds(1_700_000_000),
        original_source: None,
        attributes: None,
    },
    payload: Some(IngestPayload::Text("  Hello   world  ".into())),
};

let (doc, fingerprint) =
    process_record_with_perceptual(record, &canonical_cfg, &perceptual_cfg)?;
assert_eq!(doc.canonical_text, "hello world");
assert_eq!(fingerprint.meta.k, 5);
```

Failures bubble up as `PipelineError::Ingest(_)`, `PipelineError::Canonical(_)`,
`PipelineError::Perceptual(_)`, or `PipelineError::NonTextPayload`. The CLI binary in `src/main.rs`
invokes `big_text_demo` and prints the final MinHash signature generated from
`crates/ufp_canonical/examples/big_text.txt`.

## Metrics & Observability

Hook a recorder into `set_pipeline_metrics(...)` to track stage-level latency and outcomes. The
`examples/pipeline_metrics.rs` binary provides a reference implementation that prints events such as:

```
ingest: ok (65 us)
canonical: ok (115 us)
perceptual: ok (89 us)
```

Run it with:

```bash
cargo run --example pipeline_metrics
```

## Workspace Layout

```
crates/
  ufp_ingest/        # ingest validation and normalization
  ufp_canonical/     # canonical text pipeline
  ufp_perceptual/    # shingling, winnowing, MinHash
src/                 # workspace exports + CLI demo
tests/               # integration tests (determinism, errors, pipeline)
docs/                # static documentation site
proto/               # schema sketches and diagrams
examples/            # workspace-level demos (metrics, etc.)
```

## Roadmap

- Expand ingest metadata policies and validation rules.
- Add storage/search integrations for perceptual fingerprints.
- Extend the pipeline with cross-modal canonicalizers, fingerprints, and embedding backends:

| Modality | Canonicalizer | Fingerprint | Embedding Model |
|----------|---------------|-------------|-----------------|
| Text     | NFKC + tokenization | MinHash | BGE / E5 |
| Image    | DCT normalization | pHash | CLIP / SigLIP |
| Audio    | Mel-spectrogram | Winnowing | SpeechCLIP / Whisper |
| Video    | Keyframes | Scene hashes | VideoCLIP / XCLIP |
| Document | OCR + layout | Layout graph | LayoutLMv3 |

- Introduce semantic extraction and multi-modality pathways (e.g., text + binary embeddings) feeding the existing canonical/perceptual layers.
- Enrich observability with structured logging backends and metrics exporters.

