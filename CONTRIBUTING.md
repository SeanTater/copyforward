# Contributing

Thanks for contributing to copyforward! A few quick setup steps to run tests and linters locally.

1. Install Rust (stable) via `rustup`: `curl https://sh.rustup.rs -sSf | sh`.
2. Install `pre-commit`: `pip install pre-commit`.
3. Install the pre-commit hooks: `pre-commit install`.
4. Run the hooks locally: `pre-commit run --all-files`.

Running tests and lints

- Run the test suite: `cargo test`.
- Run clippy: `cargo clippy --all-targets -- -D warnings`.

Python bindings

- Build the Python extension with `maturin develop` (install `maturin` with `pip install maturin`).

Formatting

- Use `cargo fmt` to format code.

If you'd like to work on performance improvements (Aho-Corasick, incremental builders), open an issue and reference this repo. Thanks!


