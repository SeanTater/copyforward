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
    tokens = cf.render([9999])  # Use token 9999 as replacement
    assert isinstance(tokens, list)
    assert all(isinstance(t, list) for t in tokens)
    assert cf.render_texts("[REF]") == ["Hello world from Alice", "[REF] — how are you?", "Alice says: [REF]"]

    # Building from the rendered token ids should work but will have different structure
    # because the tokens already have replacements applied
    cf2 = copyforward.CopyForwardTokens.from_tokens(tokens, exact_mode=True)
    assert cf2.render([9999]) == tokens
    # cf2's segments will differ from cf's because cf2 was built from pre-replaced tokens
    segs2 = cf2.segments()
    assert len(segs2) == 3  # should have 3 messages


def test_render_texts_errors_without_tokenizer():
    import copyforward

    tokens = [[1, 2, 3], [1, 2, 3, 4]]
    cf = copyforward.CopyForwardTokens.from_tokens(tokens)
    with pytest.raises(TypeError):
        _ = cf.render_texts("[REF]")


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
