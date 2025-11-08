# Universal Content Fingerprinting (UCFP)

UCFP (Universal Content Fingerprint) is an open-source framework for generating unique, reproducible,
and meaning-aware fingerprints across text, audio, image, video, and document payloads. It unifies 
exact hashing, perceptual similarity, and semantic embeddings into one coherent pipeline,so developers
can identify, compare, and link content deterministically and perceptually. Built in Rust for speed and
reliability, UCFP powers use cases such as deduplication, plagiarism detection, data cleaning, 
content provenance, and multimodal search.


## Why UCFP?

- **Deterministic ingest** – strict metadata validation, canonical IDs, and consistent whitespace
  normalization keep upstream feeds clean.
- **Reproducible canonical text** – Unicode NFKC, lowercasing, punctuation stripping, token offsets,
  and SHA-256 digests are exposed as standalone helpers.
- **Perceptual fingerprints** – rolling-hash shingles, winnowing, and MinHash signatures make
  similarity search and near-duplicate detection straightforward.
- **Semantic embeddings** – `ufp_semantic` turns canonical text into ONNX/API-backed dense vectors
  with deterministic fallbacks for offline tiers.
- **Single entry point** – the root `ucfp` crate wires every stage into `process_record` and
  `process_record_with_perceptual`, so applications can adopt the full pipeline one call at a time.
- **Built-in observability** – plug in a `PipelineMetrics` recorder to capture latency and results for
  ingest, canonical, and perceptual stages.

## Use cases

| Use case              | What UCFP contributes                                                                      | Layers & configs                                           |
|-----------------------|-------------------------------------------------------------------------------------------|------------------------------------------------------------|
| Dataset deduplication | Deterministic IDs and canonical hashes collapse byte-identical submissions                | `ufp_ingest` + `IngestConfig`, `ufp_canonical` SHA-256     |
| Plagiarism detection  | Token offsets, shingles, and MinHash detect paraphrased overlaps                          | `ufp_canonical` tokens, `ufp_perceptual` tuned `k`/`w`     |
| Content provenance    | Canonical metadata + perceptual signatures trace assets across feeds, storage, and audits | `ufp_ingest`, `PipelineMetrics`, `PerceptualConfig` seeds  |
| Multimodal search     | Canonical text + binary passthrough feed embedding stores and downstream modalities       | `IngestPayload::Binary`, canonical helpers, embeddings roadmap |

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
cargo run --package ufp_semantic --example embed "Doc Title" "Some text to embed"
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
4. **Semantic (`ufp_semantic`)** – turns canonical text into dense embeddings via ONNX Runtime or
   remote HTTP APIs, then normalizes/stubs vectors based on the configured tier.

The root `ucfp` crate re-exports all public types and orchestrates the stages through:

- `process_record` (ingest + canonicalize),
- `process_record_with_perceptual` (full ingest → canonical → perceptual),
- `process_record_with_*_configs` helpers when explicit configuration objects are needed,
- `big_text_demo` for the bundled integration example,
- `set_pipeline_metrics` / `PipelineMetrics` for observability hooks.

### Layer responsibilities

| Layer           | Responsibilities                                                                                                       | Key types                                                               |
|-----------------|------------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------|
| `ufp_ingest`    | Required metadata enforcement, timestamp defaulting, control-character stripping, whitespace normalization, UTF-8 decode | `IngestConfig`, `RawIngestRecord`, `CanonicalIngestRecord`, `CanonicalPayload` |
| `ufp_canonical` | Unicode normalization, casing/punctuation policies, tokenization with byte offsets, SHA-256 hashing                     | `CanonicalizeConfig`, `CanonicalizedDocument`, `Token`                  |
| `ufp_perceptual`| Rolling-hash shingles, winnowing, MinHash signatures with deterministic seeding and optional parallelism                | `PerceptualConfig`, `PerceptualFingerprint`, `WinnowedShingle`, `PerceptualMeta` |
| `ufp_semantic`  | ONNX/API inference, tokenizer lifecycle management, deterministic stub embeddings for offline or “fast” tiers          | `SemanticConfig`, `SemanticEmbedding`, `SemanticError`                  |

### Documentation map

- [`docs/index.html`](docs/index.html) – workspace-wide architecture overview, diagrams, and glossary.
- [`crates/ufp_ingest/doc/ucfp_ingest.md`](crates/ufp_ingest/doc/ucfp_ingest.md) – ingest invariants, metadata normalization flow, and error taxonomy.
- [`crates/ufp_canonical/doc/ufp_canonical.md`](crates/ufp_canonical/doc/ufp_canonical.md) – canonical transforms, token semantics, and checksum derivation.
- [`crates/ufp_perceptual/doc/ufp_perceptual.md`](crates/ufp_perceptual/doc/ufp_perceptual.md) – shingling/winnowing internals, MinHash tuning guidance, and performance notes.
- [`crates/ufp_semantic/doc/ufp_semantic.md`](crates/ufp_semantic/doc/ufp_semantic.md) – ONNX/API setup, deterministic stub tiers, and embedding configuration tips.

### Config quick reference

| Config type          | Knobs you probably care about                                                | Default highlights                              |
|----------------------|------------------------------------------------------------------------------|-------------------------------------------------|
| `IngestConfig`       | `default_tenant_id`, `doc_id_namespace`, `strip_control_chars`               | v1, deterministic namespace UUID, strip-on      |
| `CanonicalizeConfig` | `normalize_unicode`, `strip_punctuation`, `lowercase`                        | v1, Unicode NFKC + lowercase, punctuation kept  |
| `PerceptualConfig`   | `k`, `w`, `minhash_bands`, `minhash_rows_per_band`, `seed`, `use_parallel`   | v1, 9-token shingles, 16x8 MinHash, serial mode |

```rust
use ucfp::{
    CanonicalizeConfig, IngestConfig, IngestMetadata, IngestPayload, IngestSource, PerceptualConfig,
    RawIngestRecord,
};
use uuid::Uuid;

let ingest_cfg = IngestConfig {
    default_tenant_id: "tenant-acme".into(),
    doc_id_namespace: Uuid::parse_str("3ba60f64-7d5a-11ee-b962-0242ac120002")?,
    strip_control_chars: true,
    ..Default::default()
};

let canonical_cfg = CanonicalizeConfig {
    strip_punctuation: true,
    lowercase: true,
    ..Default::default()
};

let perceptual_cfg = PerceptualConfig {
    k: 7,
    minhash_bands: 32,
    minhash_rows_per_band: 4,
    use_parallel: true,
    ..Default::default()
};

let (doc, fingerprint) = ucfp::process_record_with_perceptual(
    RawIngestRecord {
        id: "demo".into(),
        source: IngestSource::RawText,
        metadata: IngestMetadata {
            tenant_id: None,
            doc_id: None,
            received_at: None,
            original_source: None,
            attributes: None,
        },
        payload: Some(IngestPayload::Text("Streamlined config demo".into())),
    },
    &canonical_cfg,
    &perceptual_cfg,
)?;
assert_eq!(doc.canonical_text, "streamlined config demo");
```

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

## Contributing

We welcome fixes, optimizations, and new modalities. Please read [`CONTRIBUTING.md`](CONTRIBUTING.md)
for the workflow, required checks (`cargo fmt`, `cargo clippy`, `cargo test`), documentation
expectations, and guidance on updating the architecture diagram as the pipeline evolves.
