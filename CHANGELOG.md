# Changelog

All notable changes to this project will be documented in this file.
This project follows Keep a Changelog and Semantic Versioning.

## [Unreleased]
- Breaking: rename the two engines from `exact`/`approximate` to
  `greedy`/`capped`. Both are greedy exact-match heuristics (neither is
  optimal), so the old names implied a guarantee that did not exist.
  Rust: `exact`/`approximate` -> `greedy`/`capped`; `exact_tokens` /
  `approximate_tokens` -> `greedy_tokens` / `capped_tokens`; types
  `Exact`/`Approximate` -> `Greedy`/`Capped`, `ExactTokens` /
  `ApproximateTokens` -> `GreedyTokens` / `CappedTokens`.
- Breaking: the Python `exact_mode: bool` parameter is replaced by
  `engine: str` accepting `"greedy"` (default) or `"capped"`.
- Breaking: version bumped to 0.3.0.
- Fix: verify rolling-hash matches against actual token content in both
  engines; crafted 64-bit hash collisions previously corrupted output.
- Fix: implement `Config::lookback`; it was accepted but ignored, so
  references could point at arbitrarily old messages.
- Fix: capped-engine prefilter compared full capped windows, rejecting
  genuine matches that end at the current message's boundary (degrading
  them to per-token literals); windows are now compared over their common
  length.
- Fix: greedy engine no longer spends its 64-candidate extension budget on
  candidates that cannot beat the current best match, so long matches
  hidden behind many shorter candidates are found.
- Fix: capped engine now keeps the most recent occurrence of
  a deduplicated k-mer instead of the oldest, so long repeated runs match
  against the longest recent source and produce longer references with far
  fewer per-token literal fragments (and faster `segments`/render).
- Fix: capped engine no longer spends its candidate-extension budget on
  candidates that cannot beat the current best match.
- Fix: greedy engine now keeps the most recent occurrence of a deduplicated
  k-mer (repointed in place) instead of examining every occurrence, so long
  repeated runs in growing threads match their longest recent source in one
  step. On a 500-message quoted thread greedy's output shrinks ~10x
  (95% -> 99.5% of the original size) and construction is ~4x faster; the
  tradeoff is a smaller candidate pool on workloads with many competing
  short fragments.
- Fix: Python `CopyForwardText.segments()` now reports character offsets
  (matching Python string indexing) instead of byte offsets, which
  mis-indexed non-ASCII messages.
- Fix: Python `CopyForwardText.render()` no longer turns genuine
  empty-string messages into `None`; only entries that were `None` at
  construction render as `None`.
- Add `segments_chars()` to the `CopyForward` trait for character-offset
  (Unicode scalar) segment representations, alongside the byte-offset
  `segments()`.
- Add regression and round-trip test coverage (Rust and Python) for the
  above, plus unicode, empty-input, min-match-length, and randomized
  token-thread cases.
- Add a structured Criterion benchmark suite (`construct_text`,
  `construct_tokens`, and `ops_threaded` groups) covering workload shapes
  for long matches, large candidate buckets, intra-message collisions, and
  the literal-scan path, with per-case throughput and dynamic sample
  tuning.

## [0.2.1] - 2025-08-28
- Add crates.io publish workflow triggered by `v*` tags.
- Add MIT LICENSE file and update copyright.
- Add required Cargo metadata (description, readme).
- Bump version to 0.2.1.
- Add README badges and initial changelog.

