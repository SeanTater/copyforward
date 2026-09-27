use copyforward::fixture::generate_thread;
use copyforward::{Config, CopyForward, Segment, approximate};

#[test]
fn capped_preserves_rendering_small() {
    let msgs = generate_thread(1, 10, 10);
    let mut refs: Vec<&str> = Vec::with_capacity(msgs.len());
    for s in &msgs {
        refs.push(s.as_str());
    }

    let cap = approximate(&refs, Config::default());
    let rendered = cap.render_with(|_, _, _, s| s.to_string());

    for (i, r) in rendered.iter().enumerate() {
        if r != refs[i] {
            eprintln!("EXPECTED:\n{}", refs[i]);
            eprintln!("GOT:\n{}", r);
            let cf = approximate(&refs, Config::default());
            eprintln!("SEGS: {:?}", cf.segments()[i]);
        }
        assert_eq!(r, refs[i]);
    }
}

#[test]
fn capped_references_longest_recent_occurrence() {
    // Each message prepends a fixed prefix, so the last message contains a
    // long run of the prefix followed by the base. The engine should match
    // that run against the most recent (longest) occurrence instead of
    // fragmenting it into many tiny literals and short refs.
    let prefix = "Added at start.\n> ";
    let base = "This is the base post body.";
    let mut msgs: Vec<String> = vec![base.to_string()];
    for _ in 0..40 {
        msgs.push(format!("{prefix}{}", msgs.last().unwrap()));
    }
    let refs: Vec<&str> = msgs.iter().map(|s| s.as_str()).collect();

    let cf = approximate(&refs, Config::default());
    let last = &cf.segments()[40];
    let lit_bytes: usize = last
        .iter()
        .filter_map(|s| match s {
            Segment::Literal(t) => Some(t.len()),
            _ => None,
        })
        .sum();
    assert!(
        lit_bytes < prefix.len(),
        "prefix run should be referenced, not literal: {last:?}"
    );
    assert!(
        last.len() <= 4,
        "expected a few long segments, got {} segments: {last:?}",
        last.len()
    );
}

#[test]
fn capped_coalesces_adjacent_refs() {
    // Construct messages where coalescing should occur: several messages
    // with repeated content so references are consecutive.
    let msgs = vec![
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", // 64+ a's
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    ];
    let mut refs: Vec<&str> = Vec::with_capacity(msgs.len());
    for s in &msgs {
        refs.push(*s);
    }

    let cap = approximate(&refs, Config::default());
    let segs = cap.segments();
    // After coalescing we expect at least one reference segment in the second
    // message that references the first message with length >= 64
    let second = &segs[1];
    let mut found_ref = false;
    for seg in second.iter() {
        if matches!(
            seg,
            copyforward::Segment::Reference { message_idx, len, .. }
                if *message_idx == 0 && *len >= 64usize
        ) {
            found_ref = true;
            break;
        }
    }
    assert!(found_ref, "expected a coalesced reference of length >= 64");
}
