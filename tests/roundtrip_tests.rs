//! Broad round-trip and edge-case coverage for both text and token modes.
//!
//! The invariant under test everywhere: rendering with an empty replacement
//! reconstructs the original messages exactly, and every reference segment
//! points at the text it claims.

use copyforward::{
    Config, CopyForward, CopyForwardTokens, Segment, approximate, approximate_tokens, exact,
    exact_tokens, fixture::generate_thread,
};

fn assert_text_round_trip<C: CopyForward>(cf: &C, msgs: &[&str]) {
    // A replacer that returns the referenced text must reconstruct the
    // originals exactly.
    let rendered = cf.render_with(|_, _, _, text| text.to_string());
    assert_eq!(rendered.len(), msgs.len());
    for (i, (got, want)) in rendered.iter().zip(msgs.iter()).enumerate() {
        assert_eq!(got, want, "message {i} did not round-trip");
    }
}

/// Every reference passed to the replacer must point at exactly the text it
/// references in the original messages.
fn assert_reference_invariants<C: CopyForward>(cf: &C, msgs: &[String]) {
    cf.render_with(|m_idx, start, len, text| {
        let expected = &msgs[m_idx][start..start + len];
        assert_eq!(
            expected, text,
            "reference ({m_idx},{start},{len}) points at wrong text"
        );
        text.to_string()
    });
}

#[test]
fn test_empty_input() {
    let cfg = Config::default();
    let cf = exact(&[] as &[&str], cfg.clone());
    assert_eq!(cf.segments().len(), 0);
    assert_eq!(cf.render_with_static(""), Vec::<String>::new());

    let cf = approximate(&[] as &[&str], cfg);
    assert_eq!(cf.segments().len(), 0);
}

#[test]
fn test_empty_string_message() {
    let msgs = &["", "hello world", "hello world"];
    let cfg = Config::default();
    let cf = exact(msgs, cfg.clone());
    assert_text_round_trip(&cf, msgs);
    assert!(cf.segments()[0].is_empty());

    let cf = approximate(msgs, cfg);
    assert_text_round_trip(&cf, msgs);
    assert!(cf.segments()[0].is_empty());
}

#[test]
fn test_single_message() {
    let msgs = &["just one message"];
    let cfg = Config::default();
    let cf = exact(msgs, cfg.clone());
    assert_text_round_trip(&cf, msgs);
    let cf = approximate(msgs, cfg);
    assert_text_round_trip(&cf, msgs);
}

#[test]
fn test_unicode_round_trip() {
    let msgs = &[
        "héllo wörld 🌍 café",
        "🌍 héllo wörld café",
        "café héllo wörld 🌍 again",
    ];
    let cfg = Config::default();
    let cf = exact(msgs, cfg.clone());
    assert_text_round_trip(&cf, msgs);
    let cf = approximate(msgs, cfg);
    assert_text_round_trip(&cf, msgs);
}

#[test]
fn test_min_match_len_one() {
    let msgs = &["abcd", "xabc", "yabcz"];
    let cfg = Config {
        min_match_len: 1,
        ..Config::default()
    };
    let cf = exact(msgs, cfg.clone());
    assert_text_round_trip(&cf, msgs);
    let cf = approximate(msgs, cfg);
    assert_text_round_trip(&cf, msgs);
}

#[test]
fn test_min_match_len_larger_than_messages() {
    let msgs = &["short", "shorter", "shortest"];
    let cfg = Config {
        min_match_len: 100,
        ..Config::default()
    };
    // Nothing can match, so every segment must be literal.
    fn assert_all_literals<C: CopyForward>(cf: &C) {
        for segs in cf.segments() {
            for seg in segs {
                assert!(matches!(seg, Segment::Literal(_)));
            }
        }
    }
    let cf = exact(msgs, cfg.clone());
    assert_all_literals(&cf);
    let cf = approximate(msgs, cfg);
    assert_all_literals(&cf);
}

#[test]
fn test_long_repeated_run_single_reference() {
    let long = "a".repeat(1000);
    let msgs = [long.clone(), long];
    let refs: Vec<&str> = msgs.iter().map(|s| s.as_str()).collect();

    fn assert_full_run_reference<C: CopyForward>(cf: &C, name: &str) {
        let segs = cf.segments();
        assert_eq!(segs[1].len(), 1, "{name}: expected one segment");
        match &segs[1][0] {
            Segment::Reference {
                message_idx,
                start,
                len,
            } => {
                assert_eq!(*message_idx, 0);
                assert_eq!(*start, 0);
                assert_eq!(*len, 1000, "{name}: full run should be one reference");
            }
            other => panic!("{name}: expected a reference, got {other:?}"),
        }
    }
    let cf = exact(&refs, Config::default());
    assert_full_run_reference(&cf, "exact");
    let cf = approximate(&refs, Config::default());
    assert_full_run_reference(&cf, "approx");
}

#[test]
fn test_reference_invariants_on_fixture_threads() {
    for seed in 1u64..=8 {
        let msgs = generate_thread(seed, 60, 6);
        let owned: Vec<String> = msgs.clone();
        let refs: Vec<&str> = owned.iter().map(|s| s.as_str()).collect();

        let cf = exact(&refs, Config::default());
        assert_reference_invariants(&cf, &owned);
        assert_text_round_trip(&cf, &refs);

        let cf = approximate(&refs, Config::default());
        assert_reference_invariants(&cf, &owned);
        assert_text_round_trip(&cf, &refs);
    }
}

#[test]
fn test_token_zero_values_round_trip() {
    let msgs: Vec<Vec<u32>> = vec![vec![0, 0, 0, 0, 1], vec![0, 0, 0, 0, 2]];
    let refs: Vec<&[u32]> = msgs.iter().map(|v| v.as_slice()).collect();
    for name in ["exact", "approx"] {
        let rendered = if name == "exact" {
            exact_tokens(&refs, Config::default()).render_with(|_, _, _, s| s.to_vec())
        } else {
            approximate_tokens(&refs, Config::default()).render_with(|_, _, _, s| s.to_vec())
        };
        assert_eq!(rendered, msgs, "{name} must round-trip zero-valued tokens");
    }
}

#[test]
fn test_random_token_threads_round_trip() {
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha8Rng;

    for seed in 0..16u64 {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let n_msgs = rng.gen_range(1..=20);
        let msgs: Vec<Vec<u32>> = (0..n_msgs)
            .map(|_| {
                let len = rng.gen_range(0..=60);
                (0..len).map(|_| rng.gen_range(0..12)).collect()
            })
            .collect();
        let refs: Vec<&[u32]> = msgs.iter().map(|v| v.as_slice()).collect();

        let exact = exact_tokens(&refs, Config::default());
        assert_eq!(
            exact.render_with(|_, _, _, s| s.to_vec()),
            msgs,
            "exact token round-trip failed for seed {seed}"
        );

        let approx = approximate_tokens(&refs, Config::default());
        assert_eq!(
            approx.render_with(|_, _, _, s| s.to_vec()),
            msgs,
            "approx token round-trip failed for seed {seed}"
        );
    }
}

#[test]
fn test_segments_chars_uses_character_offsets() {
    let m0 = "h\u{e9}llo w\u{f6}rld \u{1f30d}";
    let m1 = "\u{1f30d} h\u{e9}llo w\u{f6}rld";
    let msgs = [m0, m1];

    fn rebuild(msgs: &[&str], segs: &[Segment]) -> String {
        let mut out = String::new();
        for seg in segs {
            match seg {
                Segment::Literal(text) => out.push_str(text),
                Segment::Reference {
                    message_idx,
                    start,
                    len,
                } => out.push_str(&msgs[*message_idx][*start..*start + len]),
            }
        }
        out
    }

    fn rebuild_chars(msgs: &[&str], segs: &[Segment]) -> String {
        let mut out = String::new();
        for seg in segs {
            match seg {
                Segment::Literal(text) => out.push_str(text),
                Segment::Reference {
                    message_idx,
                    start,
                    len,
                } => out.extend(msgs[*message_idx].chars().skip(*start).take(*len)),
            }
        }
        out
    }

    fn check(
        msgs: &[&str],
        target: &str,
        byte_segs: &[Segment],
        char_segs: &[Segment],
        name: &str,
    ) {
        // Byte offsets (documented Segment semantics) reconstruct via byte slices.
        assert_eq!(rebuild(msgs, byte_segs), target, "{name}: byte offsets");
        // Character offsets index the strings by code point.
        assert_eq!(
            rebuild_chars(msgs, char_segs),
            target,
            "{name}: char offsets must index strings by code point"
        );
        // For this input the char and byte lengths of the reference differ,
        // which is what makes the two representations observable.
        let char_ref = char_segs
            .iter()
            .find_map(|s| match s {
                Segment::Reference { start, len, .. } => Some((*start, *len)),
                _ => None,
            })
            .expect("expected a reference in message 1");
        let byte_ref = byte_segs
            .iter()
            .find_map(|s| match s {
                Segment::Reference { start, len, .. } => Some((*start, *len)),
                _ => None,
            })
            .expect("expected a reference in message 1");
        assert_ne!(char_ref, byte_ref, "test needs multi-byte content");
    }

    let cf = exact(&msgs, Config::default());
    check(
        &msgs,
        m1,
        &cf.segments()[1],
        &cf.segments_chars()[1],
        "exact",
    );
    let cf = approximate(&msgs, Config::default());
    check(
        &msgs,
        m1,
        &cf.segments()[1],
        &cf.segments_chars()[1],
        "approx",
    );
}

/// The exact engine caps expensive candidate extensions at 64 per lookup.
/// Candidates that cannot beat the current best must not count against that
/// cap, or long matches hidden past 64 weaker candidates are missed.
#[test]
fn test_exact_finds_match_beyond_sixty_four_candidates() {
    // Nine 10-char x-messages contribute 7 kmers each (63 total); the 10th
    // adds 7 more, pushing the winning 50-char candidate past position 64.
    let mut msgs: Vec<String> = vec!["x".repeat(10); 10];
    msgs.push("x".repeat(50));
    msgs.push(format!("{}z", "x".repeat(50)));
    let refs: Vec<&str> = msgs.iter().map(|s| s.as_str()).collect();

    let cf = exact(&refs, Config::default());
    let segs = cf.segments();
    assert!(
        segs[11]
            .iter()
            .any(|seg| { matches!(seg, Segment::Reference { len, .. } if *len >= 50) }),
        "exact should find the 50-char match, got {:?}",
        segs[11]
    );
}
