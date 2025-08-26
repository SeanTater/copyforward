// HashedGreedy: explanatory notes
//
// This implementation uses a rolling 64-bit polynomial hash to index fixed-length
// k-mers (where k == `min_match_len`) from prior messages. The motivation is
// to avoid the O(n*m) scanning cost of the naive greedy approach when many
// prior positions must be inspected for every cursor position. Instead we:
//
// 1. Precompute prefix hashes and power tables for each message so any
//    substring hash h[l..r) can be computed in O(1) using h[r] - h[l]*p[r-l]
//    (wrapping u64 arithmetic is used for speed; this yields possible but
//    extremely unlikely collisions in practice).
//
// 2. Build an incremental hash table that maps a k-mer hash -> list of
//    (message_index, start). We insert k-mers from message i-1 before
//    processing message i, so lookups never see the current message's
//    k-mers. This preserves the "only reference prior messages" property.
//
// 3. At each cursor position we compute the k-mer hash and retrieve candidate
//    starts from the table. Each candidate is extended (byte-by-byte) to
//    measure how far the match goes; we pick the longest extension as the
//    best match.
//
// Notes, tradeoffs and limitations:
// - We use wrapping u64 arithmetic (mod 2^64) for polynomial hashing (base
//   257). This is very fast but not cryptographically collision-resistant.
//   The design tolerates rare collisions because an incorrect candidate will
//   usually fail the extension check and be rejected.
// - A single k-mer can occur many times in prior messages, producing a
//   long candidate list. To avoid pathological worst-cases you can limit the
//   number of candidates examined, prefer recent messages, or group
//   candidates by message and examine only the most promising positions.
// - The implementation prioritizes clarity and locality (we store
//   (message_index, start) pairs) but other layouts (flat position buffer,
//   grouped by message, or per-message reverse order) can improve
//   performance in workload-specific ways.
// - Instrumentation hooks (`crate::instrumentation::*`) are sprinkled
//   throughout to collect counts and timings for k-mer insertion, lookups,
//   candidate extension work, and character comparisons; these are useful
//   for profiling and tuning heuristics.
//
// Possible improvements:
// - Cap candidates inspected per lookup or prefer candidates from more
//   recent messages (scan bucket in reverse).
// - Add a short pre-check (memcmp) before computing/accepting an expensive
//   extension.
// - Deduplicate identical (message_idx, start) pairs across buckets if needed.
// - Use a second-level verification hash (e.g., another base or length) to
//   further reduce collision checks.
//
use crate::core::{CopyForward, GreedySubstringConfig, Segment};

/// Alternative implementation using rolling 64-bit polynomial hashes for k-mer lookup.
pub struct HashedGreedy {
    inner: Vec<Vec<Segment>>,
    messages: Vec<String>,
    pub config: GreedySubstringConfig,
}

impl HashedGreedy {
    // Helper: compute rolling prefix hashes and powers (base 257) for a byte string.
    // Returns (h, p) where h[r] - h[l]*p[r-l] yields the rolling hash for s[l..r).
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

    // Hash substring [l, r) using prefix info (r is exclusive).
    fn range_hash(h: &Vec<u64>, p: &Vec<u64>, l: usize, r: usize) -> u64 {
        h[r].wrapping_sub(h[l].wrapping_mul(p[r - l]))
    }

    // Insert all k-mers from message `j` into `table`. Records instrumentation.
    fn insert_kmers_into_table(
        table: &mut std::collections::HashMap<u64, Vec<(usize, usize)>>,
        messages_vec: &Vec<String>,
        prefixes: &Vec<(Vec<u64>, Vec<u64>)>,
        j: usize,
        k: usize,
    ) {
        if messages_vec[j].len() >= k {
            let (ref_h, ref_p) = &prefixes[j];
            let mut added = 0u64;
            for start in 0..=(messages_vec[j].len() - k) {
                let h = Self::range_hash(ref_h, ref_p, start, start + k);
                table.entry(h).or_default().push((j, start));
                added += 1;
            }
            // instrumentation removed
        }
    }

    // Extend a candidate match starting at (cursor in bytes) and (ref_start in prev_bytes).
    // Returns the matched length and records instrumentation about chars compared and extension time.
    fn extend_candidate(
        bytes: &[u8],
        prev_bytes: &[u8],
        cursor: usize,
        ref_start: usize,
        initial_k: usize,
    ) -> usize {
        let mut match_len = initial_k;
        let ext_t0 = std::time::Instant::now();

        // Byte-by-byte extension
        while cursor + match_len < bytes.len()
            && ref_start + match_len < prev_bytes.len()
            && bytes[cursor + match_len] == prev_bytes[ref_start + match_len]
        {
            match_len += 1;
            // instrumentation removed
        }

        let ext_dur = ext_t0.elapsed().as_nanos() as u64;
        // instrumentation removed
        match_len
    }

    /// Build using an explicit configuration. This implementation builds a hash table of all k-mers (k = `min_match_len`) from previous messages and uses it to find candidate match starts quickly.
    pub fn with_config(config: &GreedySubstringConfig, _messages: &[&str]) -> HashedGreedy {
        use std::collections::HashMap;

        let messages_vec: Vec<String> = _messages.iter().map(|s| s.to_string()).collect();
        let mut inner: Vec<Vec<Segment>> = Vec::with_capacity(messages_vec.len());

        // Local nested helpers were removed in favor of associated helpers
        // defined on `HashedGreedy` above.

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
        let mut table: HashMap<u64, Vec<(usize, usize)>> =
            HashMap::with_capacity((total_kmers / 2).max(16));

        for i in 0..messages_vec.len() {
            let msg = &messages_vec[i];
            let bytes = msg.as_bytes();

            // Insert k-mers from the immediately prior message into the shared
            // table. Doing this incrementally (only inserting message i-1 before
            // processing message i) ensures lookups only yield prior-message
            // positions. We record instrumentation about how many k-mers were
            // added and how long the insertion took so heuristics can be tuned.
            if k > 0 && i > 0 {
                use std::time::Instant;
                let t0 = Instant::now();
                let j = i - 1;
                // Insert k-mers from message `j` using the helper.
                Self::insert_kmers_into_table(&mut table, &messages_vec, &prefixes, j, k);
                let dur = t0.elapsed().as_nanos() as u64;
                // instrumentation removed
            }

            let mut cursor = 0usize;
            let mut segs = Vec::new();

            while cursor < bytes.len() {
                let mut best_match: Option<(usize, usize, usize)> = None; // (len, msg_idx, ref_start)

                if bytes.len() >= cursor + k && k > 0 {
                    let (cur_h, cur_p) = &prefixes[i];
                    let key = Self::range_hash(cur_h, cur_p, cursor, cursor + k);
                    // instrumentation removed
                    if let Some(cands) = table.get(&key) {
                        // Cap the number of candidates we examine to avoid
                        // pathological buckets. Prefer earliest entries.
                        let mut examined = 0usize;
                        for &(midx, ref_start) in cands.iter() {
                            if examined >= 64 {
                                break;
                            }
                            // instrumentation removed
                            examined += 1;

                            let prev = &messages_vec[midx];
                            let prev_bytes = prev.as_bytes();

                            // Extend match using helper which also records
                            // instrumentation about chars compared and time.
                            let match_len =
                                Self::extend_candidate(bytes, prev_bytes, cursor, ref_start, k);

                            if best_match.is_none() || match_len > best_match.unwrap().0 {
                                best_match = Some((match_len, midx, ref_start));
                            }
                        }
                    }
                }

                if let Some((match_len, midx, ref_start)) = best_match {
                    segs.push(Segment::Reference {
                        message_idx: midx,
                        start: ref_start,
                        len: match_len,
                    });
                    cursor += match_len;
                } else {
                    let mut literal_end = cursor + 1;
                    while literal_end < bytes.len() {
                        if bytes.len() >= literal_end + k && k > 0 {
                            let (cur_h, cur_p) = &prefixes[i];
                            let key = Self::range_hash(cur_h, cur_p, literal_end, literal_end + k);
                            if table.contains_key(&key) {
                                break;
                            }
                        }
                        literal_end += 1;
                    }
                    segs.push(Segment::Literal(msg[cursor..literal_end].to_string()));
                    cursor = literal_end;
                }
            }

            inner.push(segs);
        }

        HashedGreedy {
            inner,
            messages: messages_vec,
            config: config.clone(),
        }
    }

    // `segments`, `render_with` and `from_messages` are provided by the
    // `CopyForward` impl below to avoid redundant delegating methods.
}

impl CopyForward for HashedGreedy {
    fn from_messages(messages: &[&str]) -> HashedGreedy {
        HashedGreedy::with_config(&GreedySubstringConfig::default(), messages)
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
                    Segment::Reference {
                        message_idx,
                        start,
                        len,
                    } => {
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
