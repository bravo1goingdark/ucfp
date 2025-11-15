# Contributing to UCFP

Thanks for building UCFP with us! This guide explains how to set up your environment, run the same
checks as CI, and contribute new functionality (including future modalities) with confidence.

## Quick start checklist

1. Fork the repository and clone your fork locally.
2. Install the Rust stable toolchain plus `rustfmt` and `clippy` components (see prerequisites).
3. Run `cargo fmt --all`, `cargo clippy --all --all-targets -- -D warnings`, and `cargo test --all`.
4. Keep the documentation you touch (`README.md`, crate docs, diagrams) in sync with your code.
5. Push your branch and open a pull request once all local checks pass.

## Prerequisites

- Rust toolchain `stable` (Rust 1.76 or newer). Install via `rustup toolchain install stable` and
  make it default with `rustup default stable`.
- `rustfmt` and `clippy` components so that formatting/linting matches CI:
  `rustup component add rustfmt clippy`.
- `cargo` available on your `PATH` (provided automatically by `rustup`).
- Optional but recommended:
  - [`just`](https://github.com/casey/just) for the shorthand recipes under `justfile`.
  - [`cargo-nextest`](https://nexte.st/) for faster integration test runs.
  - `protoc` 3.21+ if you need to regenerate files under `proto/`.
- Ensure git can use long filenames on Windows (`git config core.longpaths true`) to avoid checkout issues.

## Local development workflow

- **Start with an issue**: comment on or create an issue so others know you are working on a change.
- **Branch per change**: use descriptive names such as `feature/pipeline-caching` or
  `fix/canonical-nil-bytes`.
- **Keep commits focused**: prefer small, reviewable commits with actionable messages, e.g.
  `canonical: add Token::as_ref`.
- **Stay up to date**: regularly `git fetch` and rebase on the latest `main` to minimize conflicts.
- **Document as you go**: update `README.md`, `docs/architecture.svg`, and crate docs under
  `crates/*/doc` whenever behavior changes.

### Required checks (mirrors CI)

Run the same commands that CI executes before pushing:

```bash
cargo fmt --all -- --check
cargo clippy --all --all-targets -- -D warnings
cargo test --all
```

If your change only touches a subset of crates, you can run the commands with `-p <crate>` while
iterating, but make sure to run the full workspace versions before submitting a PR. CI executes the
exact triplet defined in `.github/workflows/ci.yml`.

### Additional guidance

- Avoid unnecessary allocations or clones on hot paths. If you need convenience helpers, prefer
  borrowing APIs (see `Token: AsRef<str>` for reference) or scoped `impl`s.
- Reuse the shared error enums (`PipelineError`, `IngestError`, etc.) instead of ad hoc string
  errors so that telemetry remains consistent.
- Update `PipelineMetrics` spans whenever you create new pipeline stages or metrics so latency
  reporting stays accurate.
- Keep public APIs documented with `///` comments and include examples or tests when behavior changes.
- When touching `proto/` definitions, regenerate the artifacts and commit the updated files as part
  of the same change.

## Tests and benchmarks

- Unit tests live alongside their code (`src/lib.rs`, `crates/*/src/*.rs`). Co-locate new tests with
  the functionality they cover.
- Integration tests live under `tests/`. When adding a new ingestion edge case, extend
  `tests/error_handling.rs` or an equivalent crate-specific file.
- Snapshot-style tests and fixtures belong under `tests/fixtures/` or crate-local fixture folders so
  they can be shared across crates.
- Benchmark additions belong in `benches/` (Criterion is already wired up). Run `cargo bench -p <crate>`
  when you change hot paths and paste results into the PR if they inform the review.
- If you add a new feature flag, add tests for both enabled and disabled states so CI validates both.

## Documentation and diagrams

- Update the architecture diagram (`docs/architecture.svg`) whenever you add or remove a stage or a
  future modality summary. Keep the SVG in source control; do not check in generated PNGs.
- Each crate has documentation under `crates/<name>/doc/*.md`. Describe configuration knobs,
  error cases, and helper APIs there when they change.
- Link new documentation from `README.md` (or relevant crate `README`s) to keep discovery simple.
- When adding metrics, include a short explanation in the relevant crate doc so dashboards remain
  self-explanatory.

## Future modalities

The roadmap includes image, audio, video, and document canonicalizers plus modality-specific
fingerprints/embeddings. If you prototype any of these:

- Describe the data contract and configuration knobs in a new doc (for example
  `crates/ufp_image/doc/ufp_image.md`).
- Explain how the modality feeds the canonical, perceptual, and semantic layers so other contributors
  can understand the integration points.
- Enumerate any dependencies (FFmpeg, image libraries, etc.) and how to obtain reproducible test
  fixtures.
- Update the architecture diagram to highlight the new flow and reference the doc from `README.md`.

## Opening a pull request

Include the following in every PR description:

- A short summary of the motivation and user-visible change.
- Testing evidence: `cargo fmt`, `cargo clippy`, `cargo test`, plus any extras (`cargo bench`,
  screenshots, SVG diffs, etc.).
- Links to issues the PR resolves (`Closes #123`) and any known follow-up work.
- Screenshots or SVG diffs when touching docs/diagrams so reviewers can see the change without
  rebuilding assets locally.
- Rollout or migration notes if the change requires operational follow-up.

## Need help?

If you get stuck, open a discussion thread or draft PR and call it out in Discord. Mentions are
welcome for reviewers listed in `CODEOWNERS`, and we are happy to pair on tricky canonicalizer or
metrics changes. Thanks again for contributing to UCFP!
