import pytest


def test_whitespace_tokenizer_roundtrip_texts_and_tokens():
    import copyforward

    messages = [
        "Hello world from Alice",
        "Hello world from Alice — how are you?",
        "Alice says: Hello world from Alice",
    ]

    # Build via tokenizer opt-in (tokens API)
    cf = copyforward.CopyForwardTokens.from_texts_with_tokenizer(messages, tokenizer="whitespace", exact_mode=True)

    # render() returns token ids; render_texts() decodes to original messages
    tokens = cf.render()
    assert isinstance(tokens, list)
    assert all(isinstance(t, list) for t in tokens)
    assert cf.render_texts() == messages

    # Building from the token ids should yield the same segments structure
    cf2 = copyforward.CopyForwardTokens.from_tokens(tokens, exact_mode=True)
    assert cf2.render() == tokens
    # Compare segment structures - they should have the same structure
    segs1 = cf.segments()
    segs2 = cf2.segments()
    assert len(segs1) == len(segs2)
    for msg_segs1, msg_segs2 in zip(segs1, segs2):
        assert len(msg_segs1) == len(msg_segs2)
        for seg1, seg2 in zip(msg_segs1, msg_segs2):
            assert type(seg1) == type(seg2)
            if hasattr(seg1, 'tokens'):
                assert seg1.tokens == seg2.tokens
            else:
                assert seg1.message == seg2.message
                assert seg1.start == seg2.start
                assert seg1.len == seg2.len


def test_render_texts_errors_without_tokenizer():
    import copyforward

    tokens = [[1, 2, 3], [1, 2, 3, 4]]
    cf = copyforward.CopyForwardTokens.from_tokens(tokens)
    with pytest.raises(TypeError):
        _ = cf.render_texts()


def test_feature_errors_when_missing_tokenizers_feature():
    import copyforward

    msgs = ["a", "b"]
    # These names require optional features; verify we emit a clear error.
    with pytest.raises(TypeError) as e1:
        copyforward.CopyForwardTokens.from_texts_with_tokenizer(msgs, tokenizer="hf:distilbert-base-uncased")
    assert "requires building" in str(e1.value)

    with pytest.raises(TypeError) as e2:
        copyforward.CopyForwardTokens.from_texts_with_tokenizer(msgs, tokenizer="file:/not/a/real/tokenizer.json")
    assert "requires building" in str(e2.value)
