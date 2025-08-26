# Agents & Notes

This file replaces the prior CLAUDE.md and summarizes the current state
of the project and guidance for agent/workflow contributors.

Status
------
- Instrumentation and dev-only binaries were removed prior to the first
  release to keep the codebase minimal and focused.
- The `CappedHashedGreedy` implementation is present and tuned to avoid
  allocator churn (uses `smallvec` buckets and a global seen set).

Guidance
--------
- Prefer small, well-scoped changes. Tests must only assert on correctness
  (rendered output), not on internal counters or heuristics.
- For profiling, use the removed `scripts/profile_capped.sh` workflow
  prior to deletion; rebuilding a simple script is fine for local profiling.

