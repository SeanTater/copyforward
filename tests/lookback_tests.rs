//! Tests for the `Config::lookback` window: references may only point at
//! the most recent `lookback` messages.

use copyforward::{Config, CopyForward, approximate, exact};

fn lookback(lb: usize) -> Config {
    Config {
        lookback: Some(lb),
        ..Config::default()
    }
}

#[test]
fn test_lookback_excludes_out_of_window_references() {
    // Message 2 repeats message 0, which is out of a 1-message window.
    let msgs = &[
        "unique alpha content",
        "totally different words",
        "unique alpha content",
    ];

    let exact = exact(msgs, lookback(1));
    assert_eq!(
        exact.render_with_static("[R]")[2],
        "unique alpha content",
        "exact must not reference a message outside the window"
    );

    let approx = approximate(msgs, lookback(1));
    assert_eq!(
        approx.render_with_static("[R]")[2],
        "unique alpha content",
        "approximate must not reference a message outside the window"
    );
}

#[test]
fn test_lookback_still_allows_in_window_references() {
    let msgs = &[
        "unique alpha content",
        "filler in between here",
        "unique alpha content",
    ];

    let exact = exact(msgs, lookback(2));
    assert_eq!(exact.render_with_static("[R]")[2], "[R]");

    let approx = approximate(msgs, lookback(2));
    assert_eq!(approx.render_with_static("[R]")[2], "[R]");
}

#[test]
fn test_lookback_window_slides_forward() {
    // A, B, A, A with a window of 2: message 3 may reference message 2.
    let msgs = &[
        "unique alpha content",
        "filler in between here",
        "unique alpha content",
        "unique alpha content",
    ];

    let rendered = exact(msgs, lookback(2)).render_with_static("[R]");
    assert_eq!(rendered[2], "[R]");
    assert_eq!(rendered[3], "[R]");

    let rendered = approximate(msgs, lookback(2)).render_with_static("[R]");
    assert_eq!(rendered[2], "[R]");
    assert_eq!(rendered[3], "[R]");
}

#[test]
fn test_lookback_zero_disables_references() {
    let msgs = &["hello world hello world", "hello world hello world"];

    let rendered = exact(msgs, lookback(0)).render_with_static("[R]");
    assert_eq!(rendered[1], "hello world hello world");

    let rendered = approximate(msgs, lookback(0)).render_with_static("[R]");
    assert_eq!(rendered[1], "hello world hello world");
}

#[test]
fn test_unlimited_lookback_keeps_all_references() {
    let msgs = &[
        "unique alpha content",
        "filler in between here",
        "unique alpha content",
    ];

    let rendered = exact(msgs, Config::default()).render_with_static("[R]");
    assert_eq!(rendered[2], "[R]");

    let rendered = approximate(msgs, Config::default()).render_with_static("[R]");
    assert_eq!(rendered[2], "[R]");
}
