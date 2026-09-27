"""Round-trip, empty-string, and lookback coverage for the Python bindings."""


def _reassemble_text(segments, messages):
    """Rebuild each message from its segments, resolving references."""
    out = []
    for msg_segments in segments:
        parts = []
        for seg in msg_segments:
            if hasattr(seg, "text"):
                parts.append(seg.text)
            else:
                parts.append(messages[seg.message][seg.start : seg.start + seg.len])
        out.append("".join(parts))
    return out


def test_empty_string_is_distinct_from_none():
    import copyforward

    messages = ["", "hello world", None, "hello world"]
    cf = copyforward.CopyForwardText.from_texts(messages, engine="greedy")
    rendered = cf.render("[REF]")
    # The first message is an empty string, not None: it must survive.
    assert rendered == ["", "hello world", None, "[REF]"]
    assert rendered[0] == ""
    assert rendered[0] is not None


def test_segments_reassemble_to_original_text():
    import copyforward

    messages = [
        "hello world",
        "hello world again",
        None,
        "",
        "world hello world",
    ]
    for engine in ["greedy", "capped"]: 
        cf = copyforward.CopyForwardText.from_texts(messages, engine=engine)
        segments = cf.segments()
        # None entries have no segments; resolve references against the
        # original messages.
        rebuilt = _reassemble_text(
            [segs for segs, m in zip(segments, messages) if m is not None],
            [m for m in messages if m is not None],
        )
        assert rebuilt == [m for m in messages if m is not None]


def test_unicode_round_trip():
    import copyforward

    messages = ["héllo wörld 🌍", "🌍 héllo wörld", "café 🌍 héllo"]
    for engine in ["greedy", "capped"]: 
        cf = copyforward.CopyForwardText.from_texts(messages, engine=engine)
        assert _reassemble_text(cf.segments(), messages) == messages


def test_lookback_limits_references():
    import copyforward

    messages = ["unique alpha content", "totally different words", "unique alpha content"]
    for engine in ["greedy", "capped"]: 
        cf = copyforward.CopyForwardText.from_texts(messages, engine=engine, lookback=1)
        assert cf.render("[R]") == messages

        cf = copyforward.CopyForwardText.from_texts(messages, engine=engine, lookback=2)
        assert cf.render("[R]")[2] == "[R]"


def test_token_segments_reassemble_to_original():
    import copyforward

    msgs = [[1, 2, 3, 4, 5, 6, 7, 8], [9, 1, 2, 3, 4, 5, 6]]
    for engine in ["greedy", "capped"]: 
        cf = copyforward.CopyForwardTokens.from_tokens(msgs, engine=engine)
        segs = cf.segments()
        for i, msg_segs in enumerate(segs):
            rebuilt = []
            for seg in msg_segs:
                if hasattr(seg, "tokens"):
                    rebuilt.extend(seg.tokens)
                else:
                    rebuilt.extend(msgs[seg.message][seg.start : seg.start + seg.len])
            assert rebuilt == msgs[i], f"message {i} segments do not reassemble"


def test_empty_token_message_segments():
    import copyforward

    msgs = [[1, 2, 3, 4], []]
    cf = copyforward.CopyForwardTokens.from_tokens(msgs, engine="greedy")
    segs = cf.segments()
    assert len(segs) == 2
    assert segs[1] == []
