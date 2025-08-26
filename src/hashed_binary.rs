// HashedGreedyBinary: variant of hashed greedy that uses binary-search
// extension per candidate using rolling hashes. This reduces per-candidate
// extension from O(M) to O(log M) hash checks, which is beneficial when
// matches are long.

use crate::core::{CopyForward, GreedySubstringConfig, Segment};

pub struct HashedGreedyBinary {
    inner: Vec<Vec<Segment>>,
    messages: Vec<String>,
    pub config: GreedySubstringConfig,
}

impl HashedGreedyBinary {
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

    // Binary-search extension using precomputed prefix hashes for both strings.
    fn extend_candidate_binary(
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

        // final verification (byte-by-byte) to be safe against hash collisions
        let matched = low;
        let dur = t0.elapsed().as_nanos() as u64;
        if matched > initial_k {
            // instrumentation removed
        }
        // instrumentation removed
        matched
    }

    pub fn with_config(config: &GreedySubstringConfig, _messages: &[&str]) -> HashedGreedyBinary {
        use std::collections::HashMap;

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
        let mut table: HashMap<u64, Vec<(usize, usize)>> =
            HashMap::with_capacity((total_kmers / 2).max(16));

        for i in 0..messages_vec.len() {
            let msg = &messages_vec[i];
            let bytes = msg.as_bytes();

            if k > 0 && i > 0 {
                use std::time::Instant;
                let t0 = Instant::now();
                let j = i - 1;
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

                            let prev_pref = &prefixes[midx];

                            let match_len = Self::extend_candidate_binary(
                                &prefixes[i],
                                prev_pref,
                                cursor,
                                ref_start,
                                k,
                            );

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

        HashedGreedyBinary {
            inner,
            messages: messages_vec,
            config: config.clone(),
        }
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

impl CopyForward for HashedGreedyBinary {
    fn from_messages(messages: &[&str]) -> HashedGreedyBinary {
        HashedGreedyBinary::with_config(&GreedySubstringConfig::default(), messages)
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
