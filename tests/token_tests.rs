use copyforward::{Config, CopyForwardTokens, approximate_tokens, exact_tokens};

#[test]
fn exact_tokens_round_trip() {
    let msgs: Vec<Vec<u32>> = vec![vec![1, 2, 3, 4, 5, 6], vec![1, 2, 3, 9, 9, 9, 4, 5, 6]];
    let refs: Vec<&[u32]> = msgs.iter().map(|v| v.as_slice()).collect();
    let cf = exact_tokens(&refs, Config::default());
    let rendered = cf.render_with(|_, _, _, slice| slice.to_vec());
    assert_eq!(rendered, msgs);
}

#[test]
fn approximate_tokens_round_trip() {
    let msgs: Vec<Vec<u32>> = vec![
        vec![10, 11, 12, 13, 14, 15, 16, 17],
        vec![10, 11, 12, 99, 100, 13, 14, 15, 16, 17],
    ];
    let refs: Vec<&[u32]> = msgs.iter().map(|v| v.as_slice()).collect();
    let cf = approximate_tokens(&refs, Config::default());
    let rendered = cf.render_with(|_, _, _, slice| slice.to_vec());
    assert_eq!(rendered, msgs);
}
