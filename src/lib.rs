#![allow(unsafe_op_in_unsafe_fn)]
pub mod fixture;
pub mod python_bindings;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    Literal(String),
    /// Reference to a substring of a previous message: (message_idx, start, len)
    Reference { message_idx: usize, start: usize, len: usize },
}

/// Trait describing a copy-forwarding algorithm implementation.
pub trait CopyForward {
    /// Construct the algorithm-specific representation from messages.
    fn from_messages(messages: &[&str]) -> Self
    where
        Self: Sized;

    /// Return a cloned `Vec<Vec<Segment>>` representation of the internal model.
    fn segments(&self) -> Vec<Vec<Segment>>;

    /// Render each message into a `String`, calling `replacer` for every
    /// `Reference` segment with `(message_idx, start, len, referenced_text)`.
    fn render_with<F>(&self, replacer: F) -> Vec<String>
    where
        F: FnMut(usize, usize, usize, &str) -> String;

    /// Convenience to render with a static replacement string for every
    /// `Reference` segment.
    fn render_with_static(&self, replacement: &str) -> Vec<String>
    where
        Self: Sized,
    {
        self.render_with(|_, _, _, _| replacement.to_string())
    }
}

/// Implementation that uses a greedy substring matching strategy.
pub struct GreedySubstring {
    inner: Vec<Vec<Segment>>,
    messages: Vec<String>,
    pub config: GreedySubstringConfig,
}

/// Configuration for `GreedySubstring` algorithm.
#[derive(Debug, Clone)]
pub struct GreedySubstringConfig {
    pub min_match_len: usize,
    /// lookback window: only consider previous `lookback` messages
    /// when matching. `None` means consider all previous messages.
    pub lookback: Option<usize>,
}

impl Default for GreedySubstringConfig {
    fn default() -> Self {
        GreedySubstringConfig { min_match_len: 4, lookback: None }
    }
}

impl GreedySubstring {
    /// Build using an explicit configuration.
    pub fn with_config(config: &GreedySubstringConfig, _messages: &[&str]) -> GreedySubstring {
        let messages_vec: Vec<String> = _messages.iter().map(|s| s.to_string()).collect();
        let mut inner: Vec<Vec<Segment>> = Vec::with_capacity(messages_vec.len());

        for i in 0..messages_vec.len() {
            let msg = &messages_vec[i];

            // Use a greedy approach: scan through message and find the longest match at each position
            let mut cursor = 0usize;
            let mut segs = Vec::new();

            while cursor < msg.len() {
                let mut best_match: Option<(usize, usize, usize, usize)> = None;
                // (match_len, message_idx, ref_start, msg_start)

                // Look for the longest match starting at cursor position
                for (j, prev_msg) in messages_vec.iter().enumerate().take(i) {
                    if prev_msg.is_empty() { continue; }
                    if let Some(lookback) = config.lookback {
                        if i.saturating_sub(j) > lookback { continue; }
                    }

                    // Check each position in previous message
                    for ref_start in 0..prev_msg.len() {
                        if cursor >= msg.len() || ref_start >= prev_msg.len() { continue; }
                        
                        // Check if characters match
                        if msg.as_bytes()[cursor] != prev_msg.as_bytes()[ref_start] { continue; }
                        
                        // Extend the match as far as possible
                        let mut match_len = 0;
                        while cursor + match_len < msg.len() 
                            && ref_start + match_len < prev_msg.len()
                            && msg.as_bytes()[cursor + match_len] == prev_msg.as_bytes()[ref_start + match_len] {
                            match_len += 1;
                        }
                        
                        // Keep the longest match for this position
                        if match_len >= config.min_match_len {
                            if best_match.is_none() || match_len > best_match.unwrap().0 {
                                best_match = Some((match_len, j, ref_start, cursor));
                            }
                        }
                    }
                }

                if let Some((match_len, midx, ref_start, _)) = best_match {
                    // Found a match, add it as a reference
                    segs.push(Segment::Reference { message_idx: midx, start: ref_start, len: match_len });
                    cursor += match_len;
                } else {
                    // No match found, find the next potential match or end of string
                    let mut literal_end = cursor + 1;
                    
                    // Extend literal until we find a potential match or reach end
                    while literal_end < msg.len() {
                        let mut found_match = false;
                        for (j, prev_msg) in messages_vec.iter().enumerate().take(i) {
                            if prev_msg.is_empty() { continue; }
                            if let Some(lookback) = config.lookback {
                                if i.saturating_sub(j) > lookback { continue; }
                            }

                            for ref_start in 0..prev_msg.len() {
                                if literal_end >= msg.len() || ref_start >= prev_msg.len() { continue; }
                                
                                // Check for potential match
                                if msg.as_bytes()[literal_end] == prev_msg.as_bytes()[ref_start] {
                                    // Extend to see if it meets minimum length
                                    let mut potential_len = 0;
                                    while literal_end + potential_len < msg.len() 
                                        && ref_start + potential_len < prev_msg.len()
                                        && msg.as_bytes()[literal_end + potential_len] == prev_msg.as_bytes()[ref_start + potential_len] {
                                        potential_len += 1;
                                    }
                                    
                                    if potential_len >= config.min_match_len {
                                        found_match = true;
                                        break;
                                    }
                                }
                            }
                            if found_match { break; }
                        }
                        
                        if found_match { break; }
                        literal_end += 1;
                    }
                    
                    // Add literal segment
                    segs.push(Segment::Literal(msg[cursor..literal_end].to_string()));
                    cursor = literal_end;
                }
            }

            inner.push(segs);
        }

        GreedySubstring { inner, messages: messages_vec, config: config.clone() }
    }

    fn segments(&self) -> Vec<Vec<Segment>> {
        self.inner.clone()
    }

    fn render_with<F>(&self, mut _replacer: F) -> Vec<String>
    where
        F: FnMut(usize, usize, usize, &str) -> String,
    {
        // Resolve a reference by slicing the original message text.
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

    /// Convenience constructor using default configuration.
    pub fn from_messages(messages: &[&str]) -> GreedySubstring {
        GreedySubstring::with_config(&GreedySubstringConfig::default(), messages)
    }
}

impl CopyForward for GreedySubstring {
    fn from_messages(messages: &[&str]) -> GreedySubstring {
        GreedySubstring::from_messages(messages)
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

/// Alternative implementation using rolling (u64) hashes for k-mer lookup
pub struct HashedGreedy {
    inner: Vec<Vec<Segment>>,
    messages: Vec<String>,
    pub config: GreedySubstringConfig,
}

impl HashedGreedy {
    /// Build using an explicit configuration. This implementation builds
    /// a hash table of all k-mers (k = `min_match_len`) from previous
    /// messages and uses it to find candidate match starts quickly.
    pub fn with_config(config: &GreedySubstringConfig, _messages: &[&str]) -> HashedGreedy {
        use std::collections::HashMap;

        let messages_vec: Vec<String> = _messages.iter().map(|s| s.to_string()).collect();
        let mut inner: Vec<Vec<Segment>> = Vec::with_capacity(messages_vec.len());

        // Helper: compute rolling prefix hashes and powers (base 257) for a byte string
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

        // Hash substring [l, r) using prefix info
        fn range_hash(h: &Vec<u64>, p: &Vec<u64>, l: usize, r: usize) -> u64 {
            h[r].wrapping_sub(h[l].wrapping_mul(p[r - l]))
        }

        let base: u64 = 257;

        // Precompute prefix hashes for all messages so substring hashes are O(1)
        let prefixes: Vec<(Vec<u64>, Vec<u64>)> = messages_vec.iter().map(|m| prefix_hashes(m.as_bytes(), base)).collect();

        // Prepare incremental k-mer table to avoid rebuilding per message.
        let k = config.min_match_len;
        // Compute total approximate number of k-mers to reserve capacity
        let total_kmers: usize = if k > 0 {
            messages_vec.iter().map(|m| if m.len() >= k { m.len() - k + 1 } else { 0 }).sum()
        } else { 0 };
        let mut table: HashMap<u64, Vec<(usize, usize)>> = HashMap::with_capacity((total_kmers / 2).max(16));
        // We'll add k-mers incrementally: before processing message i we insert k-mers from message i-1.
        for i in 0..messages_vec.len() {
            let msg = &messages_vec[i];
            let bytes = msg.as_bytes();

            // Insert k-mers from previous message (i-1) so table always contains all prior k-mers
            if k > 0 && i > 0 {
                let j = i - 1;
                if messages_vec[j].len() >= k {
                    let (ref_h, ref_p) = &prefixes[j];
                    for start in 0..=(messages_vec[j].len() - k) {
                        let h = range_hash(ref_h, ref_p, start, start + k);
                        table.entry(h).or_default().push((j, start));
                    }
                }
            }

            let mut cursor = 0usize;
            let mut segs = Vec::new();

            while cursor < bytes.len() {
                let mut best_match: Option<(usize, usize, usize)> = None; // (len, msg_idx, ref_start)

                if bytes.len() >= cursor + k && k > 0 {
                    // compute hash of current k-mer
                    let (cur_h, cur_p) = &prefixes[i];
                    let key = range_hash(cur_h, cur_p, cursor, cursor + k);
                    if let Some(cands) = table.get(&key) {
                        // Extend each candidate to find maximal match
                        for &(midx, ref_start) in cands.iter() {
                            let prev = &messages_vec[midx];
                            let prev_bytes = prev.as_bytes();
                            // already matched k bytes
                            let mut match_len = k;
                            while cursor + match_len < bytes.len() && ref_start + match_len < prev_bytes.len()
                                && bytes[cursor + match_len] == prev_bytes[ref_start + match_len] {
                                match_len += 1;
                            }
                            if best_match.is_none() || match_len > best_match.unwrap().0 {
                                best_match = Some((match_len, midx, ref_start));
                            }
                        }
                    }
                }

                if let Some((match_len, midx, ref_start)) = best_match {
                    segs.push(Segment::Reference { message_idx: midx, start: ref_start, len: match_len });
                    cursor += match_len;
                } else {
                    // fallback: grow literal until next potential k-mer match
                    let mut literal_end = cursor + 1;
                    while literal_end < bytes.len() {
                        if bytes.len() >= literal_end + k && k > 0 {
                            let (cur_h, cur_p) = &prefixes[i];
                            let key = range_hash(cur_h, cur_p, literal_end, literal_end + k);
                            if table.contains_key(&key) { break; }
                        }
                        literal_end += 1;
                    }
                    segs.push(Segment::Literal(msg[cursor..literal_end].to_string()));
                    cursor = literal_end;
                }
            }

            inner.push(segs);
        }

        HashedGreedy { inner, messages: messages_vec, config: config.clone() }
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

    /// Convenience constructor using default configuration.
    pub fn from_messages(messages: &[&str]) -> HashedGreedy {
        HashedGreedy::with_config(&GreedySubstringConfig::default(), messages)
    }
}

impl CopyForward for HashedGreedy {
    fn from_messages(messages: &[&str]) -> HashedGreedy {
        HashedGreedy::from_messages(messages)
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


#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::generate_thread;



    #[test]
    fn render_with_lambda_replaces_references() {
        let msgs = &["hello world", "hello world today"];
        let config = GreedySubstringConfig { min_match_len: 10, lookback: None };
        let cf = GreedySubstring::with_config(&config, msgs);

        let rendered = cf.render_with(|m_idx, start, len, referenced_text| {
            format!("<ref {m_idx}:{start}+{len}='{referenced_text}'>")
        });

        assert_eq!(rendered, vec!["hello world".to_string(), "<ref 0:0+11='hello world'> today".to_string()]);
    }


    #[test]
    fn fixture_thread_is_deduped_substantially() {
        // generate until total size >= ~25 KB
        let target_kb = 25usize;
        let mut n = 4usize;
        let mut msgs: Vec<String> = Vec::new();
        let mut orig = 0usize;
        while orig < target_kb * 1024 {
            msgs = generate_thread(12345, n, 5);
            orig = msgs.iter().map(|s| s.len()).sum();
            n *= 2;
            if n > 4096 { break; }
        }

        let refs: Vec<&str> = msgs.iter().map(|s| s.as_str()).collect();
        let cf = GreedySubstring::from_messages(&refs);
        let segs = cf.segments();
        let deduped: usize = segs.iter().flat_map(|v| v.iter()).map(|seg| match seg {
            Segment::Literal(s) => s.len(),
            Segment::Reference { .. } => 3,
        }).sum();

        if deduped as f64 > (orig as f64) * 0.1 {
            eprintln!("Dedup insufficient: orig={orig} bytes, deduped={deduped} bytes ({:.2}% )", deduped as f64 / orig as f64 * 100.0);
            eprintln!("First 5 original messages:");
            for (i, m) in msgs.iter().take(5).enumerate() {
                eprintln!("[{}] len={}: {}", i, m.len(), &m.chars().take(120).collect::<String>());
            }
            eprintln!("First 10 segments:");
            for (i, v) in segs.iter().enumerate().take(10) {
                eprint!("[{i}]: ");
                for seg in v {
                    match seg {
                        Segment::Literal(s) => eprint!("L(len={}) ", s.len()),
                        Segment::Reference { message_idx, start, len } => eprint!("R({message_idx}:{start}+{len} ) "),
                    }
                }
                eprintln!();
            }
            eprintln!("First 10 rendered messages:");
            for (i, m) in cf.render_with(|_, _, _, _| "...".to_string()).iter().take(10).enumerate() {
                eprintln!("[{i}] {m}");
            }
        }

        // expect at least 50% reduction for this first-pass algorithm
        assert!(deduped as f64 <= (orig as f64) * 0.5, "deduped={} orig={}", deduped, orig);
    }


    #[test]
    fn partial_overlaps_across_multiple_messages() {
        let msgs = &[
            "hello world everyone",
            "world peace and harmony",
            "hello world peace and joy for everyone"
        ];
        
        let config = GreedySubstringConfig { min_match_len: 10, lookback: None };
        let cf = GreedySubstring::with_config(&config, msgs);
        let segs = cf.segments();
        
        // Message 2 should reference parts of both previous messages
        assert!(segs[2].len() >= 2, "Should have multiple segments");
        
        let rendered = cf.render_with(|_, _, _, text| text.to_string());
        assert_eq!(rendered[2], "hello world peace and joy for everyone");
        
        // Should find "hello world" from msg[0] and possibly other long matches
        let has_long_match = segs[2].iter().any(|seg| match seg {
            Segment::Reference { len, .. } if *len >= 10 => true,
            _ => false,
        });
        assert!(has_long_match, "Should have at least one match of 10+ characters");
    }

    #[test]
    fn finds_longest_common_substrings() {
        let msgs = &[
            "The quick brown fox jumps over the lazy dog",
            "A quick brown fox is very fast",
            "The quick brown fox is amazing and the lazy dog sleeps"
        ];
        
        let config = GreedySubstringConfig { min_match_len: 10, lookback: None };
        let cf = GreedySubstring::with_config(&config, msgs);
        let segs = cf.segments();
        
        // Should prefer longer matches over shorter ones
        let longest_match = segs[2].iter().filter_map(|seg| match seg {
            Segment::Reference { len, .. } => Some(*len),
            _ => None,
        }).max().unwrap_or(0);
        
        assert!(longest_match >= 15, "Should find matches of 15+ characters, found {}", longest_match);
        
        // Should minimize number of segments by using longer matches
        assert!(segs[2].len() <= 5, "Should use few segments with long matches, got {}", segs[2].len());
    }

    #[test]
    fn handles_overlapping_substrings_efficiently() {
        let msgs = &[
            "programming is programming and more programming",
            "I love programming and programming languages",
            "programming and programming languages are great for programming"
        ];
        
        let config = GreedySubstringConfig { min_match_len: 11, lookback: None }; // "programming"
        let cf = GreedySubstring::with_config(&config, msgs);
        let segs = cf.segments();
        
        // Greedy algorithm should select non-overlapping matches efficiently
        let rendered = cf.render_with(|_, _, _, text| text.to_string());
        assert_eq!(rendered[2], "programming and programming languages are great for programming");
        
        // Should have multiple references to "programming" without overlaps
        let ref_count = segs[2].iter().filter(|seg| matches!(seg, Segment::Reference { .. })).count();
        assert!(ref_count >= 2, "Should find multiple non-overlapping references");
    }

    #[test]
    fn respects_minimum_match_length_realistically() {
        let msgs = &[
            "This is a short test message for our algorithm",
            "Another short test of the algorithm implementation",
            "This is a short test that validates our algorithm works correctly"
        ];
        
        // Test with realistic minimum match length
        let config = GreedySubstringConfig { min_match_len: 15, lookback: None };
        let cf = GreedySubstring::with_config(&config, msgs);
        let segs = cf.segments();
        
        // All references should meet minimum length
        for seg in segs[2].iter() {
            if let Segment::Reference { len, .. } = seg {
                assert!(*len >= 15, "Found reference shorter than min_match_len: {}", len);
            }
        }
        
        // Should still find meaningful matches
        let has_long_match = segs[2].iter().any(|seg| match seg {
            Segment::Reference { len, .. } => *len >= 15,
            _ => false,
        });
        assert!(has_long_match, "Should find at least one match >= 15 characters");
    }

    #[test]
    fn finds_substrings_from_middle_of_messages() {
        let msgs = &[
            "Hello everyone, the weather is absolutely wonderful today!",
            "I hope that the weather stays wonderful for the weekend",
            "Yes, the weather is wonderful and I love these sunny days"
        ];
        
        let config = GreedySubstringConfig { min_match_len: 12, lookback: None };
        let cf = GreedySubstring::with_config(&config, msgs);
        let segs = cf.segments();
        
        // Should find "the weather is wonderful" from middle of msg[0] and msg[1]
        let has_middle_match = segs[2].iter().any(|seg| match seg {
            Segment::Reference { message_idx, start, len, .. } => {
                *len >= 12 && *start > 0 && *message_idx < 2
            },
            _ => false,
        });
        assert!(has_middle_match, "Should find substring matches from middle of previous messages");
    }

    #[test]
    fn handles_multiple_references_to_same_substring() {
        let msgs = &[
            "artificial intelligence and machine learning",
            "machine learning algorithms use artificial intelligence",
            "artificial intelligence powers machine learning and machine learning improves artificial intelligence"
        ];
        
        let config = GreedySubstringConfig { min_match_len: 16, lookback: None };
        let cf = GreedySubstring::with_config(&config, msgs);
        let segs = cf.segments();
        
        // Should efficiently handle repeated long substrings
        let ref_segments: Vec<_> = segs[2].iter().filter_map(|seg| match seg {
            Segment::Reference { len, .. } => Some(*len),
            _ => None,
        }).collect();
        
        assert!(ref_segments.len() >= 2, "Should find multiple long references");
        assert!(ref_segments.iter().any(|&len| len >= 16), "Should find references >= 16 chars");
    }

    #[test]
    fn compression_with_realistic_conversation() {
        // Simulate realistic email/forum thread with nested quoting that breaks up original text
        let msgs = &[
            "Let's implement the new authentication system using JWT tokens for security",
            "> Let's implement the new authentication system using JWT tokens for security\nI agree, but what about token expiration policies?",
            ">> Let's implement the new authentication system using JWT tokens for security\n> I agree, but what about token expiration policies?\nGood point about token expiration policies. We should also consider secure storage",
            ">>> Let's implement the new authentication system using JWT tokens for security\n>> I agree, but what about token expiration policies?\n> Good point about token expiration policies. We should also consider secure storage\nAll these points about JWT tokens for security and token expiration policies are valid"
        ];
        
        let config = GreedySubstringConfig { min_match_len: 15, lookback: None };
        let cf = GreedySubstring::with_config(&config, msgs);
        let segs = cf.segments();
        
        
        // Calculate compression effectiveness
        let original_size: usize = msgs.iter().map(|s| s.len()).sum();
        let compressed_size: usize = segs.iter().flat_map(|v| v.iter()).map(|seg| match seg {
            Segment::Literal(s) => s.len(),
            Segment::Reference { .. } => 10, // Approximate reference cost
        }).sum();
        
        let compression_ratio = compressed_size as f64 / original_size as f64;
        // With realistic nested quoting pattern, expect significant compression (50-70% savings)
        assert!(compression_ratio < 0.5, "Should achieve major compression with nested quotes: {:.2}%", compression_ratio * 100.0);
        println!("Achieved {:.1}% compression ({:.1}% of original size)", (1.0 - compression_ratio) * 100.0, compression_ratio * 100.0);
        
        // Should find meaningful phrase matches like "JWT tokens" across messages
        let has_meaningful_matches = segs.iter().skip(1).any(|msg_segs| {
            msg_segs.iter().any(|seg| match seg {
                Segment::Reference { len, .. } => *len >= 12,
                _ => false,
            })
        });
        assert!(has_meaningful_matches, "Should find meaningful phrase matches across conversation");
    }

    #[test]
    fn hashed_greedy_matches_greedy_on_small_inputs() {
        let msgs = &[
            "hello world everyone",
            "world peace and harmony",
            "hello world peace and joy for everyone"
        ];

        let refs: Vec<&str> = msgs.iter().copied().collect();
        let config = GreedySubstringConfig { min_match_len: 10, lookback: None };
        let g = GreedySubstring::with_config(&config, &refs);
        let h = HashedGreedy::with_config(&config, &refs);

        let rendered_g = g.render_with(|_, _, _, text| text.to_string());
        let rendered_h = h.render_with(|_, _, _, text| text.to_string());

        assert_eq!(rendered_g, rendered_h, "HashedGreedy should render same as GreedySubstring for sample input");
    }
}
