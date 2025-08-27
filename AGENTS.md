# Repository Guidelines

This repository provides a Rust library and Python bindings for detecting
repeated substrings in message threads. The guide below explains where
code lives and how to contribute with minimal friction.

## Project Structure & Module Organization
- `src/` — Rust library modules (core algorithms in `capped.rs`,
  `hashed.rs`, `greedy.rs`).
- `benches/` — Criterion benchmarks.
- `tests/` — integration/unit tests.
- `python_bindings.rs` — PyO3 exposure for Python users.

## Build, Test, and Development Commands
- `cargo build` — build library.
- `cargo test` — run unit and integration tests.
- `cargo fmt` — format Rust code.
- `maturin develop` — build/install Python extension locally.

## Coding Style & Naming Conventions
- Rust 2024 idioms; `snake_case` for functions, `CamelCase` for types.
- Use `cargo fmt` and `cargo clippy` before committing.
- Prefer clear, small functions; avoid adding dev-only instrumentation.

## Testing Guidelines
- Tests use Rust's built-in test framework; place tests under `tests/`.
- Do not assert on internal counters or profiling metrics; assert on
  rendered output and correctness.
- Naming: `test_<behavior>`, keep cases small and deterministic.

## Commit & Pull Request Guidelines
- Commit messages: short imperative subject, optional body. Example:
  `feat: add capped index dedupe`.
- PRs should include a description, linked issue (if any), and test
  coverage for behavior changes. Keep changes scoped and documented.

## Rules
These are mistakes you made in the past that you need to specifically watch out for:

- Never leave breadcrumbs like "// implementation removed" when you remove a feature
- Don't prefix variables with _. Remove them instead. (Except mutex guards)
- Don't use unix tools like `applypatch` or `git patch` to change files
- Don't leave stubs or facades behind when refactoring. Remove the original instead.
