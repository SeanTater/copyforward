# copyforward - Codebase Guide for Claude Code

## Project Overview

**copyforward** is a Rust library for detecting repeated quoted substrings across messages and representing them as references to previous content. It provides both Rust and Python APIs using a dynamic programming algorithm to find optimal non-overlapping matches.

**Core Purpose**: Compress message threads by replacing repeated text with references to earlier messages, achieving significant size reduction (target: 50%+ compression).

## Architecture

### Language Stack
- **Primary**: Rust (2024 edition)
- **Bindings**: Python via PyO3
- **Build System**: Cargo + Maturin (for Python extensions)

### Core Components

#### 1. Main Library (`src/lib.rs`) - 317 lines
- **`CopyForward` trait**: Generic interface for copy-forward algorithms
- **`LongestMatch` struct**: Main implementation using weighted-interval dynamic programming
- **`LongestMatchConfig`**: Configuration with `min_match_len` and `lookback` window
- **`Segment` enum**: Represents text as `Literal(String)` or `Reference{message_idx, start, len}`

**Key Algorithm**: Uses dynamic programming to find optimal non-overlapping substring matches across previous messages, similar to weighted interval scheduling.

#### 2. Python Bindings (`src/python_bindings.rs`) - 46 lines
- **`PyLongestMatch`**: Python wrapper exposing the Rust implementation
- Supports keyword arguments: `min_match_len`, `lookback`
- Methods: `segments()` (returns formatted strings), `render_with_static()`

#### 3. Test Fixtures (`src/fixture.rs`) - 43 lines
- **`generate_thread()`**: Deterministic message thread generator for testing
- Creates realistic conversation patterns with quotes, inline replies, and text additions
- Used extensively in tests and benchmarks

#### 4. Benchmarks (`benches/bench_copyforward.rs`) - 88 lines
- Criterion-based performance testing
- Tests various message counts (10-200) and base sentence sizes (1-100)
- Validates compression effectiveness (95%+ reduction requirement)

### Dependencies
```toml
[dependencies]
pyo3 = "0.20"           # Python bindings
rand = "0.8"            # Test fixture generation
rand_chacha = "0.3"     # Deterministic RNG

[dev-dependencies]
criterion = "0.5"       # Benchmarking
```

## Development Workflow

### Essential Commands
```bash
# Core Rust development
cargo test              # Run all tests
cargo clippy --all-targets -- -D warnings  # Linting
cargo fmt              # Code formatting
cargo bench            # Performance benchmarks

# Python extension development
pip install maturin    # Install Python build tool
maturin develop        # Build and install Python extension locally

# Pre-commit hooks (recommended)
pip install pre-commit
pre-commit install
pre-commit run --all-files
```

### Build Configuration
- **Crate types**: `["cdylib", "rlib"]` - supports both Python extension and Rust library
- **CI/CD**: GitHub Actions with pre-commit hooks, clippy, and tests
- **Pre-commit**: Cargo clippy + standard hooks (trailing whitespace, YAML validation)

## Key Algorithms & Data Structures

### LongestMatch Algorithm
1. **Match Detection**: Find all occurrences of previous messages in current message
2. **Weighted Interval DP**: Select optimal non-overlapping matches to maximize saved bytes
3. **Segment Construction**: Build output as alternating `Literal` and `Reference` segments

### Configuration Options
- **`min_match_len`**: Minimum substring length to consider (default: 1)
- **`lookback`**: Limit search to N previous messages (default: None = all previous)

## Testing Strategy

### Test Coverage
- **Unit tests**: Basic functionality, edge cases, exact duplicate detection
- **Integration tests**: Realistic thread compression with fixture generator  
- **Performance tests**: Compression ratio validation (must achieve 50%+ reduction)
- **Benchmark tests**: Performance across various message thread sizes

### Key Test Patterns
```rust
// Basic usage
let msgs = &["hello", "hello world"];
let cf = LongestMatch::from_messages(msgs);
let segments = cf.segments();

// With configuration
let cfg = LongestMatchConfig { min_match_len: 4, lookback: Some(10) };
let cf = LongestMatch::with_config(&cfg, msgs);
```

## Python API Usage
```python
from copyforward import PyLongestMatch

msgs = ["hello", "hello world"]
cf = PyLongestMatch(msgs, min_match_len=4, lookback=None)
print(cf.segments())              # Get segment representation
print(cf.render_with_static("...")) # Render with replacements
```

## Architecture Notes

### Design Principles
- **Trait-based**: `CopyForward` trait allows multiple algorithm implementations
- **Zero-copy where possible**: Uses string slices and references efficiently
- **Configurable**: Tunable parameters for different use cases
- **Deterministic**: Reproducible results with seeded test fixtures

### Performance Characteristics
- **Time Complexity**: O(n²m) where n=messages, m=average message length
- **Space Complexity**: O(n + total_segments)
- **Compression Target**: 50%+ reduction on typical conversation threads

### Future Extension Points
- Aho-Corasick algorithm for faster substring matching
- Incremental builders for streaming updates
- Alternative matching strategies (suffix arrays, etc.)

## File Structure Summary
```
src/
├── lib.rs              # Main library and LongestMatch implementation
├── python_bindings.rs  # PyO3 Python interface
├── fixture.rs          # Test data generation
└── main.rs            # Binary entry point (if needed)

benches/
└── bench_copyforward.rs # Performance benchmarks

.github/workflows/
└── ci.yml             # CI/CD pipeline

Configuration:
├── Cargo.toml         # Rust package configuration
├── .pre-commit-config.yaml # Code quality hooks
└── CONTRIBUTING.md    # Development setup guide
```

This codebase is well-structured for both Rust and Python development, with strong testing practices and clear separation of concerns. The algorithm is sophisticated but the codebase remains compact and focused.