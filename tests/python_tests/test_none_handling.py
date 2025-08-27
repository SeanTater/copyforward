"""Test None value handling in Python bindings."""
import pytest

def test_text_with_none_values():
    """Test that text API handles None values correctly."""
    import copyforward
    
    messages = ["hello world", None, "hello world today"]
    cf = copyforward.CopyForwardText.from_texts(messages, exact_mode=True)
    
    segments = cf.segments()
    assert len(segments) == 3
    # First message should have segments
    assert len(segments[0]) > 0
    # Second message (None) should have empty segments
    assert len(segments[1]) == 0
    # Third message should have segments
    assert len(segments[2]) > 0
    
    rendered = cf.render("[REF]")
    assert len(rendered) == 3
    assert rendered[0] == "hello world"
    assert rendered[1] is None  # None should stay None
    assert rendered[2] == "[REF] today"

def test_text_approximate_with_none():
    """Test approximate mode with None values."""
    import copyforward
    
    messages = ["hello", None, "world", None, "hello world"]
    cf = copyforward.CopyForwardText.from_texts(messages, exact_mode=False)
    
    segments = cf.segments()
    assert len(segments) == 5
    assert len(segments[1]) == 0  # None entry
    assert len(segments[3]) == 0  # None entry
    
    rendered = cf.render("[REF]")
    assert len(rendered) == 5
    assert rendered[0] == "hello"
    assert rendered[1] is None
    assert rendered[2] == "world"
    assert rendered[3] is None
    assert rendered[4] == "hello [REF]"

def test_tokens_with_none_values():
    """Test that token API handles None values correctly."""
    import copyforward
    
    messages = [[10, 11, 12], None, [10, 11, 12, 13]]
    cf = copyforward.CopyForwardTokens.from_tokens(messages, exact_mode=True)
    
    segments = cf.segments()
    # Should have segments only for non-None entries
    assert len(segments) == 2
    
    # Render should work - but this is tricky since we don't know the original indices
    # The token API doesn't track None positions the same way as text API
    rendered = cf.render([999])  # Use token 999 as replacement
    assert len(rendered) == 2  # Only non-None entries

def test_mixed_none_all_none():
    """Test edge cases with all None or mixed None."""
    import copyforward
    
    # All None
    messages = [None, None, None]
    cf = copyforward.CopyForwardText.from_texts(messages, exact_mode=True)
    rendered = cf.render("[REF]")
    assert len(rendered) == 3
    assert all(r is None for r in rendered)
    
    # Mostly None
    messages = [None, "hello", None]
    cf = copyforward.CopyForwardText.from_texts(messages, exact_mode=True)
    rendered = cf.render("[REF]")
    assert len(rendered) == 3
    assert rendered[0] is None
    assert rendered[1] == "hello"
    assert rendered[2] is None

def test_compression_ratio_with_none():
    """Test that compression ratio calculation works with None values."""
    import copyforward
    
    messages = ["hello world", None, "hello world again"]
    cf = copyforward.CopyForwardText.from_texts(messages, exact_mode=True)
    
    ratio = cf.compression_ratio()
    assert isinstance(ratio, float)
    assert ratio > 0  # Should have some compression even with None values