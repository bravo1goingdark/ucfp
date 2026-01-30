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

## Architecture Guidelines

### Linear Dependency Chain

UCFP follows a strict **linear dependency architecture** to maintain clean separation of concerns:

```
ufp_ingest → ufp_canonical → ufp_perceptual/semantic → ufp_index → ufp_match
```

**Rules:**

1. **No circular dependencies**: Crates can only depend on earlier stages in the chain
2. **No umbrella crate dependencies**: Individual crates should not depend on the root `ucfp` crate
3. **Direct dependencies only**: Use explicit crate dependencies (e.g., `ufp_ingest = { path = "../ufp_ingest" }`)
4. **Independence**: Each crate should be usable independently

**Example - Correct dependency in ufp_match:**
```toml
[dependencies]
ufp_ingest = { path = "../ufp_ingest" }
ufp_canonical = { path = "../ufp_canonical" }
ufp_perceptual = { path = "../ufp_perceptual" }
ufp_semantic = { path = "../ufp_semantic" }
ufp_index = { path = "../ufp_index" }
```

**Example - Incorrect (circular):**
```toml
[dependencies]
ucfp = { path = "../../" }  # DON'T DO THIS - creates circular dependency
```

### Dependency Version Consistency

Keep dependency versions consistent across all crates:

- **Rust edition**: All crates use `"2021"`
- **Common dependencies** (thiserror, serde, etc.): Use the same version across all crates
- Check `Cargo.lock` after changes to ensure consistency

### Adding New Crates

When adding new modality crates (image, audio, video, document):

1. Place them in the `crates/` directory
2. Follow the naming convention: `ufp_<modality>`
3. Add to the workspace `Cargo.toml`
4. Position them appropriately in the dependency chain
5. Update `README.md` architecture diagrams
6. Add comprehensive documentation in `crates/ufp_<modality>/doc/`

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
- **Document all public items**: Every `pub struct`, `pub enum`, `pub fn`, and `pub trait` must have
  a doc comment explaining its purpose and usage.

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

### Testing Checklist

Before submitting a PR:

- [ ] `cargo test --all` passes
- [ ] New tests added for new functionality
- [ ] Documentation tests pass (`cargo test --doc`)
- [ ] Examples run successfully

## Documentation and diagrams

- Update the architecture diagram (`docs/architecture.svg`) whenever you add or remove a stage or a
  future modality summary. Keep the SVG in source control; do not check in generated PNGs.
- Each crate has documentation under `crates/<name>/doc/*.md`. Describe configuration knobs,
  error cases, and helper APIs there when they change.
- Link new documentation from `README.md` (or relevant crate `README`s) to keep discovery simple.
- When adding metrics, include a short explanation in the relevant crate doc so dashboards remain
  self-explanatory.

### Documentation Standards

All crate documentation should include:

1. **Purpose statement** - What the crate does
2. **Architecture position** - Where it fits in the pipeline
3. **Core types** - Main structs and enums with field explanations
4. **Public API** - All public functions with examples
5. **Configuration** - Detailed config options
6. **Error handling** - Error types and when they're raised
7. **Examples** - Practical code examples
8. **Best practices** - Recommended usage patterns
9. **Troubleshooting** - Common issues and solutions
10. **Integration** - How to use with other crates

See `crates/ufp_ingest/doc/ucfp_ingest.md` and `crates/ufp_canonical/doc/ufp_canonical.md` as examples.

## Future modalities

The roadmap includes image, audio, video, and document canonicalizers plus modality-specific
fingerprints/embeddings. If you prototype any of these:

### Creating a New Modality Crate

1. **Scaffold the crate**:
   ```bash
   cargo new --lib crates/ufp_<modality>
   ```

2. **Configure Cargo.toml**:
   ```toml
   [package]
   name = "ufp_<modality>"
   version = "0.1.0"
   edition = "2021"
   
   [dependencies]
   # Add dependencies on earlier stages only
   ufp_ingest = { path = "../ufp_ingest" }
   ufp_canonical = { path = "../ufp_canonical" }
   # ... other dependencies
   ```

3. **Create documentation** at `crates/ufp_<modality>/doc/ufp_<modality>.md`:
   - Describe the data contract
   - Explain configuration knobs
   - Show integration points
   - Document dependencies (FFmpeg, image libraries, etc.)
   - Provide reproducible test fixtures

4. **Update workspace**:
   - Add to root `Cargo.toml` workspace members
   - Update `README.md` architecture section
   - Update `docs/architecture.svg`

5. **Follow existing patterns**:
   - Mirror the structure of `ufp_perceptual` or `ufp_semantic`
   - Use the same error handling patterns
   - Provide deterministic fallbacks
   - Include comprehensive tests

### Modality Requirements

Each new modality should provide:

- **Canonicalization strategy**: How to normalize the content (e.g., image DCT normalization, audio Mel-spectrogram)
- **Fingerprinting**: Perceptual fingerprints (e.g., pHash, audio shingling)
- **Embedding**: Dense vector representations (e.g., CLIP, Whisper)
- **Configuration**: Config struct mirroring `PerceptualConfig` style
- **Error handling**: Descriptive error types
- **Determinism**: Reproducible outputs for same input + config
- **Documentation**: Comprehensive guide following the standards above

## Opening a pull request

Include the following in every PR description:

- A short summary of the motivation and user-visible change.
- Testing evidence: `cargo fmt`, `cargo clippy`, `cargo test`, plus any extras (`cargo bench`,
  screenshots, SVG diffs, etc.).
- Links to issues the PR resolves (`Closes #123`) and any known follow-up work.
- Screenshots or SVG diffs when touching docs/diagrams so reviewers can see the change without
  rebuilding assets locally.
- Rollout or migration notes if the change requires operational follow-up.
- Architecture impact: If changing dependencies, explain why and show the new dependency graph.

### PR Checklist

- [ ] Code formatted with `cargo fmt --all`
- [ ] No clippy warnings: `cargo clippy --all --all-targets -- -D warnings`
- [ ] All tests pass: `cargo test --all`
- [ ] Documentation updated (crate docs, README.md if needed)
- [ ] Architecture diagram updated if adding/removing crates
- [ ] No circular dependencies introduced
- [ ] Dependency versions consistent across crates

## Need help?

If you get stuck, open a discussion thread or draft PR and call it out in Discord. Mentions are
welcome for reviewers listed in `CODEOWNERS`, and we are happy to pair on tricky canonicalizer or
metrics changes. 

### Common Issues

**Circular dependency errors**: If you see errors about circular dependencies, check that you're
not importing from the `ucfp` umbrella crate in individual crates. Use direct crate dependencies
instead.

**Documentation warnings**: Run `cargo doc --all` to check for broken links or missing docs.

**Test failures**: If tests fail after your changes, check:
- Are you preserving determinism? Same input + config should produce same output
- Did you update golden/snapshot tests if behavior intentionally changed?
- Are tests isolated? They shouldn't depend on external services or files.

Thanks again for contributing to UCFP!
