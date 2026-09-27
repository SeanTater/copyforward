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
//! use copyforward::{greedy, Config, CopyForward};
//!
//! let messages = &["Hello world", "Hello world, how are you?"];
//! let compressed = greedy(messages, Config::default());
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
//! cf = copyforward.CopyForwardText.from_texts(messages)  # engine="greedy" (default)
//! print(cf.compression_ratio())
//! ```
//!
//! # Engine Selection
//!
//! Both engines perform the same greedy exact-match compression; they differ
//! in how much candidate search they spend per position.
//!
//! - **[`greedy()`]**: keeps the most recent occurrence of each k-mer and
//!   extends the match fully via binary search. Best on nested/growing
//!   threads and on data with many competing repeated fragments.
//! - **[`capped()`]**: keeps more candidate variants (one per 64-token
//!   context) but extends each only to a cap before fully extending the
//!   best. Fastest on incompressible and highly repetitive data.
//!
//! Both produce identical round-trips. Pick by data shape; see the
//! benchmark suite in `benches/` for measured tradeoffs.

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
    fn as_message(&self) -> Option<&str> {
        Some(self)
    }
}

impl MessageLike for Option<&str> {
    fn as_message(&self) -> Option<&str> {
        *self
    }
}

impl MessageLike for String {
    fn as_message(&self) -> Option<&str> {
        Some(self.as_str())
    }
}

impl MessageLike for Option<String> {
    fn as_message(&self) -> Option<&str> {
        self.as_deref()
    }
}

/// Trait for types that can be used as token inputs, supporting both regular tokens and None values.
pub trait TokenLike {
    fn as_tokens(&self) -> Option<&[u32]>;
}

impl TokenLike for &[u32] {
    fn as_tokens(&self) -> Option<&[u32]> {
        Some(self)
    }
}

impl TokenLike for Option<&[u32]> {
    fn as_tokens(&self) -> Option<&[u32]> {
        *self
    }
}

impl TokenLike for Vec<u32> {
    fn as_tokens(&self) -> Option<&[u32]> {
        Some(self.as_slice())
    }
}

impl TokenLike for Option<Vec<u32>> {
    fn as_tokens(&self) -> Option<&[u32]> {
        self.as_deref()
    }
}

/// Greedy copy-forward compression for token sequences (u32 IDs).
pub type GreedyTokens = hashed_binary::HashedGreedyBinary;

/// Capped copy-forward compression for token sequences (u32 IDs).
///
/// Keeps more candidate variants (one per 64-token context) but extends each
/// only to a cap before fully extending the best, then coalesces adjacent
/// references. Fastest on incompressible and highly repetitive data.
///
/// **Time complexity:** O(n) average case
/// **Space complexity:** O(n) for hash table and deduplication
pub type CappedTokens = capped::CappedHashedGreedy;

/// Text-mode wrapper for greedy algorithm routing through the token core.
#[derive(Debug, Clone)]
pub struct Greedy {
    inner: GreedyTokens,
    originals: Vec<String>,
    offsets: Vec<Vec<usize>>,  // byte offsets per Unicode-scalar boundary
    valid_indices: Vec<usize>, // indices of non-None messages
    none_mask: Vec<bool>,      // true for None entries
}

/// Text-mode wrapper for capped algorithm routing through the token core.
#[derive(Debug, Clone)]
pub struct Capped {
    inner: CappedTokens,
    originals: Vec<String>,
    offsets: Vec<Vec<usize>>,  // byte offsets per Unicode-scalar boundary
    valid_indices: Vec<usize>, // indices of non-None messages
    none_mask: Vec<bool>,      // true for None entries
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

/// Create a greedy copy-forward compressor.
///
/// Keeps the most recent occurrence of each k-mer and extends the match fully
/// via binary search. Best on nested/growing threads and on data with many
/// competing repeated fragments.
///
/// Supports both regular string slices and optional strings for handling missing values:
/// ```
/// use copyforward::{greedy, Config, CopyForward};
///
/// // Regular usage
/// let messages = &["Hello world", "Hello world today"];
/// let compressed = greedy(messages, Config::default());
///
/// // With None values (for dataframes)
/// let messages_with_none = &[Some("Hello"), None, Some("World")];
/// let compressed = greedy(messages_with_none, Config::default());
/// ```
pub fn greedy<M: MessageLike>(messages: &[M], config: Config) -> Greedy {
    let opts: Vec<Option<&str>> = messages.iter().map(|m| m.as_message()).collect();
    let originals: Vec<String> = opts
        .iter()
        .map(|opt| opt.unwrap_or("").to_string())
        .collect();
    let offsets: Vec<Vec<usize>> = originals.iter().map(|s| compute_offsets(s)).collect();
    let toks: Vec<Vec<u32>> = originals
        .iter()
        .map(|s| normalize::string_to_u32s(s))
        .collect();
    let valid_indices: Vec<usize> = opts
        .iter()
        .enumerate()
        .filter_map(|(i, opt)| if opt.is_some() { Some(i) } else { None })
        .collect();
    let filtered_toks: Vec<Vec<u32>> = valid_indices.iter().map(|&i| toks[i].clone()).collect();
    let refs: Vec<&[u32]> = filtered_toks.iter().map(|v| v.as_slice()).collect();
    let inner = hashed_binary::HashedGreedyBinary::new_tokens(&refs, config);
    Greedy {
        inner,
        originals,
        offsets,
        valid_indices,
        none_mask: opts.iter().map(|opt| opt.is_none()).collect(),
    }
}

/// Create a greedy token-mode compressor over u32 token sequences.
///
/// Supports both regular token slices and optional token slices for handling missing values.
pub fn greedy_tokens<T: TokenLike>(messages: &[T], config: Config) -> GreedyTokens {
    let filtered_tokens: Vec<&[u32]> = messages.iter().filter_map(|t| t.as_tokens()).collect();
    hashed_binary::HashedGreedyBinary::new_tokens(&filtered_tokens, config)
}

/// Create a capped copy-forward compressor.
///
/// Keeps more candidate variants (one per 64-token context) but extends each
/// only to a cap before fully extending the best. Fastest on incompressible
/// and highly repetitive data.
///
/// Supports both regular string slices and optional strings for handling missing values:
/// ```
/// use copyforward::{capped, Config, CopyForward};
///
/// // Regular usage
/// let messages = &["Hello world", "Hello world today"];  
/// let compressed = capped(messages, Config::default());
///
/// // With None values (for dataframes)
/// let messages_with_none = &[Some("Hello"), None, Some("World")];
/// let compressed = capped(messages_with_none, Config::default());
/// ```
pub fn capped<M: MessageLike>(messages: &[M], config: Config) -> Capped {
    let opts: Vec<Option<&str>> = messages.iter().map(|m| m.as_message()).collect();
    let originals: Vec<String> = opts
        .iter()
        .map(|opt| opt.unwrap_or("").to_string())
        .collect();
    let offsets: Vec<Vec<usize>> = originals.iter().map(|s| compute_offsets(s)).collect();
    let toks: Vec<Vec<u32>> = originals
        .iter()
        .map(|s| normalize::string_to_u32s(s))
        .collect();
    let valid_indices: Vec<usize> = opts
        .iter()
        .enumerate()
        .filter_map(|(i, opt)| if opt.is_some() { Some(i) } else { None })
        .collect();
    let filtered_toks: Vec<Vec<u32>> = valid_indices.iter().map(|&i| toks[i].clone()).collect();
    let refs: Vec<&[u32]> = filtered_toks.iter().map(|v| v.as_slice()).collect();
    let inner = capped::CappedHashedGreedy::new_tokens(&refs, config);
    Capped {
        inner,
        originals,
        offsets,
        valid_indices,
        none_mask: opts.iter().map(|opt| opt.is_none()).collect(),
    }
}

/// Map token segments back to text segments. With `byte_offsets`, reference
/// start/len are byte ranges; otherwise they are Unicode scalar (character)
/// ranges, matching how Python strings index.
fn map_text_segments(
    token_segs: &[Vec<TokenSegment>],
    valid_indices: &[usize],
    offsets: &[Vec<usize>],
    none_mask: &[bool],
    byte_offsets: bool,
) -> Vec<Vec<Segment>> {
    let mut out: Vec<Vec<Segment>> = Vec::with_capacity(none_mask.len());
    let mut token_seg_idx = 0;

    for &is_none in none_mask {
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
                        let orig_msg_idx = valid_indices[*ref_idx];
                        if byte_offsets {
                            let offs = &offsets[orig_msg_idx];
                            let bstart = offs[*start];
                            let bend = offs[start + len];
                            v.push(Segment::Reference {
                                message_idx: orig_msg_idx,
                                start: bstart,
                                len: bend - bstart,
                            });
                        } else {
                            v.push(Segment::Reference {
                                message_idx: orig_msg_idx,
                                start: *start,
                                len: *len,
                            });
                        }
                    }
                }
            }
            out.push(v);
            token_seg_idx += 1;
        }
    }
    out
}

impl CopyForward for Greedy {
    fn segments(&self) -> Vec<Vec<Segment>> {
        let token_segs = <GreedyTokens as CopyForwardTokens>::segments(&self.inner);
        map_text_segments(
            &token_segs,
            &self.valid_indices,
            &self.offsets,
            &self.none_mask,
            true,
        )
    }

    fn segments_chars(&self) -> Vec<Vec<Segment>> {
        let token_segs = <GreedyTokens as CopyForwardTokens>::segments(&self.inner);
        map_text_segments(
            &token_segs,
            &self.valid_indices,
            &self.offsets,
            &self.none_mask,
            false,
        )
    }

    fn render_with<F>(&self, mut replacer: F) -> Vec<String>
    where
        F: FnMut(usize, usize, usize, &str) -> String,
    {
        let token_segs = <GreedyTokens as CopyForwardTokens>::segments(&self.inner);
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

impl CopyForward for Capped {
    fn segments(&self) -> Vec<Vec<Segment>> {
        let token_segs = <CappedTokens as CopyForwardTokens>::segments(&self.inner);
        map_text_segments(
            &token_segs,
            &self.valid_indices,
            &self.offsets,
            &self.none_mask,
            true,
        )
    }

    fn segments_chars(&self) -> Vec<Vec<Segment>> {
        let token_segs = <CappedTokens as CopyForwardTokens>::segments(&self.inner);
        map_text_segments(
            &token_segs,
            &self.valid_indices,
            &self.offsets,
            &self.none_mask,
            false,
        )
    }

    fn render_with<F>(&self, mut replacer: F) -> Vec<String>
    where
        F: FnMut(usize, usize, usize, &str) -> String,
    {
        let token_segs = <CappedTokens as CopyForwardTokens>::segments(&self.inner);
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

/// Create a capped token-mode compressor over u32 token sequences.
///
/// Supports both regular token slices and optional token slices for handling missing values.
pub fn capped_tokens<T: TokenLike>(messages: &[T], config: Config) -> CappedTokens {
    let filtered_tokens: Vec<&[u32]> = messages.iter().filter_map(|t| t.as_tokens()).collect();
    capped::CappedHashedGreedy::new_tokens(&filtered_tokens, config)
}

// Tests live in the `tests/` directory as integration tests.
