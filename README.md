# copyforward

[![Crates.io](https://img.shields.io/crates/v/copyforward)](https://crates.io/crates/copyforward)
[![PyPI](https://img.shields.io/pypi/v/copyforward)](https://pypi.org/project/copyforward/)
[![CI](https://github.com/SeanTater/copyforward/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/SeanTater/copyforward/actions/workflows/ci.yml)
[![Python CI](https://github.com/SeanTater/copyforward/actions/workflows/python-ci.yml/badge.svg?branch=main)](https://github.com/SeanTater/copyforward/actions/workflows/python-ci.yml)
[![Crates Publish](https://github.com/SeanTater/copyforward/actions/workflows/crates-publish.yml/badge.svg?branch=main)](https://github.com/SeanTater/copyforward/actions/workflows/crates-publish.yml)

Fast copy-forward compression for message threads. Detects repeated substrings across messages and replaces them with references to earlier occurrences, typically reducing storage by 75-99% on repetitive thread data (non-repetitive text stays incompressible).

Perfect for chat logs, document histories, dataframes with missing values, and any sequence of texts with repeated content.

## Quick Start

### Python

```python
import copyforward

# Basic usage with text messages
messages = ["Hello world", "Hello world, how are you?", "Hello world today"]

# Text API (exact by default)
cf = copyforward.CopyForwardText.from_texts(messages)
print(f"Compression ratio: {cf.compression_ratio():.2f}")
# Render with replacement text to visualize references
visualized = cf.render("[REF]")  # ['Hello world', '[REF], how are you?', '[REF] today']

# Handle missing values (perfect for dataframes!)
messages_with_none = ["Hello world", None, "Hello world again"]
cf_none = copyforward.CopyForwardText.from_texts(messages_with_none)
result = cf_none.render("[REF]")  # ['Hello world', None, '[REF] again']

# Approximate (faster) text compression
cf_fast = copyforward.CopyForwardText.from_texts(messages, exact_mode=False)
approx_result = cf_fast.render("[REF]")  # May find different compression patterns

# Token API with missing values
toks = [[10, 11, 12], None, [10, 11, 12, 13]]
cf_tok = copyforward.CopyForwardTokens.from_tokens(toks, exact_mode=True)
# Render with replacement tokens - only non-None entries returned
rendered_toks = cf_tok.render([999])  # [[10, 11, 12], [999, 13]]

# Tokenizer opt-in: build token-mode directly from texts and keep tokenizer for decoding
repeated_messages = ["Hello world from Alice", "Hello world from Alice again", "Alice says hi"]
cf_tok2 = copyforward.CopyForwardTokens.from_texts_with_tokenizer(repeated_messages, tokenizer="whitespace", exact_mode=True)
token_ids = cf_tok2.render([9999])         # List[List[int]] with replacement tokens
decoded = cf_tok2.render_texts("[REF]")    # Decoded text with replacements
```

### Rust

```rust
use copyforward::{exact, approximate, Config};

// Basic usage
let messages = &["Hello world", "Hello world, how are you?"];
let compressed = exact(messages, Config::default());

// Handle missing values (Option types work seamlessly!)
let messages_with_none = &[Some("Hello world"), None, Some("Hello world again")];
let compressed = exact(messages_with_none, Config::default());

// Approximate compression - fastest on nested/growing threads
let compressed = approximate(messages, Config::default());

// Render back to original
let original = compressed.render_with(|_, _, _, text| text.to_string());
```

## Algorithm Selection

Two algorithms are available; which one is faster and smaller depends on the data:

| Algorithm | Best for | Why |
|-----------|----------|-----|
| **Exact** | Data with many competing repeated fragments (phrase pools, mixed content) | Examines up to 64 candidate matches per position and keeps the longest |
| **Approximate** | Nested/growing message threads; incompressible data | Capped extension plus most-recent matching; smallest output and fastest on threads |

Measured on the benchmark workloads (see the Performance section): on
nested threads both engines save 95-99.75%, with approximate producing the
smaller output faster; on phrase-pool data exact saves ~90% versus ~76% for
approximate and is also faster; on non-repetitive text neither saves
anything.

## Missing Value Support

Both Python and Rust APIs seamlessly handle missing/None values, making them perfect for dataframe compression:

### Python

```python
import copyforward

# DataFrame-like data with missing values
messages = [
    "User logged in",
    None,  # Missing log entry
    "User logged in successfully",
    None,
    "User logged out"
]

cf = copyforward.CopyForwardText.from_texts(messages)
compressed = cf.render("[REF]")
# Result: ['User logged in', None, '[REF] successfully', None, 'User logged out']

# Token data with missing values
tokens = [[1, 2, 3], None, [1, 2, 3, 4]]
cf_tok = copyforward.CopyForwardTokens.from_tokens(tokens)
```

### Rust

```rust
use copyforward::{exact, exact_tokens, Config};

// Mixed Option types work seamlessly
let messages = &[
    Some("User logged in"),
    None,
    Some("User logged in successfully")
];
let compressed = exact(messages, Config::default());

// Token sequences with None values
let tokens = &[
    Some(vec![1u32, 2u32, 3u32]),
    None,
    Some(vec![1u32, 2u32, 3u32, 4u32])
];
let compressed = exact_tokens(tokens, Config::default());
```

## Installation

### Python

```bash
pip install maturin
# Build the wheel with Python bindings enabled
maturin develop --features python

# If you want named tokenizers (e.g., whitespace / HF), enable the bundle:
# maturin develop --features python-tokenizers
```

### Rust

```toml
[dependencies]
copyforward = "0.2"
```

## Advanced Usage

### Python

```python
import copyforward

# Custom configuration (text)
cf = copyforward.CopyForwardText.from_texts(
    messages,
    exact_mode=True,      # Perfect compression
    min_match_len=8,      # Only create refs for 8+ char matches
    lookback=100          # Only search previous 100 messages
)

# Get detailed segment information
segments = cf.segments()
for msg_segments in segments:
    for segment in msg_segments:
        if segment['type'] == 'reference':
            print(f"Reference to message {segment['message']}")
        else:
            print(f"Literal text: {segment['text']}")

# Render with custom replacement (useful for debugging and visualization)
redacted = cf.render("[REFERENCE]")  # Shows where references occur

# Tokenization (opt-in)
cf_tok = copyforward.CopyForwardTokens.from_texts_with_tokenizer(
    messages,
    tokenizer="whitespace",   # or feature-gated 'hf:<model>' / 'file:<path>'
    exact_mode=True,
)
token_ids = cf_tok.render([9999])  # Replace references with token 9999
texts = cf_tok.render_texts("[REF]")  # Decoded text with "[REF]" replacements
```

### Viewing generated Python docs

After building the Python extension with `maturin`, the PyO3 docstrings are available via Python's help system:

```bash
# Build and install the extension into your active venv
maturin develop --features python

python -c "import copyforward; help(copyforward.CopyForwardText)"
python -c "import copyforward; help(copyforward.CopyForwardTokens)"
```

This prints the docstring and usage information emitted by the PyO3 bindings.

### Rust

```rust
use copyforward::{exact, Config, CopyForward};

// Custom configuration
let config = Config {
    min_match_len: 8,
    lookback: Some(100),
    ..Config::default()
};

let compressed = exact(&messages, config);

// Get compression details
let segments = compressed.segments();
for (i, msg_segments) in segments.iter().enumerate() {
    println!("Message {}: {} segments", i, msg_segments.len());
}

// Custom rendering
let redacted = compressed.render_with_static("[REF]");
```

## How It Works

Copy-forward compression works in two phases:

1. **Analysis**: Scan messages to find repeated substrings using rolling hash indexing
2. **Compression**: Replace repeated text with references to first occurrence

Example:
```
Input:  ["Hello world", "Hello world today"]
Output: [Literal("Hello world"), [Reference(0,0,11), Literal(" today")]]
```

This represents the second message as a reference to the entire first message plus the literal text " today".

**Missing Value Handling**: None/null values are preserved in their original positions but skipped during compression analysis, ensuring perfect round-trip fidelity for dataframe-like data.

## Performance

Compression savings by workload shape (measured; 1.0 = no savings):

| Workload | Exact | Approximate |
|----------|-------|-------------|
| Nested thread (each message quotes the previous) | 95-98% savings | 99-99.75% savings |
| Repeated messages | 99.3-99.8% savings | 99.3-99.8% savings |
| Phrase pool (many competing fragments) | ~90% savings | ~75% savings |
| Non-repetitive text | none | none |

Construction speed on ~0.5-3 MB of text depends on the shape: approximate
is ~2x faster on nested and repeated workloads, but ~1.3x slower than exact
on phrase-pool data, where exact's longer matches pay for the extra
candidate work. Run `cargo bench` for the full per-workload suite.

Missing values add minimal overhead - compression speed remains constant regardless of None density.

## Repository Structure

- `src/` — Rust library implementation
- `tests/` — Integration tests
- `benches/` — Performance benchmarks

## Changelog

See CHANGELOG.md for detailed release notes.

## License

MIT License - see LICENSE file for details.

## Features and Builds

- Default build has no Python or tokenizer dependencies, keeping Rust users lean.
- Cargo features:
  - `python`: enables PyO3 and numpy for Python bindings.
  - `tokenizers`: enables integration with the `tokenizers` crate for named tokenizers.
  - `hf-hub`: adds optional support wiring for Hub-based tokenizers; current crate version does not implement hub loading at runtime.
  - Bundles: `python-tokenizers`, `python-tokenizers-hub` for convenience.

### Python wheels

- Build Python extension (bindings only):
  - `maturin develop --features python`
- Build Python extension with tokenizer support:
  - `maturin develop --features python-tokenizers`
- Hub bundle (compiles but hub loading not implemented in tokenizers v0.15):
  - `maturin develop --features python-tokenizers-hub`

Notes
- Using `tokenizer="whitespace"` requires only the `python` feature.
- Using `tokenizer="hf:<model>"` or `tokenizer="file:<path>"` requires `tokenizers`; hub loading by name is not implemented for the current tokenizers version. Load from a local tokenizer JSON using `file:<path>`.
