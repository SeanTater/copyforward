use crate::core::{CopyForward, GreedySubstringConfig, Segment};

/// Implementation that uses a greedy substring matching strategy.
pub struct GreedySubstring {
    inner: Vec<Vec<Segment>>,
    messages: Vec<String>,
    pub config: GreedySubstringConfig,
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
                    if prev_msg.is_empty() {
                        continue;
                    }
                    if let Some(lookback) = config.lookback {
                        if i.saturating_sub(j) > lookback {
                            continue;
                        }
                    }

                    // Check each position in previous message
                    for ref_start in 0..prev_msg.len() {
                        if cursor >= msg.len() || ref_start >= prev_msg.len() {
                            continue;
                        }

                        // Check if characters match
                        if msg.as_bytes()[cursor] != prev_msg.as_bytes()[ref_start] {
                            continue;
                        }

                        // Extend the match as far as possible
                        let mut match_len = 0;
                        let ext_t0 = std::time::Instant::now();
                        while cursor + match_len < msg.len()
                            && ref_start + match_len < prev_msg.len()
                            && msg.as_bytes()[cursor + match_len]
                                == prev_msg.as_bytes()[ref_start + match_len]
                        {
                            match_len += 1;
                            // instrumentation removed
                        }
                        let ext_dur = ext_t0.elapsed().as_nanos() as u64;
                        // instrumentation removed

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
                    segs.push(Segment::Reference {
                        message_idx: midx,
                        start: ref_start,
                        len: match_len,
                    });
                    cursor += match_len;
                } else {
                    // No match found at the cursor. We now grow a literal segment
                    // until we find the next position that could start a k-mer
                    // match (or reach the end of the message). The literal
                    // growth scans forward until the first byte that could be the
                    // beginning of a k-mer in any prior message.
                    //
                    // This loop is intentionally conservative: it seeks the
                    // earliest next candidate so we don't skip potential
                    // references. Optimizations include scanning using a small
                    // bloom filter of existing starting bytes or pre-indexing
                    // single-byte occurrence sets, but the simple scan is
                    // usually adequate and keeps the implementation straightforward.
                    let mut literal_end = cursor + 1;

                    // Extend literal until we find a potential match or reach end
                    while literal_end < msg.len() {
                        let mut found_match = false;
                        for (j, prev_msg) in messages_vec.iter().enumerate().take(i) {
                            if prev_msg.is_empty() {
                                continue;
                            }
                            if let Some(lookback) = config.lookback {
                                if i.saturating_sub(j) > lookback {
                                    continue;
                                }
                            }

                            for ref_start in 0..prev_msg.len() {
                                if literal_end >= msg.len() || ref_start >= prev_msg.len() {
                                    continue;
                                }

                                // Quick byte equality test to find a possible
                                // match start. If the first bytes match, we then
                                // try to extend to see whether we reach the
                                // `min_match_len` threshold.
                                if msg.as_bytes()[literal_end] == prev_msg.as_bytes()[ref_start] {
                                    // Extend to see if it meets minimum length
                                    let mut potential_len = 0;
                                    while literal_end + potential_len < msg.len()
                                        && ref_start + potential_len < prev_msg.len()
                                        && msg.as_bytes()[literal_end + potential_len]
                                            == prev_msg.as_bytes()[ref_start + potential_len]
                                    {
                                        potential_len += 1;
                                    }

                                    if potential_len >= config.min_match_len {
                                        found_match = true;
                                        break;
                                    }
                                }
                            }
                            if found_match {
                                break;
                            }
                        }

                        if found_match {
                            break;
                        }
                        literal_end += 1;
                    }

                    // Add literal segment
                    segs.push(Segment::Literal(msg[cursor..literal_end].to_string()));
                    cursor = literal_end;
                }
            }

            inner.push(segs);
        }

        GreedySubstring {
            inner,
            messages: messages_vec,
            config: config.clone(),
        }
    }

    // `segments`, `render_with` and `from_messages` are implemented on the
    // `CopyForward` trait below to avoid duplicate delegating methods.
}

impl CopyForward for GreedySubstring {
    fn from_messages(messages: &[&str]) -> GreedySubstring {
        // Construct using the default configuration.
        GreedySubstring::with_config(&GreedySubstringConfig::default(), messages)
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
