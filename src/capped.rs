use crate::core::{CopyForward, GreedySubstringConfig, Segment};
use ahash::AHashMap as HashMap;
use ahash::AHashSet as HashSet;
use smallvec::SmallVec;

#[derive(Clone, Copy)]
struct Entry { cap_hash: u64, msg_idx: usize, start: usize }
type Bucket = SmallVec<[Entry; 4]>;

/// Approximate hashed greedy that caps per-candidate extension to a fixed length
/// and then coalesces adjacent references that point to consecutive source
/// positions. This is intended as a faster, approximate alternative.
pub struct CappedHashedGreedy {
    inner: Vec<Vec<Segment>>,
    messages: Vec<String>,
    pub config: GreedySubstringConfig,
}

impl CappedHashedGreedy {
    // CAP_LEN and NCAP are compile-time constants.
    const CAP_LEN: usize = 64;
    const NCAP: usize = 64;
    #[inline]
    fn cap_len() -> usize { Self::CAP_LEN }
    #[inline]
    fn ncap() -> usize { Self::NCAP }

    fn prefix_hashes(s: &[u8], base: u64) -> (Vec<u64>, Vec<u64>) {
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

    fn range_hash(h: &Vec<u64>, p: &Vec<u64>, l: usize, r: usize) -> u64 {
        h[r].wrapping_sub(h[l].wrapping_mul(p[r - l]))
    }

    fn insert_kmers_into_table(
        table: &mut HashMap<u64, Bucket>,
        seen: &mut HashSet<(u64,u64)>,
        messages_vec: &Vec<String>,
        prefixes: &Vec<(Vec<u64>, Vec<u64>)>,
        j: usize,
        k: usize,
    ) {
        if messages_vec[j].len() >= k {
            let (ref_h, ref_p) = &prefixes[j];
            let mut added = 0u64;
            let mut skipped = 0u64;
            for start in 0..=(messages_vec[j].len() - k) {
                let h = Self::range_hash(ref_h, ref_p, start, start + k);
                // compute cap-window hash (ensure we don't overflow bounds)
                let cap_end = std::cmp::min(messages_vec[j].len(), start + Self::cap_len());
                let cap_h = Self::range_hash(ref_h, ref_p, start, cap_end);
                let key = (h, cap_h);
                if seen.contains(&key) {
                    skipped += 1;
                } else {
                    seen.insert(key);
                    let bucket = table.entry(h).or_insert_with(|| Bucket::new());
                    bucket.push(Entry { cap_hash: cap_h, msg_idx: j, start });
                    added += 1;
                }
            }
            crate::instrumentation::add_kmers(added);
            crate::instrumentation::add_table_build_ns(skipped); // repurpose counter to record skipped (cheap)
        }
    }

    fn extend_candidate_capped(
        bytes: &[u8],
        prev_bytes: &[u8],
        cursor: usize,
        ref_start: usize,
        initial_k: usize,
    ) -> usize {
        let mut match_len = initial_k;
        let ext_t0 = std::time::Instant::now();

        while match_len < Self::cap_len()
            && cursor + match_len < bytes.len()
            && ref_start + match_len < prev_bytes.len()
            && bytes[cursor + match_len] == prev_bytes[ref_start + match_len]
        {
            match_len += 1;
            crate::instrumentation::add_chars(1);
        }

        let ext_dur = ext_t0.elapsed().as_nanos() as u64;
        crate::instrumentation::add_extension_ns(ext_dur);
        match_len
    }

    // Full extension using binary search and prefix hashes (like HashedGreedyBinary).
    fn extend_candidate_full(
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
        let t0 = std::time::Instant::now();

        while low < high {
            let mid = (low + high + 1) / 2;
            let h1 = Self::range_hash(&pref_cur.0, &pref_cur.1, cursor, cursor + mid);
            let h2 = Self::range_hash(&pref_prev.0, &pref_prev.1, ref_start, ref_start + mid);
            if h1 == h2 {
                low = mid;
            } else {
                high = mid - 1;
            }
        }

        // verification and char counting
        let matched = low;
        if matched > initial_k {
            let recovered = (matched - initial_k) as u64;
            crate::instrumentation::add_chars(recovered);
            crate::instrumentation::add_winner_extension(1);
            crate::instrumentation::add_winner_chars_recovered(recovered);
        }
        let dur = t0.elapsed().as_nanos() as u64;
        crate::instrumentation::add_extension_ns(dur);
        matched
    }

    pub fn with_config(config: &GreedySubstringConfig, _messages: &[&str]) -> CappedHashedGreedy {
        // no extra collections imported here

        let messages_vec: Vec<String> = _messages.iter().map(|s| s.to_string()).collect();
        let mut inner: Vec<Vec<Segment>> = Vec::with_capacity(messages_vec.len());

        let base: u64 = 257;
        let prefixes: Vec<(Vec<u64>, Vec<u64>)> = messages_vec
            .iter()
            .map(|m| Self::prefix_hashes(m.as_bytes(), base))
            .collect();

        let k = config.min_match_len;
        let total_kmers: usize = if k > 0 {
            messages_vec
                .iter()
                .map(|m| if m.len() >= k { m.len() - k + 1 } else { 0 })
                .sum()
        } else {
            0
        };
        // HashMap: k-mer hash -> small inline bucket of (cap_hash,msg_idx,start)
        let mut table: HashMap<u64, Bucket> = HashMap::with_capacity((total_kmers / 2).max(16));
        // global seen set of (kmer,cap) pairs to deduplicate quickly
        let mut seen: HashSet<(u64,u64)> = HashSet::with_capacity((total_kmers / 2).max(16));

        for i in 0..messages_vec.len() {
            let msg = &messages_vec[i];
            let bytes = msg.as_bytes();

            if k > 0 && i > 0 {
                use std::time::Instant;
                let t0 = Instant::now();
                let j = i - 1;
                // insert_kmers needs to know about cap-length hashing now
                Self::insert_kmers_into_table(&mut table, &mut seen, &messages_vec, &prefixes, j, k);
                let dur = t0.elapsed().as_nanos() as u64;
                crate::instrumentation::add_table_build_ns(dur);
            }

            let mut cursor = 0usize;
            let mut segs = Vec::new();

            while cursor < bytes.len() {
                let mut best_match: Option<(usize, usize, usize)> = None; // (len, msg_idx, ref_start)

                if bytes.len() >= cursor + k && k > 0 {
                    let (cur_h, cur_p) = &prefixes[i];
                    let kmer_hash = Self::range_hash(cur_h, cur_p, cursor, cursor + k);
                    crate::instrumentation::add_lookup(1);
                    // Iterate over range of entries with this kmer_hash
                    let mut seen = 0usize;
                    let cap_len = Self::cap_len();
                    let ncap = Self::ncap();
                    let cap_end_cur = std::cmp::min(bytes.len(), cursor + cap_len);
                    let cap_hash_cur = Self::range_hash(&cur_h, &cur_p, cursor, cap_end_cur);
                    if let Some(bucket) = table.get(&kmer_hash) {
                        for e in bucket.iter() {
                            if seen >= ncap { break; }
                            let midx = e.msg_idx;
                            let ref_start = e.start;
                            if midx >= i { continue; }
                            if e.cap_hash != cap_hash_cur { seen += 1; continue; }
                            crate::instrumentation::add_candidates(1);
                            let prev = &messages_vec[midx];
                            let prev_bytes = prev.as_bytes();
                            let match_len = Self::extend_candidate_capped(bytes, prev_bytes, cursor, ref_start, k);
                            if best_match.is_none() || match_len > best_match.unwrap().0 {
                                best_match = Some((match_len, midx, ref_start));
                            }
                            seen += 1;
                        }
                    }
                }

                if let Some((match_len, midx, ref_start)) = best_match {
                    // Winner-local full extension: extend the chosen candidate
                    // fully (via binary search) before emitting the reference.
                    let full_len = Self::extend_candidate_full(
                        &prefixes[i],
                        &prefixes[midx],
                        cursor,
                        ref_start,
                        match_len,
                    );
                    segs.push(Segment::Reference {
                        message_idx: midx,
                        start: ref_start,
                        len: full_len,
                    });
                    cursor += full_len;
                } else {
                    let mut literal_end = cursor + 1;
                    while literal_end < bytes.len() {
                        let mut found_match = false;
                        if k > 0 {
                            let (cur_h, cur_p) = &prefixes[i];
                            if bytes.len() >= literal_end + k {
                                let kmer_hash2 = Self::range_hash(cur_h, cur_p, literal_end, literal_end + k);
                                if table.contains_key(&kmer_hash2) {
                                    found_match = true;
                                }
                            }
                        }
                        if found_match { break; }
                        literal_end += 1;
                    }
                    segs.push(Segment::Literal(msg[cursor..literal_end].to_string()));
                    cursor = literal_end;
                }
            }

            inner.push(segs);
        }

        // Post-process: coalesce consecutive references that point to consecutive
        // locations in the same message. We do not re-extend here because the
        // chosen winners were fully extended during the main pass.
        for (msg_idx, segs) in inner.iter_mut().enumerate() {
            let mut out: Vec<Segment> = Vec::with_capacity(segs.len());
            let mut i = 0usize;
            // cursor in current message while rebuilding segments
            let mut cur_cursor = 0usize;
            while i < segs.len() {
                match &segs[i] {
                    Segment::Reference { message_idx, start, len } => {
                        let mut cur_msg = *message_idx;
                        let mut cur_start = *start;
                        let mut cur_len = *len;
                        i += 1;
                        while i < segs.len() {
                            if let Segment::Reference { message_idx: m2, start: s2, len: l2 } = &segs[i] {
                                if *m2 == cur_msg && *s2 == cur_start + cur_len {
                                    cur_len += *l2;
                                    i += 1;
                                    continue;
                                }
                            }
                            break;
                        }
                        out.push(Segment::Reference { message_idx: cur_msg, start: cur_start, len: cur_len });
                        cur_cursor += cur_len;
                    }
                    Segment::Literal(l) => {
                        out.push(Segment::Literal(l.clone()));
                        cur_cursor += l.len();
                        i += 1;
                    }
                }
            }
            *segs = out;
        }

        CappedHashedGreedy { inner, messages: messages_vec, config: config.clone() }
    }

    fn segments(&self) -> Vec<Vec<Segment>> {
        self.inner.clone()
    }

    fn render_with<F>(&self, mut _replacer: F) -> Vec<String>
    where
        F: FnMut(usize, usize, usize, &str) -> String,
    {
        let mut out = Vec::with_capacity(self.inner.len());
        for segs in self.inner.iter() {
            let mut s = String::new();
            for seg in segs {
                match seg {
                    Segment::Literal(l) => s.push_str(l),
                    Segment::Reference { message_idx, start, len } => {
                        let ref_text = &self.messages[*message_idx][*start..(*start + *len)];
                        let replaced = _replacer(*message_idx, *start, *len, ref_text);
                        s.push_str(&replaced);
                    }
                }
            }
            out.push(s);
        }
        out
    }
}

impl CopyForward for CappedHashedGreedy {
    fn from_messages(messages: &[&str]) -> CappedHashedGreedy {
        CappedHashedGreedy::with_config(&GreedySubstringConfig::default(), messages)
    }

    fn segments(&self) -> Vec<Vec<Segment>> {
        self.segments()
    }

    fn render_with<F>(&self, replacer: F) -> Vec<String>
    where
        F: FnMut(usize, usize, usize, &str) -> String,
    {
        self.render_with(replacer)
    }
}
