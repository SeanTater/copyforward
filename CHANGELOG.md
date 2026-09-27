# Changelog

All notable changes to this project will be documented in this file.
This project follows Keep a Changelog and Semantic Versioning.

## [Unreleased]
- Fix: verify rolling-hash matches against actual token content in both
  engines; crafted 64-bit hash collisions previously corrupted output.
- Fix: implement `Config::lookback`; it was accepted but ignored, so
  references could point at arbitrarily old messages.
- Fix: capped-engine prefilter compared full capped windows, rejecting
  genuine matches that end at the current message's boundary (degrading
  them to per-token literals); windows are now compared over their common
  length.
- Fix: exact engine no longer spends its 64-candidate extension budget on
  candidates that cannot beat the current best match, so long matches
  hidden behind many shorter candidates are found.
- Fix: Python `CopyForwardText.segments()` now reports character offsets
  (matching Python string indexing) instead of byte offsets, which
  mis-indexed non-ASCII messages.
- Fix: Python `CopyForwardText.render()` no longer turns genuine
  empty-string messages into `None`; only entries that were `None` at
  construction render as `None`.
- Add `Exact::segments_chars()` / `Approximate::segments_chars()` for
  character-offset segment representations.
- Add regression and round-trip test coverage (Rust and Python) for the
  above, plus unicode, empty-input, min-match-length, and randomized
  token-thread cases.

## [0.2.1] - 2025-08-28
- Add crates.io publish workflow triggered by `v*` tags.
- Add MIT LICENSE file and update copyright.
- Add required Cargo metadata (description, readme).
- Bump version to 0.2.1.
- Add README badges and initial changelog.

