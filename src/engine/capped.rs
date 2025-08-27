use crate::core::{Config, TokenSegment};
use crate::hashing::{prefix_hashes_u32, range_hash};
use ahash::AHashMap as HashMap;
use ahash::AHashSet as HashSet;
use smallvec::SmallVec;

#[derive(Clone, Copy)]
struct Entry {
    cap_hash: u64,
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
    let total_kmers: usize = if k > 0 {
        messages
            .iter()
            .map(|m| if m.len() >= k { m.len() - k + 1 } else { 0 })
            .sum()
    } else {
        0
    };
    let mut table: HashMap<u64, Bucket> = HashMap::with_capacity((total_kmers / 2).max(16));
    let mut seen: HashSet<(u64, u64)> = HashSet::with_capacity((total_kmers / 2).max(16));

    fn insert_kmers(
        table: &mut HashMap<u64, Bucket>,
        seen: &mut HashSet<(u64, u64)>,
        messages: &[Vec<u32>],
        prefixes: &[(Vec<u64>, Vec<u64>)],
        j: usize,
        k: usize,
        cap_len: usize,
    ) {
        if messages[j].len() >= k {
            let (ref_h, ref_p) = &prefixes[j];
            for start in 0..=(messages[j].len() - k) {
                let h = range_hash(ref_h, ref_p, start, start + k);
                let cap_end = std::cmp::min(messages[j].len(), start + cap_len);
                let cap_h = range_hash(ref_h, ref_p, start, cap_end);
                let key = (h, cap_h);
                if !seen.contains(&key) {
                    seen.insert(key);
                    table.entry(h).or_default().push(Entry {
                        cap_hash: cap_h,
                        msg_idx: j,
                        start,
                    });
                }
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

    #[allow(clippy::manual_div_ceil)]
    fn extend_full(
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
            insert_kmers(
                &mut table,
                &mut seen,
                messages,
                &prefixes,
                j,
                k,
                config.cap_len,
            );
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
                let cap_hash_cur = range_hash(cur_h, cur_p, cursor, cap_end_cur);
                if let Some(bucket) = table.get(&kmer_hash) {
                    for e in bucket.iter() {
                        if examined >= ncap {
                            break;
                        }
                        let midx = e.msg_idx;
                        let ref_start = e.start;
                        if midx >= i {
                            continue;
                        }
                        if e.cap_hash != cap_hash_cur {
                            examined += 1;
                            continue;
                        }
                        let prev = &messages[midx];
                        let match_len = extend_capped(msg, prev, cursor, ref_start, k, cap_len);
                        if best_match.is_none() || match_len > best_match.unwrap().0 {
                            best_match = Some((match_len, midx, ref_start));
                        }
                        examined += 1;
                    }
                }
            }

            if let Some((match_len, midx, ref_start)) = best_match {
                let full_len =
                    extend_full(&prefixes[i], &prefixes[midx], cursor, ref_start, match_len);
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
