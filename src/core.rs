#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    Literal(String),
    /// Reference to a substring of a previous message: (message_idx, start, len)
    Reference {
        message_idx: usize,
        start: usize,
        len: usize,
    },
}

/// Trait describing a copy-forwarding algorithm implementation.
pub trait CopyForward {
    /// Construct the algorithm-specific representation from messages.
    fn from_messages(messages: &[&str]) -> Self
    where
        Self: Sized;

    /// Return a cloned `Vec<Vec<Segment>>` representation of the internal model.
    fn segments(&self) -> Vec<Vec<Segment>>;

    /// Render each message into a `String`, calling `replacer` for every
    /// `Reference` segment with `(message_idx, start, len, referenced_text)`.
    fn render_with<F>(&self, replacer: F) -> Vec<String>
    where
        F: FnMut(usize, usize, usize, &str) -> String;

    /// Convenience to render with a static replacement string for every
    /// `Reference` segment.
    fn render_with_static(&self, replacement: &str) -> Vec<String>
    where
        Self: Sized,
    {
        self.render_with(|_, _, _, _| replacement.to_string())
    }
}

/// Configuration for `GreedySubstring` algorithm.
#[derive(Debug, Clone)]
pub struct GreedySubstringConfig {
    pub min_match_len: usize,
    /// lookback window: only consider previous `lookback` messages
    /// when matching. `None` means consider all previous messages.
    pub lookback: Option<usize>,
}

impl Default for GreedySubstringConfig {
    fn default() -> Self {
        GreedySubstringConfig {
            min_match_len: 4,
            lookback: None,
        }
    }
}
