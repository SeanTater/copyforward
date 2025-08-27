//! Utilities to normalize text into u32 code points.
//!
//! The engines operate over sequences of u32 (Unicode scalar values). For text,
//! we map `char` to `u32`. The token-only core uses direct u32 conversion.

/// Build a String from a slice of Unicode scalar values (u32).
///
/// Panics if any value is not a valid Unicode scalar; this should not happen
/// when values originated from Rust `char` conversions.
pub fn string_from_u32(codes: &[u32]) -> String {
    let mut s = String::with_capacity(codes.len());
    for &u in codes {
        let ch = std::char::from_u32(u).expect("invalid unicode scalar in string_from_u32");
        s.push(ch);
    }
    s
}

/// Convert a UTF-8 string into a vector of Unicode scalar values (u32).
///
/// This is a thin adapter used by the token-only core. It does not compute
/// or return byte offsets.
pub fn string_to_u32s(text: &str) -> Vec<u32> {
    // Reserve exactly the number of chars to avoid reallocations
    let mut v = Vec::with_capacity(text.chars().count());
    for ch in text.chars() {
        v.push(ch as u32);
    }
    v
}

/// Convert a slice of Unicode scalar values (u32) back into a String.
///
/// Panics if any value is not a valid Unicode scalar; callers should only
/// pass values produced by `string_to_u32s` or other valid sources.
pub fn u32s_to_string(units: &[u32]) -> String {
    string_from_u32(units)
}
