# Contributing to UCFP

Thanks for building UCFP with us! This guide explains how to set up your environment, run the same
checks as CI, and contribute new functionality (including future modalities) with confidence.

## Prerequisites

- Rust toolchain `stable` (Rust 1.76 or newer). Install via `rustup toolchain install stable`.
- `cargo` available on your `PATH`.
- Optional: [`just`](https://github.com/casey/just) or other task runners, though all commands below
  can be run directly.

## Workflow overview

1. **Fork and branch** – create a feature branch per change.
2. **Keep code formatted** – run `cargo fmt --all`.
3. **Lint and test** – run the same commands as CI (see below). Fix warnings before opening a PR.
4. **Document as you go** – keep `README.md`, `docs/architecture.svg`, and crate-specific docs under
   `crates/*/doc` in sync with the behavior you changed.
5. **Open a pull request** – describe the motivation, the changes, and how you tested. Include links
   to any issue(s) addressed.

## Required checks (mirrors CI)

```bash
cargo fmt --all -- --check
cargo clippy --all --all-targets -- -D warnings
cargo test --all
```

Run them locally before pushing. CI executes the exact same trio in `.github/workflows/ci.yml`.

## Coding guidelines

- Prefer small, focused commits with descriptive messages (e.g. `canonical: add Token::as_ref`).
- Avoid unnecessary allocations or clones on hot paths. If you introduce a new convenience helper,
  gate it behind an `impl` or borrow where possible (see `Token: AsRef<str>` for inspiration).
- Reuse the workspace error enums (`PipelineError`, `IngestError`, etc.) instead of introducing ad
  hoc string errors.
- When adding pipeline stages or metrics, ensure the `PipelineMetrics` spans are updated so latency
  reporting remains accurate.
- Keep public APIs documented with `///` comments and add examples/tests when behavior changes.

## Tests

- Unit tests live next to the code (`src/lib.rs`, `crates/*/src/lib.rs`).
- Integration tests live under `tests/`.
- If you add a new ingestion edge case, include coverage in `tests/error_handling.rs` or the
  relevant crate.
- Benchmark additions belong in `benches/` (Criterion is already wired up).

## Documentation & diagrams

- Update the architecture diagram (`docs/architecture.svg`) whenever you add/remove a stage or a
  future modality summary. Keep the diagram source-controlled—no generated PNGs.
- Each crate has a doc in `crates/<name>/doc/*.md`. Describe any new configuration knobs, error
  cases, or helper APIs there.
- Link to the new documentation from `README.md` when it helps discoverability.

## Future modalities

The roadmap includes image, audio, video, and document canonicalizers plus modality-specific
fingerprints/embeddings. If you prototype any of these:

- Describe the data contract and configuration knobs in a new doc (e.g.
  `crates/ufp_image/doc/ufp_image.md`).
- Explain how the modality feeds the canonical/perceptual/semantic layers so other contributors see
  the integration points.
- Update the architecture diagram to highlight the new flow.

## Opening a pull request

Please include the following in every PR description:

- A short summary of the change.
- Testing evidence (`cargo fmt`, `cargo clippy`, `cargo test`, plus any extras like `cargo bench`).
- Screenshots or SVG diffs when touching docs/diagrams.
- Any known follow-up work (if applicable).

Thanks again for contributing to UCFP!
