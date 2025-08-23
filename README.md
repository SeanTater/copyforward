# copyforward

Rust library to detect repeated quoted substrings across messages and represent
them as references to previous content (copy-forward). This repo contains:

- `src/lib.rs`: Rust library with a `CopyForward` trait and `LongestMatch`
  implementation (simple DP-based multi-match algorithm).
- `src/fixture.rs`: deterministic message-thread generator used in tests/benches.
- `benches/`: Criterion benchmarks.
- `src/python_bindings.rs`: basic pyo3 bindings exposing `PyLongestMatch`.

Usage (Rust)

Add this crate as a dependency (local development) and use `LongestMatch::from_messages`.

Usage (Python)

Build the Python extension with maturin or `maturin develop` (recommended). Then:

```python
from copyforward_py import PyLongestMatch
msgs = ["hello", "hello world"]
cf = PyLongestMatch(msgs)
print(cf.segments())
print(cf.render_with_static("..."))
```


