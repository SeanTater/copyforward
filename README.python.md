# Python bindings (pyo3)

This crate exposes a minimal Python extension `copyforward_py` built with pyo3.

Build (recommended):

- Install `maturin` (`pip install maturin`) and run `maturin develop` in the repo root.
- Alternatively build a wheel with `maturin build`.

Example usage:

```py
from copyforward_py import PyGreedySubstring
msgs = ["hello world", "hello world today"]
cf = PyGreedySubstring(msgs, min_match_len=10)
print(cf.segments())
print(cf.render_with_static("..."))
```


