use crate::core::{Config, CopyForwardTokens, TokenSegment};
use crate::engine::capped::compute_capped_segments;

/// Approximate hashed greedy (token-only): caps per-candidate extension to a
/// fixed length and then coalesces adjacent references to consecutive source
/// positions. Faster, approximate alternative to binary extension.
#[derive(Debug, Clone)]
pub struct CappedHashedGreedy {
    token_segs: Vec<Vec<TokenSegment>>, // segments in u32 units
    messages: Vec<Vec<u32>>,            // originals for rendering
    pub config: Config,
}

impl CappedHashedGreedy {
    pub fn new_tokens(messages: &[&[u32]], config: Config) -> CappedHashedGreedy {
        let messages_vec: Vec<Vec<u32>> = messages.iter().map(|s| s.to_vec()).collect();
        let inner = compute_capped_segments(&messages_vec, &config);
        CappedHashedGreedy {
            token_segs: inner,
            messages: messages_vec,
            config,
        }
    }
}

impl CopyForwardTokens for CappedHashedGreedy {
    fn segments(&self) -> Vec<Vec<TokenSegment>> {
        self.token_segs.clone()
    }

    fn render_with<F>(&self, mut replacer: F) -> Vec<Vec<u32>>
    where
        F: FnMut(usize, usize, usize, &[u32]) -> Vec<u32>,
    {
        let mut out = Vec::with_capacity(self.token_segs.len());
        for segs in self.token_segs.iter() {
            let mut v: Vec<u32> = Vec::new();
            for seg in segs {
                match seg {
                    TokenSegment::Literal(toks) => v.extend_from_slice(toks),
                    TokenSegment::Reference {
                        message_idx,
                        start,
                        len,
                    } => {
                        let ref_slice = &self.messages[*message_idx][*start..(*start + *len)];
                        let replaced = replacer(*message_idx, *start, *len, ref_slice);
                        v.extend_from_slice(&replaced);
                    }
                }
            }
            out.push(v);
        }
        out
    }
}
