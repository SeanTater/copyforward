#![allow(unsafe_op_in_unsafe_fn)]
pub mod fixture;
pub mod python_bindings;
// Lightweight instrumentation for counting hotspots in development.
// Uses atomics to avoid locking overhead; reset and snapshot helpers
// allow a small dev binary to collect simple breakdowns.
// instrumentation removed for production/profile runs; keep no-op stub
pub mod core;
pub mod greedy;
pub mod hashed;

pub mod capped;
pub mod hashed_binary;

pub use crate::capped::CappedHashedGreedy;
pub use crate::core::{CopyForward, GreedySubstringConfig, Segment};
pub use crate::greedy::GreedySubstring;
pub use crate::hashed::HashedGreedy;
pub use crate::hashed_binary::HashedGreedyBinary;

// Tests live in the `tests/` directory as integration tests.
