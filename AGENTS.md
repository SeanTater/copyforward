# Repository Guidelines

This repository provides a Rust library and Python bindings for detecting
repeated substrings in message threads. The guide below explains where
code lives and how to contribute with minimal friction.

## Project Structure & Module Organization
- `src/` — Rust library. Public API in `lib.rs`/`core.rs`; compression
  engines in `engine/binary.rs` (exact) and `engine/capped.rs`
  (approximate), wrapped by `hashed_binary.rs` and `capped.rs`.
  Helpers in `hashing.rs` (rolling hashes), `normalize.rs` (char<->u32),
  `tokenization.rs` (Python tokenizer opt-in), `fixture.rs` (test data).
- `src/python_bindings.rs` — PyO3 exposure for Python users.
- `benches/` — Criterion benchmarks.
- `tests/` — Rust integration tests; Python tests in `tests/python_tests/`.

## Build, Test, and Development Commands
- `cargo build` — build library.
- `cargo test` — run unit and integration tests.
- `cargo bench` — run Criterion benchmarks (construct + ops groups).
- `cargo fmt` — format Rust code.
- `maturin develop` — build/install Python extension locally.
- `.venv/bin/python -m pytest tests/python_tests/ -q` — run Python tests
  (create the venv with `uv venv .venv && uv pip install -p .venv maturin
  pytest numpy`, then `maturin develop`).

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
