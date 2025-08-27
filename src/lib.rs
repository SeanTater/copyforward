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

/// Trait for types that can be used as message inputs, supporting both regular strings and None values.
pub trait MessageLike {
    fn as_message(&self) -> Option<&str>;
}

impl MessageLike for &str {
    fn as_message(&self) -> Option<&str> { Some(self) }
}

impl MessageLike for Option<&str> {
    fn as_message(&self) -> Option<&str> { *self }
}

impl MessageLike for String {
    fn as_message(&self) -> Option<&str> { Some(self.as_str()) }
}

impl MessageLike for Option<String> {
    fn as_message(&self) -> Option<&str> { self.as_deref() }
}

/// Trait for types that can be used as token inputs, supporting both regular tokens and None values.
pub trait TokenLike {
    fn as_tokens(&self) -> Option<&[u32]>;
}

impl TokenLike for &[u32] {
    fn as_tokens(&self) -> Option<&[u32]> { Some(self) }
}

impl TokenLike for Option<&[u32]> {
    fn as_tokens(&self) -> Option<&[u32]> { *self }
}

impl TokenLike for Vec<u32> {
    fn as_tokens(&self) -> Option<&[u32]> { Some(self.as_slice()) }
}

impl TokenLike for Option<Vec<u32>> {
    fn as_tokens(&self) -> Option<&[u32]> { self.as_deref() }
}

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
    valid_indices: Vec<usize>, // indices of non-None messages
    none_mask: Vec<bool>, // true for None entries
}

/// Text-mode wrapper for approximate algorithm routing through the token core.
#[derive(Debug, Clone)]
pub struct Approximate {
    inner: ApproximateTokens,
    originals: Vec<String>,
    offsets: Vec<Vec<usize>>, // byte offsets per Unicode-scalar boundary
    valid_indices: Vec<usize>, // indices of non-None messages
    none_mask: Vec<bool>, // true for None entries
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
/// Supports both regular string slices and optional strings for handling missing values:
/// ```
/// use copyforward::{exact, Config, CopyForward};
///
/// // Regular usage
/// let messages = &["Hello world", "Hello world today"];
/// let compressed = exact(messages, Config::default());
///
/// // With None values (for dataframes)
/// let messages_with_none = &[Some("Hello"), None, Some("World")];
/// let compressed = exact(messages_with_none, Config::default());
/// ```
pub fn exact<M: MessageLike>(messages: &[M], config: Config) -> Exact {
    let opts: Vec<Option<&str>> = messages.iter().map(|m| m.as_message()).collect();
    let originals: Vec<String> = opts.iter().map(|opt| opt.unwrap_or("").to_string()).collect();
    let offsets: Vec<Vec<usize>> = originals.iter().map(|s| compute_offsets(s)).collect();
    let toks: Vec<Vec<u32>> = originals
        .iter()
        .map(|s| normalize::string_to_u32s(s))
        .collect();
    let valid_indices: Vec<usize> = opts.iter().enumerate().filter_map(|(i, opt)| if opt.is_some() { Some(i) } else { None }).collect();
    let filtered_toks: Vec<Vec<u32>> = valid_indices.iter().map(|&i| toks[i].clone()).collect();
    let refs: Vec<&[u32]> = filtered_toks.iter().map(|v| v.as_slice()).collect();
    let inner = hashed_binary::HashedGreedyBinary::new_tokens(&refs, config);
    Exact {
        inner,
        originals,
        offsets,
        valid_indices,
        none_mask: opts.iter().map(|opt| opt.is_none()).collect(),
    }
}

/// Create an exact token-mode compressor over u32 token sequences.
/// 
/// Supports both regular token slices and optional token slices for handling missing values.
pub fn exact_tokens<T: TokenLike>(messages: &[T], config: Config) -> ExactTokens {
    let filtered_tokens: Vec<&[u32]> = messages.iter().filter_map(|t| t.as_tokens()).collect();
    hashed_binary::HashedGreedyBinary::new_tokens(&filtered_tokens, config)
}

/// Create an approximate copy-forward compressor.
///
/// Caps extension at 64 bytes for ~2x speed improvement over [`exact()`]. May split
/// some long matches but still achieves excellent compression ratios.
///
/// Supports both regular string slices and optional strings for handling missing values:
/// ```
/// use copyforward::{approximate, Config, CopyForward};
///
/// // Regular usage
/// let messages = &["Hello world", "Hello world today"];  
/// let compressed = approximate(messages, Config::default());
///
/// // With None values (for dataframes)
/// let messages_with_none = &[Some("Hello"), None, Some("World")];
/// let compressed = approximate(messages_with_none, Config::default());
/// ```
pub fn approximate<M: MessageLike>(messages: &[M], config: Config) -> Approximate {
    let opts: Vec<Option<&str>> = messages.iter().map(|m| m.as_message()).collect();
    let originals: Vec<String> = opts.iter().map(|opt| opt.unwrap_or("").to_string()).collect();
    let offsets: Vec<Vec<usize>> = originals.iter().map(|s| compute_offsets(s)).collect();
    let toks: Vec<Vec<u32>> = originals
        .iter()
        .map(|s| normalize::string_to_u32s(s))
        .collect();
    let valid_indices: Vec<usize> = opts.iter().enumerate().filter_map(|(i, opt)| if opt.is_some() { Some(i) } else { None }).collect();
    let filtered_toks: Vec<Vec<u32>> = valid_indices.iter().map(|&i| toks[i].clone()).collect();
    let refs: Vec<&[u32]> = filtered_toks.iter().map(|v| v.as_slice()).collect();
    let inner = capped::CappedHashedGreedy::new_tokens(&refs, config);
    Approximate {
        inner,
        originals,
        offsets,
        valid_indices,
        none_mask: opts.iter().map(|opt| opt.is_none()).collect(),
    }
}

impl CopyForward for Exact {
    fn segments(&self) -> Vec<Vec<Segment>> {
        let token_segs = <ExactTokens as CopyForwardTokens>::segments(&self.inner);
        let mut out: Vec<Vec<Segment>> = Vec::with_capacity(self.none_mask.len());
        let mut token_seg_idx = 0;
        
        for &is_none in &self.none_mask {
            if is_none {
                // Empty segments for None entries
                out.push(vec![]);
            } else {
                let segs = &token_segs[token_seg_idx];
                let mut v: Vec<Segment> = Vec::with_capacity(segs.len());
                for seg in segs {
                    match seg {
                        TokenSegment::Literal(toks) => {
                            v.push(Segment::Literal(normalize::u32s_to_string(toks)))
                        }
                        TokenSegment::Reference {
                            message_idx: ref_idx,
                            start,
                            len,
                        } => {
                            // Map back to original indices
                            let orig_msg_idx = self.valid_indices[*ref_idx];
                            let offs = &self.offsets[orig_msg_idx];
                            let bstart = offs[*start];
                            let bend = offs[start + len];
                            v.push(Segment::Reference {
                                message_idx: orig_msg_idx,
                                start: bstart,
                                len: bend - bstart,
                            });
                        }
                    }
                }
                out.push(v);
                token_seg_idx += 1;
            }
        }
        out
    }

    fn render_with<F>(&self, mut replacer: F) -> Vec<String>
    where
        F: FnMut(usize, usize, usize, &str) -> String,
    {
        let token_segs = <ExactTokens as CopyForwardTokens>::segments(&self.inner);
        let mut out: Vec<String> = Vec::with_capacity(self.none_mask.len());
        let mut token_seg_idx = 0;
        
        for &is_none in &self.none_mask {
            if is_none {
                // Return original empty/none value
                out.push(String::new());
            } else {
                let segs = &token_segs[token_seg_idx];
                let mut s = String::new();
                for seg in segs {
                    match seg {
                        TokenSegment::Literal(toks) => s.push_str(&normalize::u32s_to_string(toks)),
                        TokenSegment::Reference {
                            message_idx: ref_idx,
                            start,
                            len,
                        } => {
                            let orig_msg_idx = self.valid_indices[*ref_idx];
                            let offs = &self.offsets[orig_msg_idx];
                            let bstart = offs[*start];
                            let bend = offs[start + len];
                            let ref_text = &self.originals[orig_msg_idx][bstart..bend];
                            let replaced = replacer(orig_msg_idx, bstart, bend - bstart, ref_text);
                            s.push_str(&replaced);
                        }
                    }
                }
                out.push(s);
                token_seg_idx += 1;
            }
        }
        out
    }
}

impl CopyForward for Approximate {
    fn segments(&self) -> Vec<Vec<Segment>> {
        let token_segs = <ApproximateTokens as CopyForwardTokens>::segments(&self.inner);
        let mut out: Vec<Vec<Segment>> = Vec::with_capacity(self.none_mask.len());
        let mut token_seg_idx = 0;
        
        for &is_none in &self.none_mask {
            if is_none {
                // Empty segments for None entries
                out.push(vec![]);
            } else {
                let segs = &token_segs[token_seg_idx];
                let mut v: Vec<Segment> = Vec::with_capacity(segs.len());
                for seg in segs {
                    match seg {
                        TokenSegment::Literal(toks) => {
                            v.push(Segment::Literal(normalize::u32s_to_string(toks)))
                        }
                        TokenSegment::Reference {
                            message_idx: ref_idx,
                            start,
                            len,
                        } => {
                            // Map back to original indices
                            let orig_msg_idx = self.valid_indices[*ref_idx];
                            let offs = &self.offsets[orig_msg_idx];
                            let bstart = offs[*start];
                            let bend = offs[start + len];
                            v.push(Segment::Reference {
                                message_idx: orig_msg_idx,
                                start: bstart,
                                len: bend - bstart,
                            });
                        }
                    }
                }
                out.push(v);
                token_seg_idx += 1;
            }
        }
        out
    }

    fn render_with<F>(&self, mut replacer: F) -> Vec<String>
    where
        F: FnMut(usize, usize, usize, &str) -> String,
    {
        let token_segs = <ApproximateTokens as CopyForwardTokens>::segments(&self.inner);
        let mut out: Vec<String> = Vec::with_capacity(self.none_mask.len());
        let mut token_seg_idx = 0;
        
        for &is_none in &self.none_mask {
            if is_none {
                // Return original empty/none value
                out.push(String::new());
            } else {
                let segs = &token_segs[token_seg_idx];
                let mut s = String::new();
                for seg in segs {
                    match seg {
                        TokenSegment::Literal(toks) => s.push_str(&normalize::u32s_to_string(toks)),
                        TokenSegment::Reference {
                            message_idx: ref_idx,
                            start,
                            len,
                        } => {
                            let orig_msg_idx = self.valid_indices[*ref_idx];
                            let offs = &self.offsets[orig_msg_idx];
                            let bstart = offs[*start];
                            let bend = offs[start + len];
                            let ref_text = &self.originals[orig_msg_idx][bstart..bend];
                            let replaced = replacer(orig_msg_idx, bstart, bend - bstart, ref_text);
                            s.push_str(&replaced);
                        }
                    }
                }
                out.push(s);
                token_seg_idx += 1;
            }
        }
        out
    }
}

/// Create an approximate token-mode compressor over u32 token sequences.
/// 
/// Supports both regular token slices and optional token slices for handling missing values.
pub fn approximate_tokens<T: TokenLike>(messages: &[T], config: Config) -> ApproximateTokens {
    let filtered_tokens: Vec<&[u32]> = messages.iter().filter_map(|t| t.as_tokens()).collect();
    capped::CappedHashedGreedy::new_tokens(&filtered_tokens, config)
}

// Tests live in the `tests/` directory as integration tests.
