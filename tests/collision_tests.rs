//! Regression tests for rolling-hash collisions.
//!
//! Both engines index k-mers by a 64-bit polynomial rolling hash (base 257,
//! mod 2^64). Distinct token sequences can share a hash: e.g. [1000, 5000]
//! and [1016, 888] both hash to 262_000. A candidate selected on hash
//! equality alone must be verified against actual content, otherwise a
//! collision silently substitutes the wrong text.

use copyforward::{
    Config, CopyForward, CopyForwardTokens, TokenSegment, capped, capped_tokens, greedy,
    greedy_tokens,
};

fn min2() -> Config {
    Config {
        min_match_len: 2,
        ..Config::default()
    }
}

/// [1000, 5000] and [1016, 888] collide under the engine rolling hash.
#[test]
fn test_token_kmer_collision_round_trip_exact() {
    let msgs: Vec<Vec<u32>> = vec![vec![1000, 5000], vec![1016, 888]];
    let refs: Vec<&[u32]> = msgs.iter().map(|v| v.as_slice()).collect();
    let cf = greedy_tokens(&refs, min2());
    let rendered = cf.render_with(|_, _, _, slice| slice.to_vec());
    assert_eq!(rendered, msgs, "collision must not corrupt greedy output");
}

#[test]
fn test_token_kmer_collision_round_trip_approx() {
    let msgs: Vec<Vec<u32>> = vec![vec![1000, 5000], vec![1016, 888]];
    let refs: Vec<&[u32]> = msgs.iter().map(|v| v.as_slice()).collect();
    let cf = capped_tokens(&refs, min2());
    let rendered = cf.render_with(|_, _, _, slice| slice.to_vec());
    assert_eq!(rendered, msgs, "collision must not corrupt capped output");
}

/// A colliding k-mer with no real overlap must yield no reference at all.
#[test]
fn test_token_collision_without_overlap_yields_literal() {
    let msgs: Vec<Vec<u32>> = vec![vec![1000, 5000, 7, 7, 7], vec![1016, 888, 7, 7, 7]];
    let refs: Vec<&[u32]> = msgs.iter().map(|v| v.as_slice()).collect();

    let greedy = greedy_tokens(&refs, min2());
    assert_eq!(greedy.render_with(|_, _, _, s| s.to_vec()), msgs);
    // The first two tokens of message 1 share no real content with message 0.
    match &greedy.segments()[1][0] {
        TokenSegment::Literal(toks) => assert_eq!(toks, &[1016, 888]),
        other => panic!("expected literal prefix, got {other:?}"),
    }

    let approx = capped_tokens(&refs, min2());
    assert_eq!(approx.render_with(|_, _, _, s| s.to_vec()), msgs);
}

/// Length-3 collision: [1000, 2000, 70000] and [1001, 2000, 3951] both hash
/// to 66_633_000. Exercises verification of extended (multi-token) matches.
#[test]
fn test_token_kmer_collision_length_three() {
    let msgs: Vec<Vec<u32>> = vec![vec![1000, 2000, 70000], vec![1001, 2000, 3951]];
    let refs: Vec<&[u32]> = msgs.iter().map(|v| v.as_slice()).collect();
    let cfg = Config {
        min_match_len: 3,
        ..Config::default()
    };
    for name in ["greedy", "approx"] {
        let rendered = if name == "greedy" {
            greedy_tokens(&refs, cfg.clone()).render_with(|_, _, _, s| s.to_vec())
        } else {
            capped_tokens(&refs, cfg.clone()).render_with(|_, _, _, s| s.to_vec())
        };
        assert_eq!(rendered, msgs, "{name} must survive the length-3 collision");
    }
}

/// Same collisions expressed as Unicode scalar values through the text API.
#[test]
fn test_text_kmer_collision_round_trip() {
    let m0 = "\u{03e8}\u{1388}"; // scalar values 1000, 5000
    let m1 = "\u{03f8}\u{0378}"; // scalar values 1016, 888
    let msgs = vec![m0.to_string(), m1.to_string()];
    let refs: Vec<&str> = msgs.iter().map(|s| s.as_str()).collect();

    let greedy = greedy(&refs, min2());
    assert_eq!(greedy.render_with(|_, _, _, t| t.to_string()), msgs);

    let approx = capped(&refs, min2());
    assert_eq!(approx.render_with(|_, _, _, t| t.to_string()), msgs);
}
