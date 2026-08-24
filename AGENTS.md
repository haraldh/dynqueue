# Repository Guidelines

## Project Structure & Module Organization

DynQueue is a small Rust 2024 library. Public APIs and queue implementations live in `src/lib.rs`; unit and concurrency regression tests live in `src/tests.rs` and are included through `#[cfg(test)]`. `README.md` contains the user-facing overview and examples, and is also compiled as documentation by the crate. Cargo metadata and features are defined in `Cargo.toml`; keep `Cargo.lock` updated when dependencies change. Nix users can enter the development environment through `flake.nix`, `default.nix`, or `shell.nix`. GitHub Actions workflows under `.github/workflows/` enforce formatting, builds, tests, and coverage.

## Build, Test, and Development Commands

- `cargo build --all-features`: compile the library and optional `crossbeam-queue` backend.
- `cargo test --all-features`: run unit, documentation, and feature-gated tests.
- `cargo fmt --all -- --check`: verify standard Rust formatting without modifying files.
- `cargo clippy --all-targets --all-features -- -D warnings`: catch lint issues; the crate also denies all Clippy lints.
- `cargo doc --all-features --no-deps`: validate public documentation locally.
- `cargo llvm-cov --all-features --workspace`: reproduce the coverage job when `cargo-llvm-cov` is installed.

Use Rust 1.87 or newer, as declared by the crate's MSRV. `nix develop` provides the repository's pinned development shell.

## Coding Style & Naming Conventions

Use `rustfmt` defaults (four-space indentation) and idiomatic Rust naming: `snake_case` for functions/modules, `CamelCase` for types and traits, and `SCREAMING_SNAKE_CASE` for constants. Document every public item because `missing_docs` is denied. Keep synchronization and termination logic comments focused on invariants and race prevention. Avoid weakening `Send`/`Sync` bounds without a concurrency-focused justification.

## Testing Guidelines

Add tests to `src/tests.rs` with descriptive `snake_case` names. Cover `Vec`, `VecDeque`, and feature-gated `SegQueue` behavior when backend semantics change. Concurrency tests must allow scheduling overhead and should assert outcomes deterministically (for example, sort collected results before comparison). Run the full all-features suite before submitting; some tests intentionally sleep and may take several seconds.

## Commit & Pull Request Guidelines

Recent history follows Conventional Commit-style subjects such as `fix:`, `test:`, `build:`, and scoped forms like `chore(nix):`. Write imperative, focused subjects and keep unrelated changes separate. Pull requests should explain the behavioral impact, call out API or MSRV changes, link relevant issues, and include the commands run. Update `README.md` and public docs for user-visible API changes; screenshots are generally unnecessary for this library.
