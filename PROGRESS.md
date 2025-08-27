Here’s a concrete migration plan to unify everything on &[u32] while keeping things clean and fast. We’ll centralize all
algorithms on u32 sequences, with thin wrappers for text and pre-tokenized inputs. Python bindings remain stable in behavior; Rust
API can change.

Goals

- Unify algorithm core to operate on &[u32] for both text and tokens.
- Remove duplicate token/text code in algorithms and hashing.
- Keep UTF-8 correctness and fast path for both modes.
- Preserve Python-facing behavior; Rust API can evolve.

Current State

- Duplication across text and token implementations in:
    - src/capped.rs: CappedHashedGreedy + CappedHashedGreedyTokens
    - src/hashed_binary.rs: HashedGreedyBinary + HashedGreedyBinaryTokens
- Hashing has separate prefix_hashes (u8) and prefix_hashes_u32.
- Python bindings branch per mode and mirror the duplication.

Proposed Architecture

- Core operates over sequences of u32: one generic engine shared by both “text” and “token” modes.
- Text normalization layer converts &str → Vec<u32> as Unicode scalar values (chars); text adapters render Strings from u32s.
- Single rolling-hash module for u32 only.
- Python bindings wrap the unified engine; text path uses char-normalization; token path uses passed-in ints. Optional tokenizer
path in Python continues to work, but now targets the unified engine directly.

Modules

- src/normalize.rs
    - Text normalization: string_to_u32s(&str) -> Vec<u32>, u32s_to_string(&[u32]) -> String.
- src/hashing.rs
    - Rolling hash utilities (u32 path used by engines).
- src/engine/
    - core.rs: Generic routines over &[Vec<u32>] to produce token-style segments (indices/lengths in u32 units).
    - binary.rs: Binary-search extension (current HashedGreedyBinary logic).
    - capped.rs: Capped extension + winner-local full extend (current CappedHashedGreedy logic).
    - Both engines return Vec<Vec<TokenSegment>> and retain coalescing logic.
- src/adapters.rs
    - Text adapter: wraps engine::* for text; holds original strings and offsets to render Segment with byte offsets or to render
strings directly.
    - Token adapter: thin wrapper mapping TokenSegment plus access to original token buffers for rendering.

Public API Changes (Rust)

- Internals consolidated; we can either:
    - Option A (minimal churn): Preserve current pub type Exact = ... and Approximate = ..., but implement them using the unified
engine via adapters. Keep CopyForward and CopyForwardTokens.
    - Option B (cleanup): Collapse to a single CopyForwardU32-style trait and expose text/token constructors that return the same
concrete type. Tests will need updates. Given tests exist, prefer Option A first, then follow-up cleanup.

Given “only Python bindings need to work,” we can do Option B eventually, but to keep confidence high while migrating, take Option
A first, then collapse.

Python Bindings

- Replace branching PyAlgorithm variants with a single variant that holds:
    - Mode enum: Text { originals: Vec<String> } or Tokens.
    - Segments: Vec<Vec<TokenSegment>> from the unified engine.
    - Optional tokenizer for from_texts(..., tokenizer=...).
- segments():
    - In text mode: keep current shape for dicts.
    - In token mode: identical to now (list or numpy).
- render():
    - Text mode: reconstruct from token segments by decoding u32s back to String; use byte offsets computed from char indices when needed by Segment API.
    - Token mode: unchanged.
- render_texts():
    - Text mode: same as render() with no replacement.
    - Token mode with tokenizer: decode tokens to strings via tokenizer as today.

This hides internal unification from users and keeps Python behavior unchanged.

Step-by-Step Plan

1. Design unified &[u32] core

- Choose “u32 = Unicode scalar for text, raw id for tokens”.
- Confirm base hash and collision considerations stay unchanged.

2. Prototype text→u32 normalization

- Add normalize.rs with string_to_u32s and u32s_to_string.

3. Refactor hashing.rs to u32-only

- Remove prefix_hashes(&[u8], ...).
- Rename hashing.rs → hash.rs or keep file name but delete u8 variant.
- Update imports in hashed_binary.rs, capped.rs.

4. Extract core engine logic

- Move shared logic (k-mer table build, prefix hashes, candidate extension, coalescing) into engine/binary.rs and engine/capped.rs
over &[Vec<u32>].
- Return Vec<Vec<TokenSegment>>.

5. Build adapters for text/tokens

- Text: normalize messages to u32s; call engine; map TokenSegment to text by computing byte offsets from char indices when returning segments and when rendering.
- Tokens: thin wrapper around engine results; keep CopyForwardTokens.

6. Wire up public types

- Keep Exact, ExactTokens, Approximate, ApproximateTokens type aliases but point to new adapters.
- Ensure lib.rs re-exports unchanged for now.

7. Update Python bindings to call unified core

- Internally treat both text and token paths as driving the same engine through adapters.
- Ensure segments() and render() shapes remain identical to tests.

8. Remove duplicates and update docs
- Delete token-specific and text-specific duplicate code from capped.rs and hashed_binary.rs.
- Remove dead functions and comments. Don’t leave stubs.
- Update README and rustdoc examples to reflect unified internals.

9. Validate

- Run cargo test, cargo fmt, cargo clippy.
- Run Python tests via maturin develop and pytest, if available locally.

Key Design Details

 - Reference units:
     - Engine operates in u32 units.
     - Text adapter converts u32 spans back to Strings; byte spans for Segment API are computed from char indices as needed.
- Hashing:
    - Keep base = 257; pure wrapping u64 ops as today.
    - Single prefix_hashes_u32 covers both modes.
- Coalescing:
    - Keep existing coalescing pass on TokenSegment::Reference with consecutive spans.

Migration Phases and Safety

- Phase 1 (internal): Add normalization and unify hashing; build core engines without removing old code. Add unit tests for
normalization and engine output parity with old implementations on a few fixtures.
- Phase 2 (switch adapters): Replace old algorithms with adapters using the new core; keep public types to avoid touching tests
initially.
- Phase 3 (cleanup): Remove old duplicated paths; simplify Python bindings variant structure.
- Phase 4 (optional): Collapse Rust traits/types toward a single u32-centric API if desired.

Risks and Mitigations

 - UTF-8 boundary correctness:
     - Compute byte offsets from char_indices when needed for Segment byte ranges; add targeted tests.
- Behavior drift:
    - Compare segments and rendered outputs from old vs new on representative fixtures before deletion.
- Python binding correctness:
    - Keep output shapes; reuse tokenizer for render_texts in token mode.
