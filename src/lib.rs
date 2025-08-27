//! Fast copy-forward compression for message threads.
//!
//! Detects repeated substrings across messages and replaces them with
//! references to earlier occurrences, reducing storage and bandwidth.
//!
//! Copy-forward compression is particularly effective for:
//! - Chat logs and message threads with quoted replies
//! - Document version histories with incremental changes  
//! - Any sequence of texts with repeated phrases or patterns
//!
//! # Quick Start
//!
//! ## Rust
//!
//! ```
//! use copyforward::{exact, Config, CopyForward};
//!
//! let messages = &["Hello world", "Hello world, how are you?"];
//! let compressed = exact(messages, Config::default());
//!
//! // Render back to original text
//! let original = compressed.render_with(|_, _, _, text| text.to_string());
//! assert_eq!(original, messages);
//! ```
//!
//! ## Python
//!
//! ```python
//! import copyforward
//!
//! messages = ["Hello world", "Hello world, how are you?"]
//! # Exact mode (default) - perfect compression
//! cf = copyforward.CopyForward.from_texts(messages)
//! print(cf.compression_ratio())
//! ```
//!
//! # Algorithm Selection
//!
//! Choose between two optimized algorithms:
//!
//! - **[`exact()`]**: Perfect compression using binary search extension (O(n log m) time)
//!   - Best for: <1MB total text, when perfect compression is needed
//!   - Finds optimal substring matches, never misses opportunities
//!
//! - **[`approximate()`]**: Fast compression with capped extension (~2x faster)  
//!   - Best for: >1MB text, when speed matters more than perfect compression
//!   - May split long references into multiple shorter ones
//!   - Still achieves excellent compression ratios (typically 50-90% size reduction)

#![allow(unsafe_op_in_unsafe_fn)]

mod capped;
pub mod core;
mod engine;
pub mod fixture;
mod hashed_binary;
pub mod hashing;
mod normalize;
#[cfg(feature = "python")]
pub mod python_bindings;
pub mod tokenization;

// Public API - only expose what users need
pub use crate::core::{Config, CopyForward, CopyForwardTokens, Segment, TokenSegment};

/// Exact copy-forward compression for token sequences (u32 IDs).
pub type ExactTokens = hashed_binary::HashedGreedyBinary;

/// Approximate copy-forward compression with capped extension.
///
/// Uses rolling hash indexing but caps extension at 64 bytes per candidate, then
/// coalesces adjacent references. Trades perfect accuracy for ~2x speed improvement.
///
/// **Time complexity:** O(n) average case  
/// **Space complexity:** O(n) for hash table and deduplication
///
/// Best for large message sets (>1MB) where speed matters more than perfect compression.
/// Still achieves excellent ratios, just may split some long matches into multiple references.
/// Approximate copy-forward compression for token sequences (u32 IDs).
pub type ApproximateTokens = capped::CappedHashedGreedy;

/// Text-mode wrapper for exact algorithm routing through the token core.
#[derive(Debug, Clone)]
pub struct Exact {
    inner: ExactTokens,
    originals: Vec<String>,
    offsets: Vec<Vec<usize>>, // byte offsets per Unicode-scalar boundary
}

/// Text-mode wrapper for approximate algorithm routing through the token core.
#[derive(Debug, Clone)]
pub struct Approximate {
    inner: ApproximateTokens,
    originals: Vec<String>,
    offsets: Vec<Vec<usize>>, // byte offsets per Unicode-scalar boundary
}

fn compute_offsets(s: &str) -> Vec<usize> {
    let mut offs: Vec<usize> = Vec::with_capacity(s.chars().count() + 1);
    offs.push(0);
    for (byte_idx, _) in s.char_indices() {
        if *offs.last().unwrap() != byte_idx {
            offs.push(byte_idx);
        }
    }
    if *offs.last().unwrap() != s.len() {
        offs.push(s.len());
    }
    offs
}

/// Create an exact copy-forward compressor.
///
/// Uses binary search extension to find optimal substring matches. Perfect compression
/// but slower than [`approximate()`] for large texts.
///
/// # Example
/// ```
/// use copyforward::{exact, Config, CopyForward};
///
/// let messages = &["Hello world", "Hello world today"];
/// let compressed = exact(messages, Config::default());
/// let segments = compressed.segments();
/// ```
pub fn exact(messages: &[&str], config: Config) -> Exact {
    let originals: Vec<String> = messages.iter().map(|s| (*s).to_string()).collect();
    let offsets: Vec<Vec<usize>> = originals.iter().map(|s| compute_offsets(s)).collect();
    let toks: Vec<Vec<u32>> = originals
        .iter()
        .map(|s| normalize::string_to_u32s(s))
        .collect();
    let refs: Vec<&[u32]> = toks.iter().map(|v| v.as_slice()).collect();
    let inner = exact_tokens(&refs, config);
    Exact {
        inner,
        originals,
        offsets,
    }
}

/// Create an exact token-mode compressor over u32 token sequences.
pub fn exact_tokens(messages: &[&[u32]], config: Config) -> ExactTokens {
    hashed_binary::HashedGreedyBinary::new_tokens(messages, config)
}

/// Create an approximate copy-forward compressor.
///
/// Caps extension at 64 bytes for ~2x speed improvement over [`exact()`]. May split
/// some long matches but still achieves excellent compression ratios.
///
/// # Example
/// ```
/// use copyforward::{approximate, Config, CopyForward};
///
/// let messages = &["Hello world", "Hello world today"];  
/// let compressed = approximate(messages, Config::default());
/// let ratio = compressed.segments().len() as f64 / messages.len() as f64;
/// ```
pub fn approximate(messages: &[&str], config: Config) -> Approximate {
    let originals: Vec<String> = messages.iter().map(|s| (*s).to_string()).collect();
    let offsets: Vec<Vec<usize>> = originals.iter().map(|s| compute_offsets(s)).collect();
    let toks: Vec<Vec<u32>> = originals
        .iter()
        .map(|s| normalize::string_to_u32s(s))
        .collect();
    let refs: Vec<&[u32]> = toks.iter().map(|v| v.as_slice()).collect();
    let inner = approximate_tokens(&refs, config);
    Approximate {
        inner,
        originals,
        offsets,
    }
}

impl CopyForward for Exact {
    fn segments(&self) -> Vec<Vec<Segment>> {
        let token_segs = <ExactTokens as CopyForwardTokens>::segments(&self.inner);
        let mut out: Vec<Vec<Segment>> = Vec::with_capacity(token_segs.len());
        for segs in token_segs.into_iter() {
            let mut v: Vec<Segment> = Vec::with_capacity(segs.len());
            for seg in segs {
                match seg {
                    TokenSegment::Literal(toks) => {
                        v.push(Segment::Literal(normalize::u32s_to_string(&toks)))
                    }
                    TokenSegment::Reference {
                        message_idx,
                        start,
                        len,
                    } => {
                        let offs = &self.offsets[message_idx];
                        let bstart = offs[start];
                        let bend = offs[start + len];
                        v.push(Segment::Reference {
                            message_idx,
                            start: bstart,
                            len: bend - bstart,
                        });
                    }
                }
            }
            out.push(v);
        }
        out
    }

    fn render_with<F>(&self, mut replacer: F) -> Vec<String>
    where
        F: FnMut(usize, usize, usize, &str) -> String,
    {
        let token_segs = <ExactTokens as CopyForwardTokens>::segments(&self.inner);
        let mut out: Vec<String> = Vec::with_capacity(token_segs.len());
        for segs in token_segs.into_iter() {
            let mut s = String::new();
            for seg in segs {
                match seg {
                    TokenSegment::Literal(toks) => s.push_str(&normalize::u32s_to_string(&toks)),
                    TokenSegment::Reference {
                        message_idx,
                        start,
                        len,
                    } => {
                        let offs = &self.offsets[message_idx];
                        let bstart = offs[start];
                        let bend = offs[start + len];
                        let ref_text = &self.originals[message_idx][bstart..bend];
                        let replaced = replacer(message_idx, bstart, bend - bstart, ref_text);
                        s.push_str(&replaced);
                    }
                }
            }
            out.push(s);
        }
        out
    }
}

impl CopyForward for Approximate {
    fn segments(&self) -> Vec<Vec<Segment>> {
        let token_segs = <ApproximateTokens as CopyForwardTokens>::segments(&self.inner);
        let mut out: Vec<Vec<Segment>> = Vec::with_capacity(token_segs.len());
        for segs in token_segs.into_iter() {
            let mut v: Vec<Segment> = Vec::with_capacity(segs.len());
            for seg in segs {
                match seg {
                    TokenSegment::Literal(toks) => {
                        v.push(Segment::Literal(normalize::u32s_to_string(&toks)))
                    }
                    TokenSegment::Reference {
                        message_idx,
                        start,
                        len,
                    } => {
                        let offs = &self.offsets[message_idx];
                        let bstart = offs[start];
                        let bend = offs[start + len];
                        v.push(Segment::Reference {
                            message_idx,
                            start: bstart,
                            len: bend - bstart,
                        });
                    }
                }
            }
            out.push(v);
        }
        out
    }

    fn render_with<F>(&self, mut replacer: F) -> Vec<String>
    where
        F: FnMut(usize, usize, usize, &str) -> String,
    {
        let token_segs = <ApproximateTokens as CopyForwardTokens>::segments(&self.inner);
        let mut out: Vec<String> = Vec::with_capacity(token_segs.len());
        for segs in token_segs.into_iter() {
            let mut s = String::new();
            for seg in segs {
                match seg {
                    TokenSegment::Literal(toks) => s.push_str(&normalize::u32s_to_string(&toks)),
                    TokenSegment::Reference {
                        message_idx,
                        start,
                        len,
                    } => {
                        let offs = &self.offsets[message_idx];
                        let bstart = offs[start];
                        let bend = offs[start + len];
                        let ref_text = &self.originals[message_idx][bstart..bend];
                        let replaced = replacer(message_idx, bstart, bend - bstart, ref_text);
                        s.push_str(&replaced);
                    }
                }
            }
            out.push(s);
        }
        out
    }
}

/// Create an approximate token-mode compressor over u32 token sequences.
pub fn approximate_tokens(messages: &[&[u32]], config: Config) -> ApproximateTokens {
    capped::CappedHashedGreedy::new_tokens(messages, config)
}

// Tests live in the `tests/` directory as integration tests.
