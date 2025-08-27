use copyforward::{exact, approximate, exact_tokens, approximate_tokens, Config, CopyForward, CopyForwardTokens};

#[test]
fn test_exact_with_none_strings() {
    let messages = &[Some("hello world"), None, Some("hello world today")];
    let compressed = exact(messages, Config::default());
    
    let segments = compressed.segments();
    assert_eq!(segments.len(), 3);
    // First message should have segments
    assert!(!segments[0].is_empty());
    // Second message (None) should have empty segments
    assert!(segments[1].is_empty());
    // Third message should have segments (likely referencing first)
    assert!(!segments[2].is_empty());
    
    let rendered = compressed.render_with(|_, _, _, text| text.to_string());
    assert_eq!(rendered.len(), 3);
    assert_eq!(rendered[0], "hello world");
    assert_eq!(rendered[1], ""); // None becomes empty string
    assert_eq!(rendered[2], "hello world today");
}

#[test]
fn test_approximate_with_none_strings() {
    let messages = &[Some("hello"), None, Some("world"), None, Some("hello world")];
    let compressed = approximate(messages, Config::default());
    
    let segments = compressed.segments();
    assert_eq!(segments.len(), 5);
    assert!(segments[1].is_empty()); // None entry
    assert!(segments[3].is_empty()); // None entry
    
    let rendered = compressed.render_with(|_, _, _, text| text.to_string());
    assert_eq!(rendered.len(), 5);
    assert_eq!(rendered[0], "hello");
    assert_eq!(rendered[1], "");
    assert_eq!(rendered[2], "world");
    assert_eq!(rendered[3], "");
    assert_eq!(rendered[4], "hello world");
}

#[test]
fn test_exact_tokens_with_none() {
    let messages = &[Some(vec![1u32, 2u32]), None, Some(vec![1u32, 2u32, 3u32])];
    let compressed = exact_tokens(messages, Config::default());
    
    let segments = compressed.segments();
    // Should only have segments for the non-None entries
    assert_eq!(segments.len(), 2);
}

#[test]
fn test_approximate_tokens_with_none() {
    let messages = &[Some(vec![10u32, 20u32]), None, Some(vec![30u32]), None, Some(vec![10u32, 20u32, 30u32])];
    let compressed = approximate_tokens(messages, Config::default());
    
    let segments = compressed.segments();
    // Should only have segments for the non-None entries (3 entries)
    assert_eq!(segments.len(), 3);
}

#[test]
fn test_mixed_string_types() {
    // Test that our MessageLike trait works with different string types
    let messages = &["hello", "world"]; // &str
    let compressed1 = exact(messages, Config::default());
    assert!(compressed1.segments().len() == 2);
    
    let messages2 = &[Some("hello"), Some("world")]; // Option<&str>
    let compressed2 = exact(messages2, Config::default());
    assert!(compressed2.segments().len() == 2);
    
    let messages3 = vec!["hello".to_string(), "world".to_string()]; // String
    let compressed3 = exact(&messages3, Config::default());
    assert!(compressed3.segments().len() == 2);
}

#[test]
fn test_mixed_token_types() {
    // Test that our TokenLike trait works with different token types
    let slice1: &[u32] = &[1, 2, 3];
    let slice2: &[u32] = &[4, 5, 6];
    let messages1 = &[slice1, slice2]; // &[u32]
    let compressed1 = exact_tokens(messages1, Config::default());
    assert!(compressed1.segments().len() == 2);
    
    let messages2 = &[Some(vec![1u32, 2u32]), Some(vec![4u32, 5u32])]; // Option<Vec<u32>>
    let compressed2 = exact_tokens(messages2, Config::default());
    assert!(compressed2.segments().len() == 2);
    
    let messages3 = vec![vec![1u32, 2u32], vec![4u32, 5u32]]; // Vec<u32>
    let compressed3 = exact_tokens(&messages3, Config::default());
    assert!(compressed3.segments().len() == 2);
}