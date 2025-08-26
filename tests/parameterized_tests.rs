use copyforward::core::{CopyForward, GreedySubstringConfig, Segment};
use copyforward::fixture::generate_thread;
use copyforward::greedy::GreedySubstring;
use copyforward::hashed::HashedGreedy;
use copyforward::hashed_binary::HashedGreedyBinary;

// Generic helpers: accept any concrete implementation of CopyForward.
fn run_render_with_lambda_replaces_references<C: CopyForward>(cf: C) {
    let rendered = cf.render_with(|m_idx, start, len, referenced_text| {
        format!("<ref {m_idx}:{start}+{len}='{referenced_text}'>")
    });
    assert_eq!(
        rendered,
        vec![
            "hello world".to_string(),
            "<ref 0:0+11='hello world'> today".to_string()
        ]
    );
}

#[test]
fn render_with_lambda_replaces_references() {
    let msgs = &["hello world", "hello world today"];
    run_render_with_lambda_replaces_references(GreedySubstring::from_messages(msgs));
    run_render_with_lambda_replaces_references(HashedGreedy::from_messages(msgs));
    run_render_with_lambda_replaces_references(HashedGreedyBinary::from_messages(msgs));
}

fn run_fixture_thread_is_deduped_substantially<C: CopyForward>(orig_msgs: Vec<String>) {
    let refs: Vec<&str> = orig_msgs.iter().map(|s| s.as_str()).collect();
    let cf = C::from_messages(&refs);
    let segs = cf.segments();
    let deduped: usize = segs
        .iter()
        .flat_map(|v| v.iter())
        .map(|seg| match seg {
            &Segment::Literal(ref s) => s.len(),
            &Segment::Reference { .. } => 3,
        })
        .sum();
    let orig: usize = orig_msgs.iter().map(|s| s.len()).sum();
    assert!(
        deduped as f64 <= (orig as f64) * 0.5,
        "deduped={} orig={}",
        deduped,
        orig
    );
}

#[test]
fn fixture_thread_is_deduped_substantially() {
    let target_kb = 25usize;
    let mut n = 4usize;
    let mut msgs: Vec<String> = Vec::new();
    let mut orig = 0usize;
    while orig < target_kb * 1024 {
        msgs = generate_thread(12345, n, 5);
        orig = msgs.iter().map(|s| s.len()).sum();
        n *= 2;
        if n > 4096 {
            break;
        }
    }

    run_fixture_thread_is_deduped_substantially::<GreedySubstring>(msgs.clone());
    run_fixture_thread_is_deduped_substantially::<HashedGreedy>(msgs.clone());
    run_fixture_thread_is_deduped_substantially::<HashedGreedyBinary>(msgs);
}

fn run_partial_overlaps_across_multiple_messages<C: CopyForward>() {
    let msgs = &[
        "hello world everyone",
        "world peace and harmony",
        "hello world peace and joy for everyone",
    ];
    let cf = C::from_messages(msgs);
    let segs = cf.segments();
    assert!(segs[2].len() >= 2, "Should have multiple segments");
    let rendered = cf.render_with(|_, _, _, text| text.to_string());
    assert_eq!(rendered[2], "hello world peace and joy for everyone");
    let has_long_match = segs[2].iter().any(|seg| match seg {
        &Segment::Reference { len, .. }
            if len >= GreedySubstringConfig::default().min_match_len =>
        {
            true
        }
        _ => false,
    });
    assert!(
        has_long_match,
        "Should have at least one match of 10+ characters"
    );
}

#[test]
fn partial_overlaps_across_multiple_messages() {
    run_partial_overlaps_across_multiple_messages::<GreedySubstring>();
    run_partial_overlaps_across_multiple_messages::<HashedGreedy>();
    run_partial_overlaps_across_multiple_messages::<HashedGreedyBinary>();
}

fn run_finds_longest_common_substrings<C: CopyForward>() {
    let msgs = &[
        "The quick brown fox jumps over the lazy dog",
        "A quick brown fox is very fast",
        "The quick brown fox is amazing and the lazy dog sleeps",
    ];
    let cf = C::from_messages(msgs);
    let segs = cf.segments();
    let longest_match = segs[2]
        .iter()
        .filter_map(|seg| match seg {
            &Segment::Reference { len, .. } => Some(len),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    assert!(
        longest_match >= GreedySubstringConfig::default().min_match_len,
        "Should find matches of at least {} characters, found {}",
        GreedySubstringConfig::default().min_match_len,
        longest_match
    );
    assert!(
        segs[2].len() <= 5,
        "Should use few segments with long matches, got {}",
        segs[2].len()
    );
}

#[test]
fn finds_longest_common_substrings() {
    run_finds_longest_common_substrings::<GreedySubstring>();
    run_finds_longest_common_substrings::<HashedGreedy>();
    run_finds_longest_common_substrings::<HashedGreedyBinary>();
}

fn run_handles_overlapping_substrings_efficiently<C: CopyForward>() {
    let msgs = &[
        "programming is programming and more programming",
        "I love programming and programming languages",
        "programming and programming languages are great for programming",
    ];
    let cf = C::from_messages(msgs);
    let segs = cf.segments();
    let rendered = cf.render_with(|_, _, _, text| text.to_string());
    assert_eq!(
        rendered[2],
        "programming and programming languages are great for programming"
    );
    let ref_count = segs[2]
        .iter()
        .filter(|seg| matches!(seg, &Segment::Reference { .. }))
        .count();
    assert!(
        ref_count >= 2,
        "Should find multiple non-overlapping references"
    );
}

#[test]
fn handles_overlapping_substrings_efficiently() {
    run_handles_overlapping_substrings_efficiently::<GreedySubstring>();
    run_handles_overlapping_substrings_efficiently::<HashedGreedy>();
    run_handles_overlapping_substrings_efficiently::<HashedGreedyBinary>();
}

fn run_respects_minimum_match_length_realistically<C: CopyForward>() {
    let msgs = &[
        "This is a short test message for our algorithm",
        "Another short test of the algorithm implementation",
        "This is a short test that validates our algorithm works correctly",
    ];
    let cf = C::from_messages(msgs);
    let segs = cf.segments();
    for seg in segs[2].iter() {
        if let &Segment::Reference { len, .. } = seg {
            assert!(
                len >= GreedySubstringConfig::default().min_match_len,
                "Found reference shorter than min_match_len: {}",
                len
            );
        }
    }
    let has_long_match = segs[2].iter().any(|seg| match seg {
        &Segment::Reference { len, .. } => len >= GreedySubstringConfig::default().min_match_len,
        _ => false,
    });
    assert!(
        has_long_match,
        "Should find at least one match >= {} characters",
        GreedySubstringConfig::default().min_match_len
    );
}

#[test]
fn respects_minimum_match_length_realistically() {
    run_respects_minimum_match_length_realistically::<GreedySubstring>();
    run_respects_minimum_match_length_realistically::<HashedGreedy>();
    run_respects_minimum_match_length_realistically::<HashedGreedyBinary>();
}

fn run_finds_substrings_from_middle_of_messages<C: CopyForward>() {
    let msgs = &[
        "Hello everyone, the weather is absolutely wonderful today!",
        "I hope that the weather stays wonderful for the weekend",
        "Yes, the weather is wonderful and I love these sunny days",
    ];
    let cf = C::from_messages(msgs);
    let segs = cf.segments();
    let has_middle_match = segs[2].iter().any(|seg| match seg {
        &Segment::Reference {
            message_idx,
            start,
            len,
            ..
        } => len >= 12 && start > 0 && message_idx < 2,
        _ => false,
    });
    assert!(
        has_middle_match,
        "Should find substring matches from middle of previous messages"
    );
}

#[test]
fn finds_substrings_from_middle_of_messages() {
    let config = GreedySubstringConfig {
        min_match_len: 12,
        lookback: None,
    };
    run_finds_substrings_from_middle_of_messages::<GreedySubstring>();
    run_finds_substrings_from_middle_of_messages::<HashedGreedy>();
    run_finds_substrings_from_middle_of_messages::<HashedGreedyBinary>();
}

fn run_handles_multiple_references_to_same_substring<C: CopyForward>() {
    let msgs = &[
        "artificial intelligence and machine learning",
        "machine learning algorithms use artificial intelligence",
        "artificial intelligence powers machine learning and machine learning improves artificial intelligence",
    ];
    let cf = C::from_messages(msgs);
    let segs = cf.segments();
    let ref_segments: Vec<_> = segs[2]
        .iter()
        .filter_map(|seg| match seg {
            &Segment::Reference { len, .. } => Some(len),
            _ => None,
        })
        .collect();
    assert!(
        ref_segments.len() >= 2,
        "Should find multiple long references"
    );
    assert!(
        ref_segments
            .iter()
            .any(|&len| len >= GreedySubstringConfig::default().min_match_len),
        "Should find references >= {} chars",
        GreedySubstringConfig::default().min_match_len
    );
}

#[test]
fn handles_multiple_references_to_same_substring() {
    run_handles_multiple_references_to_same_substring::<GreedySubstring>();
    run_handles_multiple_references_to_same_substring::<HashedGreedy>();
    run_handles_multiple_references_to_same_substring::<HashedGreedyBinary>();
}

fn run_compression_with_realistic_conversation<C: CopyForward>() {
    let msgs = &[
        "Let's implement the new authentication system using JWT tokens for security",
        "> Let's implement the new authentication system using JWT tokens for security\nI agree, but what about token expiration policies?",
        ">> Let's implement the new authentication system using JWT tokens for security\n> I agree, but what about token expiration policies?\nGood point about token expiration policies. We should also consider secure storage",
        ">>> Let's implement the new authentication system using JWT tokens for security\n>> I agree, but what about token expiration policies?\n> Good point about token expiration policies. We should also consider secure storage\nAll these points about JWT tokens for security and token expiration policies are valid",
    ];
    let cf = C::from_messages(msgs);
    let segs = cf.segments();
    let original_size: usize = msgs.iter().map(|s| s.len()).sum();
    let compressed_size: usize = segs
        .iter()
        .flat_map(|v| v.iter())
        .map(|seg| match seg {
            &Segment::Literal(ref s) => s.len(),
            &Segment::Reference { .. } => 10,
        })
        .sum();
    let compression_ratio = compressed_size as f64 / original_size as f64;
    assert!(
        compression_ratio < 0.5,
        "Should achieve major compression with nested quotes: {:.2}%",
        compression_ratio * 100.0
    );
    let has_meaningful_matches = segs.iter().skip(1).any(|msg_segs| {
        msg_segs.iter().any(|seg| match seg {
            &Segment::Reference { len, .. } => {
                len >= GreedySubstringConfig::default().min_match_len
            }
            _ => false,
        })
    });
    assert!(
        has_meaningful_matches,
        "Should find meaningful phrase matches across messages"
    );
}

#[test]
fn compression_with_realistic_conversation() {
    run_compression_with_realistic_conversation::<GreedySubstring>();
    run_compression_with_realistic_conversation::<HashedGreedy>();
    run_compression_with_realistic_conversation::<HashedGreedyBinary>();
}
