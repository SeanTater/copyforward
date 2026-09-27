//! Shared hashing utilities for polynomial rolling hashes used by hashed algorithms.
//!
//! Uses wrapping u64 arithmetic (mod 2^64) for speed. This is not cryptographically
//! secure but collision rates are extremely low in practice for text compression.

/// Compute rolling prefix hashes and powers for a byte string.
/// Returns `(h, p)` where `h[r] - h[l]*p[r-l]` yields the rolling hash for `s[l..r)`.
pub fn prefix_hashes(s: &[u8], base: u64) -> (Vec<u64>, Vec<u64>) {
    let mut h = Vec::with_capacity(s.len() + 1);
    let mut p = Vec::with_capacity(s.len() + 1);
    h.push(0);
    p.push(1);
    let mut lh = 0u64;
    let mut lp = 1u64;
    for &b in s {
        lh = lh.wrapping_mul(base).wrapping_add(b as u64);
        h.push(lh);
        lp = lp.wrapping_mul(base);
        p.push(lp);
    }
    (h, p)
}

/// Hash substring [l, r) using prefix info (r is exclusive).
pub fn range_hash(h: &[u64], p: &[u64], l: usize, r: usize) -> u64 {
    h[r].wrapping_sub(h[l].wrapping_mul(p[r - l]))
}

/// Compute rolling prefix hashes and powers for a u32 token sequence.
/// Mirrors `prefix_hashes` but consumes u32 values.
pub fn prefix_hashes_u32(s: &[u32], base: u64) -> (Vec<u64>, Vec<u64>) {
    let mut h = Vec::with_capacity(s.len() + 1);
    let mut p = Vec::with_capacity(s.len() + 1);
    h.push(0);
    p.push(1);
    let mut lh = 0u64;
    let mut lp = 1u64;
    for &t in s {
        lh = lh.wrapping_mul(base).wrapping_add(t as u64);
        h.push(lh);
        lp = lp.wrapping_mul(base);
        p.push(lp);
    }
    (h, p)
}
