# Universal Content Fingerprinting (UCFP)

UCFP is a Rust workspace that prepares text and binary payloads for the perceptual and indexing
pipeline. The workspace currently ships two layers:

- **`ufp_ingest`** - validates ingest metadata, normalizes payloads, and emits deterministic
  `CanonicalIngestRecord` values.
- **`ufp_canonical`** - canonicalizes normalized text into lowercase NFKC strings, token streams, and
  SHA-256 hashes.

The root crate exports both layers and offers `process_record`, a glue function that feeds a
`RawIngestRecord` through ingest validation and canonicalization.

## Workspace Layout

```
crates/
  ufp_ingest/
    src/            # ingest validation and normalization
    examples/       # ingest_demo.rs
    notes/          # ucfp_ingest.md design notes
  ufp_canonical/
    src/            # canonicalization pipeline
    examples/       # demo.rs
    doc/            # ufp_canonical.md usage guide
src/lib.rs          # workspace exports + process_record glue
proto/              # draft schemas and diagrams
```

## Getting Started

Verify the entire workspace:

```bash
cargo build --workspace
cargo test --workspace --all-features
```

Linting and formatting gates:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -D warnings
```

Run the examples:

```bash
cargo run --package ufp_ingest --example ingest_demo
cargo run --package ufp_ingest --example batch_ingest
cargo run --package ufp_canonical --example demo
cargo run --package ufp_canonical --example helpers
```

## API Highlights

```rust
use chrono::{Duration, Utc};
use ucfp::{process_record, CanonicalizeConfig, IngestMetadata, IngestPayload, IngestSource, RawIngestRecord};

let cfg = CanonicalizeConfig::default();
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

let doc = process_record(record, &cfg)?;
assert_eq!(doc.canonical_text, "hello world");
```

`process_record` returns `PipelineError::NonTextPayload` when invoked on binary payloads and
`PipelineError::Ingest(...)` when ingest validation fails.

## Next Steps

- Expand the ingest layer with additional metadata validation rules.
- Feed canonicalized text into the perceptual layer for shingling and minhash.
- Wire CI to enforce `cargo fmt`, `cargo clippy`, and the workspace test suite.
