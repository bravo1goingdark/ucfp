# Repository Guidelines

## Project Structure & Module Organization
- `Cargo.toml` at the root defines the workspace and shared metadata.
- `crates/ufp_canonical/src/lib.rs` owns text canonicalization, token offsets, and checksum logic; `examples/demo.rs` shows usage.
- `crates/ufp_ingest/src/lib.rs` validates ingest metadata and payloads; reuse its `examples/ingest_demo.rs` for end-to-end flow tests.
- `proto/` hosts draft schemas and pipeline diagrams; keep exploratory assets there.
- Leave build artefacts in `target/` untracked and use `src/lib.rs` for cross-crate exports and pipeline glue (`canonicalize_ingest_record`).

## Build, Test, and Development Commands
- `cargo build --workspace` verifies every crate compiles with the Rust 2024 edition.
- `cargo test --workspace --all-features` runs inline unit tests; add `-- --nocapture` when debugging payload normalization.
- `cargo fmt --all` and `cargo clippy --workspace --all-targets -D warnings` enforce formatting and lint gates before commits.
- `cargo run --package ufp_canonical --example demo` inspects canonical output; use `cargo run --package ufp_ingest --example ingest_demo` for ingest scenarios.

## Coding Style & Naming Conventions
- Rely on `rustfmt` defaults (four spaces, trailing commas) and keep modules focused in single-purpose files.
- Use `snake_case` for functions/modules, `PascalCase` for types/enums, and `SCREAMING_SNAKE_CASE` for constants.
- Derive `Debug`, `Serialize`, and `Deserialize` on public data structures to stay interoperable.
- Prefer explicit error enums with `thiserror`; surface recoverable issues through `Result` instead of `unwrap`.

## Testing Guidelines
- Co-locate unit tests in `#[cfg(test)]` modules beside implementations, naming cases as `test_<behavior>`.
- Target deterministic assertions: compare canonical text, token vectors, and checksums for canonicalization; assert UUID handling in ingest.
- Favor table-driven tests for punctuation and whitespace edge cases to reduce duplication.
- Run `cargo test --workspace` before opening a pull request and document any skipped coverage in the PR body.

## Commit & Pull Request Guidelines
- Follow the existing conventional commit pattern (`feat(core): ...`, `docs: ...`) using present-tense, imperative summaries.
- Keep each commit focused, with formatted code and passing tests recorded in the message trailer when helpful.
- PR descriptions should outline intent, list verification commands, and link issues or design notes.
- Include screenshots or CLI transcripts only when they clarify user-visible behavior changes; otherwise, describe the impact succinctly.
