use crate::core::{Config, TokenSegment};
use crate::hashing::{prefix_hashes_u32, range_hash};
use std::collections::HashMap;

/// Maximum candidates extended per lookup. Candidates that cannot beat the
/// current best do not count against this cap.
const MAX_EXAMINED: usize = 64;

/// Compute token segments using binary-search extension over &[u32] messages.
/// This mirrors HashedGreedyBinaryTokens::new logic but as a reusable engine.
pub fn compute_binary_segments(messages: &[Vec<u32>], config: &Config) -> Vec<Vec<TokenSegment>> {
    let mut inner: Vec<Vec<TokenSegment>> = Vec::with_capacity(messages.len());

    let base: u64 = 257;
    let prefixes: Vec<(Vec<u64>, Vec<u64>)> = messages
        .iter()
        .map(|m| prefix_hashes_u32(m, base))
        .collect();

    let k = config.min_match_len;
    let lookback = config.lookback;
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
    // K-mer hashes inserted per message, so lookback eviction can remove them.
    let mut inserted: Vec<Vec<u64>> = Vec::with_capacity(messages.len());
    // Dedupes identical k-mer hashes, keeping the most recent owner: the
    // shared table entry is repointed in place (at a stable slot) so it
    // always tracks the newest occurrence, the likeliest longest match in
    // growing threads. Only safe with unlimited lookback: under a bounded
    // window an earlier owner is evicted before a newer duplicate stops
    // needing the hash, which would leave it uncovered.
    let mut seen: HashMap<u64, (usize, usize, usize)> =
        HashMap::with_capacity((total_kmers / 2).max(16));
    let dedupe = lookback.is_none();

    fn insert_kmers(
        table: &mut HashMap<u64, Vec<(usize, usize)>>,
        seen: &mut HashMap<u64, (usize, usize, usize)>,
        messages: &[Vec<u32>],
        prefixes: &[(Vec<u64>, Vec<u64>)],
        j: usize,
        k: usize,
        dedupe: bool,
    ) -> Vec<u64> {
        let mut hashes = Vec::new();
        if messages[j].len() >= k {
            let (ref_h, ref_p) = &prefixes[j];
            for start in 0..=(messages[j].len() - k) {
                let h = range_hash(ref_h, ref_p, start, start + k);
                if dedupe {
                    if let Some((owner_msg, owner_start, owner_slot)) = seen.get(&h).copied() {
                        if j == owner_msg {
                            // Within one message the earliest start keeps
                            // the longest potential match.
                            continue;
                        }
                        // j > owner_msg: repoint the shared entry (at its
                        // stable slot) at the newer message's occurrence, so
                        // the front-of-bucket entry always tracks the most
                        // recent (likeliest longest) match.
                        let repointed = table
                            .get_mut(&h)
                            .and_then(|bucket| bucket.get_mut(owner_slot))
                            .is_some_and(|e| {
                                if e.0 != owner_msg || e.1 != owner_start {
                                    return false;
                                }
                                *e = (j, start);
                                true
                            });
                        if repointed {
                            seen.insert(h, (j, start, owner_slot));
                            continue;
                        }
                        // Stale slot (should not happen): fall through and
                        // insert a fresh entry.
                    }
                    // New pair: remember the slot the entry will occupy so a
                    // later duplicate can repoint it in O(1).
                    let slot = table.get(&h).map_or(0, |b| b.len());
                    seen.insert(h, (j, start, slot));
                }
                table.entry(h).or_default().push((j, start));
                hashes.push(h);
            }
        }
        hashes
    }

    fn evict(table: &mut HashMap<u64, Vec<(usize, usize)>>, victim: usize, hashes: &[u64]) {
        for &h in hashes {
            let drop = match table.get_mut(&h) {
                Some(bucket) => {
                    bucket.retain(|&(m, _)| m != victim);
                    bucket.is_empty()
                }
                None => false,
            };
            if drop {
                table.remove(&h);
            }
        }
    }

    /// Binary-search the longest extension by rolling hash, then verify the
    /// claimed length against actual content. A 64-bit hash collision can
    /// overstate the match, so a mismatch falls back to a linear scan of the
    /// true longest common prefix.
    #[allow(clippy::manual_div_ceil)]
    fn verified_extend(
        cur: &[u32],
        prev: &[u32],
        cursor: usize,
        ref_start: usize,
        pref_cur: &(Vec<u64>, Vec<u64>),
        pref_prev: &(Vec<u64>, Vec<u64>),
        initial_k: usize,
    ) -> usize {
        let max_possible = std::cmp::min(cur.len() - cursor, prev.len() - ref_start);
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
        if cur[cursor..cursor + low] == prev[ref_start..ref_start + low] {
            return low;
        }
        let mut m = 0usize;
        while m < max_possible && cur[cursor + m] == prev[ref_start + m] {
            m += 1;
        }
        m
    }

    for i in 0..messages.len() {
        let msg = &messages[i];

        if k > 0 && i > 0 {
            inserted.push(insert_kmers(
                &mut table,
                &mut seen,
                messages,
                &prefixes,
                i - 1,
                k,
                dedupe,
            ));
        } else if i > 0 {
            inserted.push(Vec::new());
        }
        if let Some(lb) = lookback
            && i > lb
        {
            let victim = i - lb - 1;
            evict(&mut table, victim, &inserted[victim]);
        }

        let mut cursor = 0usize;
        let mut segs = Vec::new();

        while cursor < msg.len() {
            let mut best_match: Option<(usize, usize, usize)> = None;

            if msg.len() >= cursor + k && k > 0 {
                let (cur_h, cur_p) = &prefixes[i];
                let key = range_hash(cur_h, cur_p, cursor, cursor + k);
                if let Some(cands) = table.get(&key) {
                    let remaining = msg.len() - cursor;
                    let mut examined = 0usize;
                    for &(midx, ref_start) in cands.iter() {
                        let max_possible = remaining.min(messages[midx].len() - ref_start);
                        if let Some((best_len, _, _)) = best_match
                            && max_possible <= best_len
                        {
                            continue;
                        }
                        if examined >= MAX_EXAMINED {
                            break;
                        }
                        examined += 1;
                        let match_len = verified_extend(
                            msg,
                            &messages[midx],
                            cursor,
                            ref_start,
                            &prefixes[i],
                            &prefixes[midx],
                            k,
                        );
                        if match_len < k {
                            // K-mer hash collision; not a real match.
                            continue;
                        }
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
