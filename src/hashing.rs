//! Shared hashing utilities for polynomial rolling hashes used by hashed algorithms.
//!
//! Uses wrapping u64 arithmetic (mod 2^64) for speed. This is not cryptographically
//! secure but collision rates are extremely low in practice for text compression.

/// Compute rolling prefix hashes and powers for a byte string.
/// Returns (h, p) where h[r] - h[l]*p[r-l] yields the rolling hash for s[l..r).
pub fn prefix_hashes(s: &[u8], base: u64) -> (Vec<u64>, Vec<u64>) {
    let mut h = Vec::with_capacity(s.len() + 1);
    let mut p = Vec::with_capacity(s.len() + 1);
    h.push(0);
    p.push(1);
    for &b in s {
        let last_h: u64 = *h.last().unwrap();
        h.push(last_h.wrapping_mul(base).wrapping_add(b as u64));
        let last_p: u64 = *p.last().unwrap();
        p.push(last_p.wrapping_mul(base));
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
    for &t in s {
        let last_h: u64 = *h.last().unwrap();
        h.push(last_h.wrapping_mul(base).wrapping_add(t as u64));
        let last_p: u64 = *p.last().unwrap();
        p.push(last_p.wrapping_mul(base));
    }
    (h, p)
}
