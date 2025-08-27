/// A segment of a compressed message - either literal text or a reference.
///
/// Messages are compressed into sequences of segments. References point to
/// substrings of earlier messages to avoid storing duplicate text.
///
/// # Example
/// ```
/// use copyforward::Segment;
///
/// let literal = Segment::Literal("Hello ".to_string());
/// let reference = Segment::Reference {
///     message_idx: 0,
///     start: 6,
///     len: 5
/// }; // Points to "world" in message 0
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// Literal text that appears directly in the compressed message.
    Literal(String),
    /// Reference to a substring of a previous message.
    ///
    /// Points to `messages[message_idx][start..start+len]`.
    Reference {
        /// Index of the referenced message (must be < current message index).
        message_idx: usize,
        /// Byte offset where the referenced substring starts.
        start: usize,
        /// Length in bytes of the referenced substring.
        len: usize,
    },
}

/// A segment of a compressed token sequence (u32 token IDs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenSegment {
    /// Literal token span that appears directly in the compressed message.
    Literal(Vec<u32>),
    /// Reference to a subspan of a previous tokenized message.
    Reference {
        /// Index of the referenced message (must be < current message index).
        message_idx: usize,
        /// Token index where the referenced subspan starts.
        start: usize,
        /// Length in tokens of the referenced subspan.
        len: usize,
    },
}

/// Common interface for copy-forward compression algorithms.
///
/// Implementations compress message sequences into [`Segment`] representations
/// and provide rendering methods to reconstruct the original text.
///
/// # Example
/// ```
/// use copyforward::{CopyForward, exact, Config};
///
/// let messages = &["Hello world", "Hello world, how are you?"];
/// let compressed = exact(messages, Config::default());
///
/// // Get the compressed representation
/// let segments = compressed.segments();
///
/// // Render back to original text
/// let original = compressed.render_with(|_, _, _, text| text.to_string());
/// assert_eq!(original, messages);
/// ```
pub trait CopyForward {
    /// Get the compressed segment representation.
    ///
    /// Returns a vector where each element corresponds to one input message,
    /// containing the segments that make up that compressed message.
    ///
    /// # Returns
    /// Vector of segment vectors, one per input message.
    fn segments(&self) -> Vec<Vec<Segment>>;

    /// Render messages by calling a replacer function for each reference.
    ///
    /// For each [`Segment::Reference`], calls `replacer(message_idx, start, len, referenced_text)`
    /// and uses the returned string. [`Segment::Literal`] segments are included directly.
    ///
    /// # Parameters
    /// - `replacer`: Function called for each reference with (message_idx, start, len, text)
    ///
    /// # Returns
    /// Vector of rendered message strings.
    fn render_with<F>(&self, replacer: F) -> Vec<String>
    where
        F: FnMut(usize, usize, usize, &str) -> String;

    /// Render with a static replacement string for all references.
    ///
    /// Convenience method that replaces every [`Segment::Reference`] with the same string.
    /// Useful for debugging or creating redacted versions.
    ///
    /// # Example
    /// ```
    /// use copyforward::{exact, Config, CopyForward};
    ///
    /// let messages = &["Hello world", "Hello world today"];
    /// let compressed = exact(messages, Config::default());
    /// let redacted = compressed.render_with_static("[REFERENCE]");
    /// ```
    fn render_with_static(&self, replacement: &str) -> Vec<String> {
        self.render_with(|_, _, _, _| replacement.to_string())
    }
}

/// Copy-forward interface specialized for token sequences (u32 token IDs).
pub trait CopyForwardTokens {
    /// Get the compressed token segment representation.
    fn segments(&self) -> Vec<Vec<TokenSegment>>;

    /// Render token messages by calling a replacer for each reference.
    fn render_with<F>(&self, replacer: F) -> Vec<Vec<u32>>
    where
        F: FnMut(usize, usize, usize, &[u32]) -> Vec<u32>;

    /// Render with a static replacement for all references.
    fn render_with_static(&self, replacement: &[u32]) -> Vec<Vec<u32>> {
        self.render_with(|_, _, _, _| replacement.to_vec())
    }
}

/// Configuration for copy-forward compression algorithms.
///
/// Controls the behavior and performance characteristics of compression.
/// All parameters have sensible defaults for typical use cases.
///
/// # Example
/// ```
/// use copyforward::{Config, exact};
///
/// let config = Config {
///     min_match_len: 8,  // Only create references for 8+ byte matches
///     lookback: Some(10), // Only look at previous 10 messages
///     ..Config::default()
/// };
///
/// let compressed = exact(&["test"], config);
/// ```
#[derive(Debug, Clone)]
pub struct Config {
    /// Minimum match length required to create a reference.
    ///
    /// For token mode, this is measured in tokens. For text mode, this is
    /// measured in Unicode scalar values (characters), ensuring UTF-8-safe
    /// boundaries for all references and literals.
    ///
    /// **Default:** 4
    pub min_match_len: usize,

    /// Limit search to the most recent N messages.
    ///
    /// `None` considers all previous messages. Limiting lookback can improve
    /// speed for very long message sequences at the cost of some compression.
    ///
    /// **Default:** None (unlimited)
    pub lookback: Option<usize>,

    /// Maximum extension length for approximate algorithms (internal tuning).
    ///
    /// Controls speed vs accuracy tradeoff in [`crate::approximate()`]. Ignored by [`crate::exact()`].
    /// For token mode this is measured in tokens; for text mode, in Unicode
    /// scalar values (characters).
    ///
    /// **Default:** 64
    pub cap_len: usize,

    /// Maximum candidates examined per lookup in approximate algorithms (internal tuning).
    ///
    /// Limits worst-case performance when many matches exist. Ignored by [`crate::exact()`].
    ///
    /// **Default:** 64 candidates
    pub ncap: usize,
}

impl Default for Config {
    /// Creates a configuration with balanced defaults for typical use cases.
    fn default() -> Self {
        Config {
            min_match_len: 4,
            lookback: None,
            cap_len: 64,
            ncap: 64,
        }
    }
}
