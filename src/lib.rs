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

/// Example implementation that uses a longest-match strategy.
pub struct LongestMatch {
    inner: Vec<Vec<Segment>>,
    messages: Vec<String>,
}

impl CopyForward for LongestMatch {
    fn from_messages(_messages: &[&str]) -> LongestMatch {
        let messages_vec: Vec<String> = _messages.iter().map(|s| s.to_string()).collect();
        let mut inner: Vec<Vec<Segment>> = Vec::with_capacity(messages_vec.len());

        for i in 0..messages_vec.len() {
            let msg = &messages_vec[i];

            // Fallback: find multiple non-overlapping occurrences of previous
            // messages inside `msg` using a simple weighted-interval DP. We
            // enumerate all matches (start, end, weight=len) and pick the set
            // that maximizes total covered bytes.
            let mut matches: Vec<(usize, usize, usize, usize)> = Vec::new();
            // (start, end, len, message_idx)
            for j in 0..i {
                let cand = &messages_vec[j];
                if cand.is_empty() { continue; }
                let mut start_pos = 0usize;
                while let Some(pos) = msg[start_pos..].find(cand) {
                    let abs = start_pos + pos;
                    matches.push((abs, abs + cand.len(), cand.len(), j));
                    start_pos = abs + 1; // allow overlapping candidates but DP will pick non-overlap
                }
            }

            if matches.is_empty() {
                inner.push(vec![Segment::Literal(msg.clone())]);
            } else {
                // sort by end
                matches.sort_by_key(|t| t.1);
                let m = matches.len();
                // compute p[k] = largest index < k that doesn't overlap with k
                let mut p = vec![0usize; m];
                for k in 0..m {
                    let (s_k, e_k, _, _) = matches[k];
                    let mut pi = None;
                    for t in (0..k).rev() {
                        if matches[t].1 <= s_k { pi = Some(t); break; }
                    }
                    p[k] = pi.unwrap_or(usize::MAX);
                }

                // DP table: best weight up to index k
                let mut dp = vec![0usize; m];
                for k in 0..m {
                    let weight = matches[k].2;
                    let incl = if p[k] == usize::MAX { weight } else { weight + dp[p[k]] };
                    let excl = if k==0 { 0 } else { dp[k-1] };
                    dp[k] = std::cmp::max(incl, excl);
                }

                // Reconstruct chosen intervals
                let mut chosen = Vec::new();
                let mut k = m.checked_sub(1);
                while let Some(idx) = k {
                    let take = if idx==0 { dp[idx] > 0 } else { dp[idx] != dp[idx-1] };
                    if take {
                        chosen.push(idx);
                        k = p[idx].checked_sub(0);
                        if let Some(pk) = p[idx].checked_sub(0) {
                            if pk==usize::MAX { k = None; } else { k = Some(pk); }
                        } else { k = None; }
                    } else {
                        if idx==0 { k = None; } else { k = Some(idx-1); }
                    }
                }
                chosen.reverse();

                // build segments from chosen intervals
                let mut cursor = 0usize;
                let mut segs = Vec::new();
                for &ci in &chosen {
                    let (s, _e, match_len, midx) = matches[ci];
                    if s > cursor {
                        segs.push(Segment::Literal(msg[cursor..s].to_string()));
                    }
                    // reference the matched previous message from its start (full match)
                    segs.push(Segment::Reference { message_idx: midx, start: 0, len: match_len });
                    cursor = s + match_len;
                }
                if cursor < msg.len() {
                    segs.push(Segment::Literal(msg[cursor..].to_string()));
                }
                inner.push(segs);
            }
        }

        LongestMatch { inner, messages: messages_vec }
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
        for (i, segs) in self.inner.iter().enumerate() {
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


#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::generate_thread;

    #[test]
    fn segments_exact_duplicate_message_becomes_reference() {
        let msgs = &["hello", "hello"];
        let cf = LongestMatch::from_messages(msgs);
        let segs = cf.segments();
        assert_eq!(
            segs,
            vec![
                vec![Segment::Literal("hello".to_string())],
                vec![Segment::Reference { message_idx: 0, start: 0, len: 5 }]
            ]
        );
    }

    #[test]
    fn segments_prefix_reuse_becomes_reference_plus_literal() {
        let msgs = &["I love pizza", "I love pizza and pasta"];
        let cf = LongestMatch::from_messages(msgs);
        let segs = cf.segments();
        assert_eq!(
            segs,
            vec![
                vec![Segment::Literal("I love pizza".to_string())],
                vec![
                    Segment::Reference { message_idx: 0, start: 0, len: 12 },
                    Segment::Literal(" and pasta".to_string())
                ]
            ]
        );
    }

    #[test]
    fn render_with_lambda_replaces_references() {
        let msgs = &["foo", "foo bar"];
        let cf = LongestMatch::from_messages(msgs);

        let rendered = cf.render_with(|m_idx, start, len, referenced_text| {
            format!("<ref {}:{}+{}='{}'>", m_idx, start, len, referenced_text)
        });

        assert_eq!(rendered, vec!["foo".to_string(), "<ref 0:0+3='foo'> bar".to_string()]);
    }

    #[test]
    fn render_with_static_replaces_references_with_ellipses() {
        let msgs = &["repeat", "repeat repeat"];
        let cf = LongestMatch::from_messages(msgs);
        let rendered = cf.render_with_static("...");

        // DP may match both occurrences of the previous message, producing
        // two replacements. Accept that behavior.
        assert_eq!(rendered, vec!["repeat".to_string(), "... ...".to_string()]);
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
        let cf = LongestMatch::from_messages(&refs);
        let segs = cf.segments();
        let deduped: usize = segs.iter().flat_map(|v| v.iter()).map(|seg| match seg {
            Segment::Literal(s) => s.len(),
            Segment::Reference { .. } => 3,
        }).sum();

        if deduped as f64 > (orig as f64) * 0.1 {
            eprintln!("Dedup insufficient: orig={} bytes, deduped={} bytes ({}%)", orig, deduped, deduped as f64 / orig as f64 * 100.0);
            eprintln!("First 5 original messages:");
            for (i, m) in msgs.iter().take(5).enumerate() {
                eprintln!("[{}] len={}: {}", i, m.len(), &m.chars().take(120).collect::<String>());
            }
            eprintln!("First 10 segments:");
            for (i, v) in segs.iter().enumerate().take(10) {
                eprint!("[{}]: ", i);
                for seg in v {
                    match seg {
                        Segment::Literal(s) => eprint!("L(len={}) ", s.len()),
                        Segment::Reference { message_idx, start, len } => eprint!("R({}:{}+{} ) ", message_idx, start, len),
                    }
                }
                eprintln!("");
            }
            eprintln!("First 10 rendered messages:");
            for (i, m) in cf.render_with(|_, _, _, _| "...".to_string()).iter().take(10).enumerate() {
                eprintln!("[{}] {}", i, m);
            }
        }

        // expect at least 50% reduction for this first-pass algorithm
        assert!(deduped as f64 <= (orig as f64) * 0.5, "deduped={} orig={}", deduped, orig);
    }
}


