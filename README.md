# Universal Content Fingerprinting (UCFP)

UCFP is a Rust workspace that prepares text and binary payloads for the perceptual and indexing
pipeline. The workspace currently ships three core layers:

- **`ufp_ingest`** - validates ingest metadata, normalizes payloads, and emits deterministic
  `CanonicalIngestRecord` values.
- **`ufp_canonical`** - canonicalizes normalized text into lowercase NFKC strings, token streams,
  and SHA-256 hashes.
- **`ufp_perceptual`** - shingles token streams, performs winnowing, and emits MinHash signatures for
  perceptual similarity.

The root crate exports all three layers and offers:

- `process_record` for ingest + canonicalize.
- `process_record_with_perceptual` for the full ingest -> canonical -> perceptual pipeline.
- `big_text_demo` and the binary in `src/main.rs` to run the bundled `big_text.txt` sample end-to-end.

## Workspace Layout

```
crates/
  ufp_ingest/
    src/            # ingest validation and normalization
    examples/       # ingest_demo.rs
    notes/          # ucfp_ingest.md design notes
  ufp_canonical/
    src/            # canonicalization pipeline
    examples/       # demo.rs, big_text.txt
    doc/            # ufp_canonical.md usage guide
  ufp_perceptual/
    src/            # shingling, winnowing, MinHash
    examples/       # fingerprint_demo.rs
    doc/            # ufp_perceptual.md usage guide
src/lib.rs          # workspace exports + pipeline helpers
src/main.rs         # binary printing perceptual hash for big_text.txt
proto/              # draft schemas and diagrams
docs/               # web docs served via docs/index.html
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
cargo run --package ufp_perceptual --example fingerprint_demo
cargo run                              # end-to-end demo on big_text.txt
```

## API Highlights

```rust
use chrono::{Duration, Utc};
use ucfp::{
    process_record_with_perceptual, CanonicalizeConfig, IngestMetadata, IngestPayload,
    IngestSource, PerceptualConfig, RawIngestRecord,
};

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

let perceptual_cfg = PerceptualConfig {
    k: 5,
    w: 4,
    minhash_bands: 16,
    minhash_rows_per_band: 8,
    seed: 0x5EED,
    use_parallel: false,
};

let (doc, fingerprint) =
    process_record_with_perceptual(record, &cfg, &perceptual_cfg)?;
assert_eq!(doc.canonical_text, "hello world");
assert_eq!(fingerprint.meta.k, 5);
```

`process_record_with_perceptual` returns `PipelineError::Perceptual` if the perceptual stage fails,
`PipelineError::NonTextPayload` when invoked on binary payloads, and `PipelineError::Ingest(...)`
when ingest validation fails. The binary in `src/main.rs` loads
`crates/ufp_canonical/examples/big_text.txt`, runs every layer, and prints the final MinHash
signature.

## Next Steps

- Expand the ingest layer with additional metadata validation rules.
- Extend perceptual matching with storage and similarity search.
- Wire CI to enforce `cargo fmt`, `cargo clippy`, and the workspace test suite.

