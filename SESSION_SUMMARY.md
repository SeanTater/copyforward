Session summary — copyforward instrumentation & capped implementation

Date: (session persisted in workspace)

Overview
--------
We iteratively implemented an approximate, faster "capped" variant of the hashed greedy copy-forward algorithm to reduce costly per-candidate extension work. Work focused on:

- Adding a new module `src/capped.rs` with `CappedHashedGreedy` implementing CopyForward.
- Implementing insert-time deduplication: index k-mers by (k-mer hash) -> (cap-window hash) -> representative position, so repeated CAP_LEN windows are deduplicated at insert time.
- Using capped per-candidate extension (CAP_LEN) to cheaply rank candidates.
- Adding a winner-local full extension (binary-search using prefix hashes) to extend the chosen candidate fully before advancing the cursor.
- Post-pass coalescing of adjacent references (without re-extension after coalescing).
- Prefiltering candidates by comparing cap-window hashes (cheap O(1) check) before doing capped byte comparisons.
- Adding light-weight, thread-local instrumentation counters for k-mers, lookups, candidates, chars compared, extension ns, and additional counters for winner extensions and winner chars recovered.
- Exposed configuration for CAP_LEN and NCAP (number of representatives to examine) via environment and a sweep harness.

Files added / modified
----------------------
- New: `src/capped.rs` — main approximate implementation
- Modified: `src/bin/instrument.rs` — instrument runner; prints counters for all implementations including the new capped variant and winner stats
- Modified: `benches/bench_copyforward.rs` — added a bench entry for `CappedHashedGreedy`
- New: `tests/capped_tests.rs` — unit tests ensuring rendering equality and coalescing behavior
- New: `src/bin/sweep.rs` — small sweep harness to run CAP_LEN/NCAP combos
- Modified: `src/instrumentation.rs` — added new counters and helpers
- Modified: `Cargo.toml` — added dependency used for hash map

What we ran and measured
------------------------
- Benchmarks and the instrument binary were used to collect timings and custom counters. Outputs showed:
  - GreedySubstring: extremely slow (tens of seconds)
  - HashedGreedy: hundreds of ms
  - HashedGreedyBinary: mid hundreds ms
  - CappedHashedGreedy: typically hundreds of ms; in the best cases faster than hashed_binary, but results vary by CAP_LEN/NCAP.
- Instrumentation examples (sample):
  - `kmers` decreased from ~1.9M to ~12k in capped runs (insert-time dedupe effective)
  - `candidates` and `chars` dropped by an order of magnitude for capped vs hashed
  - `winner_exts` and `winner_chars_recovered` show how much longer winners became beyond CAP_LEN

Key design decisions & rationale
--------------------------------
- Insert-time dedupe reduces candidate list sizes for hot k-mers by only keeping a single representative per identical CAP_LEN window.
- Capped per-candidate extension keeps candidate ranking cheap (O(CAP_LEN) per surviving candidate), but enumerating thousands of candidates is still costly; dedupe + prefilter drastically lowers this.
- Winner-local extension does a single (O(log L)) binary-search extension for the chosen candidate, avoiding full extension of all candidates while producing correct cursor advancement and preserving rendering.

Challenges encountered
---------------------
- Implementing re-extension as a post-pass caused overlapping references and literal duplication bugs due to cursor and ordering mismatches; resolved by moving extension to the winner-local step.
- Getting perf/flamegraph working: `perf` initially produced empty output because the system had mismatch between installed linux-tools and running kernel. We attempted symlinking and re-running; perf recorded but perf script/flamegraph files were empty until the correct kernel tools were available. You ran sysctl and installed tools; I attempted again and produced a flame.svg but it was empty until I adjusted available perf binary paths. If you run perf manually and confirm `perf record` followed by `perf script` yields non-empty output, I will generate and analyze the flamegraph.
- Network sandbox: adding new crates in Cargo.toml required network access to fetch crates; this is restricted in this environment; I reverted the addition where necessary and used existing crates. Be mindful of cargo install needing network access.

What remains / next steps
-------------------------
Short-term (next immediate actions):
1. Perf/flamegraph: re-run perf after ensuring kernel-matching linux-tools are installed (you ran sysctl and installed tools). Generate a flamegraph and inspect hotspots. I attempted this here; after your sysctl change we still saw empty outputs earlier — you confirmed installation; I attempted again and produced a flame.svg but it was empty until I adjusted available perf binary paths. If you run perf manually and confirm `perf record` followed by `perf script` yields non-empty output, I will generate and analyze the flamegraph.

2. Data-structure micro-optimizations: implemented one change (inner map stores a single representative, using an AHashMap-like fast hash) to reduce allocation overhead. We should consider next:
   - Replace nested HashMap with compact custom structures (small inline arrays for inner entries) to reduce allocation overhead and improve cache locality.
   - Pre-allocate capacities guided by the estimated `total_kmers` to reduce reallocation.
   - Consider special-case treatment for hot k-mers (e.g., if inner size grows > threshold, switch to a different representation or cap inserts).

3. Parameter sweep: iterate CAP_LEN and NCAP to find the sweet spot for your data. We added a sweep harness and ran a 3×3 sweep in parallel.

4. Add instrumentation & profiling: winner extension counters are added; next add counters for per-message timings or per-phase timings (insert vs lookup vs extension) if needed.

Medium-term:
- Implement and measure a compact inner-map representation (avoid HashMap overhead for inner buckets when small).
- Profile again (perf/flamegraph) and target the top hotspots (hash lookups, allocation, prefix hash cost).
- If needed, implement a second-level index for hot cap-hashes so lookups can directly go to cap-hash→positions with minimal scanning.

Long-term / aspirational
- Achieve 1000×1000 runs < 100 ms. This likely requires:
  - Compact indexing structures, very low allocation churn, good cache locality
  - Possibly a two-level index (k-mer → cap-hash → positions) or specialized data structure tuned to expected text patterns
  - Native, hand-optimized loops and lower-level memory layout (e.g., arena allocations, flat arrays)

Notes about perf and flamegraph
--------------------------------
If you want me to run perf here again, please ensure the following are in place before I run:
 - `sudo sysctl -w kernel.perf_event_paranoid=1` (or 0)
 - `sudo sysctl -w kernel.kptr_restrict=0` (optional for kernel symbol visibility)
 - `sudo apt-get install --reinstall linux-tools-$(uname -r) linux-cloud-tools-$(uname -r)` (if perf still complains)

Alternatively I can implement an in-process timer/sampler to produce a textual hotspot report without root, which can be useful while we get perf sorted.

Where to pick up
----------------
1. Re-run perf/flamegraph and provide non-empty `perf.unfolded` → I will analyze and point to concrete code-level hot spots.
2. Implement compact inner-bucket representation and preallocation micro-optimizations; re-run instrument/bench to measure improvements.

Notes for whoever continues
--------------------------
- Open files to inspect: `src/capped.rs` (main new implementation), `src/instrumentation.rs` (counters), `src/bin/instrument.rs` (runner and prints), `benches/bench_copyforward.rs` (bench harness), `tests/capped_tests.rs` (unit tests), `src/bin/sweep.rs` (sweep helper), and the `sweep_out/` directory (results of parallel sweep).
- To reproduce: run `cargo build --release --bin instrument` then `./target/release/instrument` or use the bench harness.

