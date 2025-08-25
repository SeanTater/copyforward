# copyforward

Rust library to detect repeated quoted substrings across messages and represent
them as references to previous content (copy-forward). This repo contains:

- `src/lib.rs`: Rust library with a `CopyForward` trait and `GreedySubstring`
  implementation (greedy substring matching algorithm).
- `src/fixture.rs`: deterministic message-thread generator used in tests/benches.
- `benches/`: Criterion benchmarks.
- `src/python_bindings.rs`: basic pyo3 bindings exposing `PyGreedySubstring`.

Usage (Rust)

Add this crate as a dependency (local development) and use `GreedySubstring::from_messages`.

Example (Rust) — configure minimum match length:

```rust
use copyforward::{GreedySubstring, GreedySubstringConfig};

let msgs = &["hello", "hello world"];
let cfg = GreedySubstringConfig { min_match_len: 4, lookback: None };
let cf = GreedySubstring::with_config(&cfg, msgs);
let segments = cf.segments();
```

Usage (Python)

Build the Python extension with maturin or `maturin develop` (recommended). Then:

```python
from copyforward import PyLongestMatch
msgs = ["hello", "hello world"]
# keyword args supported for min_match_len and lookback
cf = PyLongestMatch(msgs, min_match_len=4, lookback=None)
print(cf.segments())
print(cf.render_with_static("..."))
```


