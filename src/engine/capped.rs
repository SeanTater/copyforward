use crate::core::{Config, TokenSegment};
use crate::hashing::{prefix_hashes_u32, range_hash};
use ahash::AHashMap as HashMap;
use smallvec::SmallVec;

#[derive(Clone, Copy)]
struct Entry {
    /// Number of tokens in the capped window (may be shorter than the cap
    /// when the source message ends early).
    cap_span: usize,
    msg_idx: usize,
    start: usize,
}
type Bucket = SmallVec<[Entry; 4]>;

/// Compute token segments using capped extension with per-candidate early stop
/// and winner-local full extension using rolling hashes.
pub fn compute_capped_segments(messages: &[Vec<u32>], config: &Config) -> Vec<Vec<TokenSegment>> {
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
    let mut table: HashMap<u64, Bucket> = HashMap::with_capacity((total_kmers / 2).max(16));
    // Dedupes identical (kmer hash, capped hash) pairs, keeping the most
    // recent owner: in copy-forward workloads the newest occurrence is the
    // likeliest best match, so the shared table entry is updated in place
    // instead of pointing at the oldest occurrence. Only safe with
    // unlimited lookback: under a bounded window an earlier owner is
    // evicted before a newer duplicate stops needing the pair, which would
    // leave it uncovered.
    let mut seen: HashMap<(u64, u64), (usize, usize, usize)> =
        HashMap::with_capacity((total_kmers / 2).max(16));
    // Pairs inserted per message, so lookback eviction can remove them.
    let mut inserted: Vec<Vec<(u64, u64)>> = Vec::with_capacity(messages.len());

    fn insert_kmers(
        table: &mut HashMap<u64, Bucket>,
        seen: &mut HashMap<(u64, u64), (usize, usize, usize)>,
        messages: &[Vec<u32>],
        prefixes: &[(Vec<u64>, Vec<u64>)],
        j: usize,
        config: &Config,
    ) -> Vec<(u64, u64)> {
        let k = config.min_match_len;
        let cap_len = config.cap_len;
        let dedupe = config.lookback.is_none();
        let mut pairs: Vec<(u64, u64)> = Vec::new();
        if messages[j].len() >= k {
            let (ref_h, ref_p) = &prefixes[j];
            for start in 0..=(messages[j].len() - k) {
                let h = range_hash(ref_h, ref_p, start, start + k);
                let cap_end = std::cmp::min(messages[j].len(), start + cap_len);
                let cap_h = range_hash(ref_h, ref_p, start, cap_end);
                let key = (h, cap_h);
                let span = cap_end - start;
                if dedupe {
                    if let Some((owner_msg, owner_start, owner_slot)) = seen.get(&key).copied() {
                        if j == owner_msg {
                            // Within one message the earliest start keeps
                            // the longest potential match.
                            continue;
                        }
                        // j > owner_msg: repoint the shared entry (at its
                        // stable slot) at the newer message's occurrence.
                        let repointed = table
                            .get_mut(&h)
                            .and_then(|bucket| bucket.get_mut(owner_slot))
                            .is_some_and(|e| {
                                if e.msg_idx != owner_msg || e.start != owner_start {
                                    return false;
                                }
                                e.msg_idx = j;
                                e.start = start;
                                e.cap_span = span;
                                true
                            });
                        if repointed {
                            seen.insert(key, (j, start, owner_slot));
                            continue;
                        }
                        // Stale slot (should not happen): fall through and
                        // insert a fresh entry.
                    }
                    // New pair: remember the slot the entry will occupy so a
                    // later duplicate can repoint it in O(1).
                    let slot = table.get(&h).map_or(0, |b| b.len());
                    seen.insert(key, (j, start, slot));
                }
                table.entry(h).or_default().push(Entry {
                    cap_span: span,
                    msg_idx: j,
                    start,
                });
                pairs.push(key);
            }
        }
        pairs
    }

    fn evict(table: &mut HashMap<u64, Bucket>, victim: usize, pairs: &[(u64, u64)]) {
        for &(h, _cap_h) in pairs {
            let drop = match table.get_mut(&h) {
                Some(bucket) => {
                    bucket.retain(|e| e.msg_idx != victim);
                    bucket.is_empty()
                }
                None => false,
            };
            if drop {
                table.remove(&h);
            }
        }
    }

    fn extend_capped(
        cur: &[u32],
        prev: &[u32],
        cursor: usize,
        ref_start: usize,
        initial_k: usize,
        cap_len: usize,
    ) -> usize {
        let mut match_len = initial_k;
        while match_len < cap_len
            && cursor + match_len < cur.len()
            && ref_start + match_len < prev.len()
            && cur[cursor + match_len] == prev[ref_start + match_len]
        {
            match_len += 1;
        }
        match_len
    }

    /// Extend the verified match as far as the rolling hash allows, then
    /// verify the claimed length against actual content. A 64-bit hash
    /// collision can overstate the match, so a mismatch falls back to a
    /// linear scan from the verified prefix.
    #[allow(clippy::manual_div_ceil)]
    fn verified_full_extend(
        cur: &[u32],
        prev: &[u32],
        cursor: usize,
        ref_start: usize,
        pref_cur: &(Vec<u64>, Vec<u64>),
        pref_prev: &(Vec<u64>, Vec<u64>),
        initial: usize,
    ) -> usize {
        let max_possible = std::cmp::min(cur.len() - cursor, prev.len() - ref_start);
        let mut low = initial;
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
        let mut m = initial;
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
                config,
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
                let kmer_hash = range_hash(cur_h, cur_p, cursor, cursor + k);
                let mut examined = 0usize;
                let cap_len = config.cap_len;
                let ncap = config.ncap;
                let cap_end_cur = std::cmp::min(msg.len(), cursor + cap_len);
                let cur_span = cap_end_cur - cursor;
                if let Some(bucket) = table.get(&kmer_hash) {
                    let remaining = msg.len() - cursor;
                    for e in bucket.iter() {
                        let midx = e.msg_idx;
                        let ref_start = e.start;
                        // Candidates that cannot beat the current best do
                        // not spend the ncap budget.
                        let max_possible = remaining.min(messages[midx].len() - ref_start);
                        if let Some((best_len, _, _)) = best_match
                            && max_possible <= best_len
                        {
                            continue;
                        }
                        if examined >= ncap {
                            break;
                        }
                        // Compare capped windows over their common length so
                        // a match ending at the current message's boundary is
                        // not rejected just because the source window is
                        // longer.
                        let common = cur_span.min(e.cap_span);
                        let h_cur = range_hash(cur_h, cur_p, cursor, cursor + common);
                        let h_prev = range_hash(
                            &prefixes[midx].0,
                            &prefixes[midx].1,
                            ref_start,
                            ref_start + common,
                        );
                        if h_cur != h_prev {
                            examined += 1;
                            continue;
                        }
                        let prev = &messages[midx];
                        if msg[cursor..cursor + k] != prev[ref_start..ref_start + k] {
                            // K-mer hash collision; not a real match.
                            examined += 1;
                            continue;
                        }
                        let match_len = extend_capped(msg, prev, cursor, ref_start, k, cap_len);
                        if best_match.is_none() || match_len > best_match.unwrap().0 {
                            best_match = Some((match_len, midx, ref_start));
                        }
                        examined += 1;
                    }
                }
            }

            if let Some((match_len, midx, ref_start)) = best_match {
                let full_len = verified_full_extend(
                    msg,
                    &messages[midx],
                    cursor,
                    ref_start,
                    &prefixes[i],
                    &prefixes[midx],
                    match_len,
                );
                segs.push(TokenSegment::Reference {
                    message_idx: midx,
                    start: ref_start,
                    len: full_len,
                });
                cursor += full_len;
            } else {
                let mut literal_end = cursor + 1;
                while literal_end < msg.len() {
                    let mut found = false;
                    if k > 0 {
                        let (cur_h, cur_p) = &prefixes[i];
                        if msg.len() >= literal_end + k {
                            let kmer_hash2 = range_hash(cur_h, cur_p, literal_end, literal_end + k);
                            if table.contains_key(&kmer_hash2) {
                                found = true;
                            }
                        }
                    }
                    if found {
                        break;
                    }
                    literal_end += 1;
                }
                segs.push(TokenSegment::Literal(msg[cursor..literal_end].to_vec()));
                cursor = literal_end;
            }
        }

        inner.push(segs);
    }

    // Coalesce consecutive references to consecutive source spans
    for segs in inner.iter_mut() {
        let mut out: Vec<TokenSegment> = Vec::with_capacity(segs.len());
        let mut i = 0usize;
        while i < segs.len() {
            match &segs[i] {
                TokenSegment::Reference {
                    message_idx,
                    start,
                    len,
                } => {
                    let cur_msg = *message_idx;
                    let cur_start = *start;
                    let mut cur_len = *len;
                    i += 1;
                    while i < segs.len() {
                        if let TokenSegment::Reference {
                            message_idx: m2,
                            start: s2,
                            len: l2,
                        } = &segs[i]
                            && *m2 == cur_msg
                            && *s2 == cur_start + cur_len
                        {
                            cur_len += *l2;
                            i += 1;
                            continue;
                        }
                        break;
                    }
                    out.push(TokenSegment::Reference {
                        message_idx: cur_msg,
                        start: cur_start,
                        len: cur_len,
                    });
                }
                TokenSegment::Literal(l) => {
                    out.push(TokenSegment::Literal(l.clone()));
                    i += 1;
                }
            }
        }
        *segs = out;
    }

    inner
}
