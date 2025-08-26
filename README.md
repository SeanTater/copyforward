# copyforward

Python-first usage
-------------------

This project provides a Rust implementation of a "copy-forward" algorithm
and exposes a Python binding for easy use from Python code. It detects
repeated quoted substrings across messages and represents them as references
to earlier messages (reducing duplication in message threads).

Quickstart (Python)

1. Build and install the Python extension (requires `maturin`):

   ```bash
   pip install maturin
   maturin develop
   ```

2. Use the Python API:

   ```py
   from copyforward import PyGreedySubstring
   msgs = ["Hello world", "Hello world again"]
   cf = PyGreedySubstring(msgs, min_match_len=4)
   print(cf.segments())
   print(cf.render_with_static("[REF]"))
   ```

Library (Rust)
----------------

If you prefer to use the Rust API directly, the crate exposes a `CopyForward`
trait and several implementations in `src/` (including the `CappedHashedGreedy`
approximate implementation). Typical usage:

```rust
use copyforward::{GreedySubstring, GreedySubstringConfig};
let msgs = &["Hello", "Hello world"];
let cfg = GreedySubstringConfig { min_match_len: 4, lookback: None };
let cf = GreedySubstring::with_config(&cfg, msgs);
let out = cf.render_with(|_m, _s, _l, text| text.to_string());
```

Repository layout
------------------

- `src/` — main Rust library and implementations
- `benches/` — Criterion benchmarks
- `tests/` — integration/unit tests

Development notes
------------------

- `cargo test` runs the Rust tests.
- `cargo fmt` formats the code.
- `maturin develop` builds and installs the Python extension.

Contributing
-------------

This repo aims to be minimal and focused. If you'd like to contribute,
open an issue or a pull request with a clear description and tests. We
prioritize correctness and reproducible benchmarks.

