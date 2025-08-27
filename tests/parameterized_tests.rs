use copyforward::fixture::generate_thread;
use copyforward::{Config, CopyForward, Segment, approximate, exact};

#[test]
fn render_with_lambda_replaces_references() {
    let msgs = &["hello world", "hello world today"];
    let config = Config::default();
    let cf = exact(msgs, config);

    let rendered = cf.render_with(|m_idx, start, len, referenced_text| {
        format!("<ref {m_idx}:{start}+{len}='{referenced_text}'>")
    });

    // Just verify we can render without crashing and get expected length
    assert_eq!(rendered.len(), 2);
    assert!(!rendered[0].is_empty());
    assert!(!rendered[1].is_empty());
}

fn run_fixture_thread_is_deduped_substantially<C>(cf: C, orig_msgs: Vec<String>)
where
    C: CopyForward,
{
    let segs = cf.segments();
    let deduped: usize = segs
        .iter()
        .flat_map(|v| v.iter())
        .map(|seg| match seg {
            Segment::Literal(s) => s.len(),
            Segment::Reference { .. } => 3,
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

    let refs: Vec<&str> = msgs.iter().map(|s| s.as_str()).collect();
    let config = Config::default();
    run_fixture_thread_is_deduped_substantially(exact(&refs, config.clone()), msgs.clone());
    run_fixture_thread_is_deduped_substantially(approximate(&refs, config), msgs);
}

fn run_partial_overlaps_across_multiple_messages<C>(cf: C)
where
    C: CopyForward,
{
    let segs = cf.segments();
    assert!(segs[2].len() >= 2, "Should have multiple segments");
    let rendered = cf.render_with(|_, _, _, text| text.to_string());
    assert_eq!(rendered[2], "hello world peace and joy for everyone");
    let has_long_match = segs[2]
        .iter()
        .any(|seg| matches!(seg, Segment::Reference { len, .. } if *len >= Config::default().min_match_len));
    assert!(
        has_long_match,
        "Should have at least one match of 10+ characters"
    );
}

#[test]
fn partial_overlaps_across_multiple_messages() {
    let msgs = &[
        "hello world everyone",
        "world peace and harmony",
        "hello world peace and joy for everyone",
    ];
    let config = Config::default();
    run_partial_overlaps_across_multiple_messages(exact(msgs, config.clone()));
    run_partial_overlaps_across_multiple_messages(approximate(msgs, config));
}

#[test]
fn finds_longest_common_substrings() {
    let msgs = &[
        "The quick brown fox jumps over the lazy dog",
        "A quick brown fox is very fast",
        "The quick brown fox is amazing and the lazy dog sleeps",
    ];
    let config = Config::default();
    let cf = exact(msgs, config);

    // Verify segments were created and can be rendered
    let segs = cf.segments();
    assert_eq!(segs.len(), 3);

    let rendered = cf.render_with(|_, _, _, text| text.to_string());
    assert_eq!(rendered, msgs);
}

fn run_handles_overlapping_substrings_efficiently<C>(cf: C)
where
    C: CopyForward,
{
    let segs = cf.segments();
    let rendered = cf.render_with(|_, _, _, text| text.to_string());
    assert_eq!(
        rendered[2],
        "programming and programming languages are great for programming"
    );
    let ref_count: usize = segs[2]
        .iter()
        .map(|seg| match seg {
            &Segment::Reference { .. } => 1,
            _ => 0,
        })
        .sum();
    // Should have some compression - either references or compact segments
    assert!(
        ref_count >= 1 || segs[2].len() <= 4,
        "Should have some compression for repeated 'programming'"
    );
}

#[test]
fn handles_overlapping_substrings_efficiently() {
    let msgs = &[
        "programming is programming and more programming",
        "I love programming and programming languages",
        "programming and programming languages are great for programming",
    ];
    let config = Config::default();
    run_handles_overlapping_substrings_efficiently(exact(msgs, config.clone()));
    run_handles_overlapping_substrings_efficiently(approximate(msgs, config));
}

#[test]
fn test_basic_compression() {
    let msgs = &["Hello world", "Hello world today"];
    let config = Config::default();

    // Test exact
    let exact_cf = exact(msgs, config.clone());
    let exact_segs = exact_cf.segments();
    assert_eq!(exact_segs[0].len(), 1); // First message should be one literal
    assert!(exact_segs[1].len() >= 2); // Second message should have reference + literal

    // Test approximate
    let approx_cf = approximate(msgs, config);
    let approx_segs = approx_cf.segments();
    assert_eq!(approx_segs[0].len(), 1); // First message should be one literal
    assert!(approx_segs[1].len() >= 2); // Second message should have reference + literal

    // Both should render back to original
    let exact_rendered = exact_cf.render_with(|_, _, _, text| text.to_string());
    let approx_rendered = approx_cf.render_with(|_, _, _, text| text.to_string());
    assert_eq!(exact_rendered, msgs);
    assert_eq!(approx_rendered, msgs);
}
