#![allow(unsafe_op_in_unsafe_fn)]
pub mod fixture;
pub mod python_bindings;
// Lightweight instrumentation for counting hotspots in development.
// Uses atomics to avoid locking overhead; reset and snapshot helpers
// allow a small dev binary to collect simple breakdowns.
pub mod instrumentation;

pub mod core;
pub mod greedy;
pub mod hashed;

pub mod hashed_binary;
pub mod capped;

pub use crate::core::{CopyForward, GreedySubstringConfig, Segment};
pub use crate::greedy::GreedySubstring;
pub use crate::hashed::HashedGreedy;
pub use crate::hashed_binary::HashedGreedyBinary;
pub use crate::capped::CappedHashedGreedy;

// Tests live in the `tests/` directory as integration tests.
