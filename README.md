# copyforward

Fast copy-forward compression for message threads. Detects repeated substrings across messages and replaces them with references to earlier occurrences, reducing storage requirements by 50-90%.

Perfect for chat logs, document histories, and any sequence of texts with repeated content.

## Quick Start

### Python

```python
import copyforward

messages = ["Hello world", "Hello world, how are you?", "Hello world today"]

# Text API (exact by default)
cf = copyforward.CopyForwardText.from_texts(messages)
print(f"Compression ratio: {cf.compression_ratio():.2f}")
original = cf.render()  # ['Hello world', 'Hello world, how are you?', 'Hello world today']

# Approximate (faster) text compression
cf_fast = copyforward.CopyForwardText.from_texts(messages, exact_mode=False)
assert cf_fast.render() == messages

# Token API
toks = [[10, 11, 12], [10, 11, 12, 13]]
cf_tok = copyforward.CopyForwardTokens.from_tokens(toks, exact_mode=True)
assert cf_tok.render() == toks

# Tokenizer opt-in: build token-mode directly from texts and keep tokenizer for decoding
cf_tok2 = copyforward.CopyForwardTokens.from_texts_with_tokenizer(messages, tokenizer="whitespace", exact_mode=True)
token_ids = cf_tok2.render()         # List[List[int]]
roundtrip = cf_tok2.render_texts()   # Decoded via stored tokenizer
assert roundtrip == messages
```

### Rust

```rust
use copyforward::{exact, approximate, Config};

let messages = &["Hello world", "Hello world, how are you?"];

// Exact compression - finds optimal matches
let compressed = exact(messages, Config::default());

// Fast approximate compression - 2x speed for large texts
let compressed = approximate(messages, Config::default()); 

// Render back to original
let original = compressed.render_with(|_, _, _, text| text.to_string());
```

## Algorithm Selection

Choose between two optimized algorithms:

| Algorithm | Best for | Speed | Accuracy |
|-----------|----------|-------|----------|
| **Exact** | < 1MB total text, perfect compression needed | Slower | Perfect |
| **Approximate** | > 1MB text, speed matters | ~2x faster | Excellent |

The approximate algorithm may split some long references but still achieves excellent compression ratios.

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
copyforward = "0.1"
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

# Render with custom replacement (useful for debugging)
redacted = cf.render("[REFERENCE]")

# Tokenization (opt-in)
cf_tok = copyforward.CopyForwardTokens.from_texts_with_tokenizer(
    messages,
    tokenizer="whitespace",   # or feature-gated 'hf:<model>' / 'file:<path>'
    exact_mode=True,
)
token_ids = cf_tok.render()
texts = cf_tok.render_texts()   # Decoded via stored tokenizer
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

## Performance

Typical compression ratios:
- **Chat logs**: 60-80% space savings
- **Code diffs**: 70-90% space savings  
- **Document versions**: 50-80% space savings

Speed comparison on 1MB of message data:
- **Exact**: ~50ms, perfect compression
- **Approximate**: ~25ms, 95% of perfect compression

## Repository Structure

- `src/` — Rust library implementation
- `tests/` — Integration tests  
- `benches/` — Performance benchmarks

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
