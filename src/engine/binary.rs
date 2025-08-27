use crate::core::{Config, TokenSegment};
use crate::hashing::{prefix_hashes_u32, range_hash};

/// Compute token segments using binary-search extension over &[u32] messages.
/// This mirrors HashedGreedyBinaryTokens::new logic but as a reusable engine.
pub fn compute_binary_segments(messages: &[Vec<u32>], config: &Config) -> Vec<Vec<TokenSegment>> {
    use std::collections::HashMap;
    let mut inner: Vec<Vec<TokenSegment>> = Vec::with_capacity(messages.len());

    let base: u64 = 257;
    let prefixes: Vec<(Vec<u64>, Vec<u64>)> = messages
        .iter()
        .map(|m| prefix_hashes_u32(m, base))
        .collect();

    let k = config.min_match_len;
    let total_kmers: usize = if k > 0 {
        messages
            .iter()
            .map(|m| if m.len() >= k { m.len() - k + 1 } else { 0 })
            .sum()
    } else {
        0
    };
    let mut table: HashMap<u64, Vec<(usize, usize)>> =
        HashMap::with_capacity((total_kmers / 2).max(16));

    fn insert_kmers(
        table: &mut HashMap<u64, Vec<(usize, usize)>>,
        messages: &[Vec<u32>],
        prefixes: &[(Vec<u64>, Vec<u64>)],
        j: usize,
        k: usize,
    ) {
        if messages[j].len() >= k {
            let (ref_h, ref_p) = &prefixes[j];
            for start in 0..=(messages[j].len() - k) {
                let h = range_hash(ref_h, ref_p, start, start + k);
                table.entry(h).or_default().push((j, start));
            }
        }
    }

    #[allow(clippy::manual_div_ceil)]
    fn extend_candidate(
        pref_cur: &(Vec<u64>, Vec<u64>),
        pref_prev: &(Vec<u64>, Vec<u64>),
        cursor: usize,
        ref_start: usize,
        initial_k: usize,
    ) -> usize {
        let max_possible = std::cmp::min(
            pref_cur.0.len() - 1 - cursor,
            pref_prev.0.len() - 1 - ref_start,
        );
        let mut low = initial_k;
        let mut high = max_possible;
        while low < high {
            let mid = low + (high - low + 1) / 2;
            let h1 = range_hash(&pref_cur.0, &pref_cur.1, cursor, cursor + mid);
            let h2 = range_hash(&pref_prev.0, &pref_prev.1, ref_start, ref_start + mid);
            if h1 == h2 {
                low = mid;
            } else {
                high = mid - 1;
            }
        }
        low
    }

    for i in 0..messages.len() {
        let msg = &messages[i];

        if k > 0 && i > 0 {
            let j = i - 1;
            insert_kmers(&mut table, messages, &prefixes, j, k);
        }

        let mut cursor = 0usize;
        let mut segs = Vec::new();

        while cursor < msg.len() {
            let mut best_match: Option<(usize, usize, usize)> = None;

            if msg.len() >= cursor + k && k > 0 {
                let (cur_h, cur_p) = &prefixes[i];
                let key = range_hash(cur_h, cur_p, cursor, cursor + k);
                if let Some(cands) = table.get(&key) {
                    for (examined, &(midx, ref_start)) in cands.iter().enumerate() {
                        if examined >= 64 {
                            break;
                        }
                        let prev_pref = &prefixes[midx];
                        let match_len =
                            extend_candidate(&prefixes[i], prev_pref, cursor, ref_start, k);
                        if best_match.is_none() || match_len > best_match.unwrap().0 {
                            best_match = Some((match_len, midx, ref_start));
                        }
                    }
                }
            }

            if let Some((match_len, midx, ref_start)) = best_match {
                segs.push(TokenSegment::Reference {
                    message_idx: midx,
                    start: ref_start,
                    len: match_len,
                });
                cursor += match_len;
            } else {
                let mut literal_end = cursor + 1;
                while literal_end < msg.len() {
                    if msg.len() >= literal_end + k && k > 0 {
                        let (cur_h, cur_p) = &prefixes[i];
                        let key = range_hash(cur_h, cur_p, literal_end, literal_end + k);
                        if table.contains_key(&key) {
                            break;
                        }
                    }
                    literal_end += 1;
                }
                let lit = msg[cursor..literal_end].to_vec();
                segs.push(TokenSegment::Literal(lit));
                cursor = literal_end;
            }
        }

        inner.push(segs);
    }

    inner
}
