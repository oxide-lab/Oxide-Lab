//! Streaming parser for `<think>` tags (Chain of Thought).
//!
//! State machine that separates model output into `thinking` and `content` channels.
//! Based on Ollama's thinking parser implementation with full parity.
//!
//! Key behaviors:
//! - If non-whitespace content appears BEFORE `<think>`, thinking is skipped entirely
//! - Whitespace between tags and content is trimmed
//! - Partial tags are buffered until disambiguated
//! - Only the first `<think>...</think>` block is treated as thinking
//! - Trailing whitespace is buffered to handle ambiguous tag boundaries

use serde::{Deserialize, Serialize};

const THINK_OPEN: &str = "<think>";
const THINK_CLOSE: &str = "</think>";

/// Parser state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThinkingState {
    /// Looking for opening tag, haven't seen non-whitespace yet
    LookingForOpening,
    /// Seen opening tag, eating whitespace before thinking content
    ThinkingStartedEatingWhitespace,
    /// Inside thinking block, collecting thinking content
    CollectingThinking,
    /// Seen closing tag, eating whitespace before content
    ThinkingDoneEatingWhitespace,
    /// Thinking complete, collecting regular content
    CollectingContent,
}

impl std::fmt::Display for ThinkingState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LookingForOpening => write!(f, "LookingForOpening"),
            Self::ThinkingStartedEatingWhitespace => write!(f, "ThinkingStartedEatingWhitespace"),
            Self::CollectingThinking => write!(f, "CollectingThinking"),
            Self::ThinkingDoneEatingWhitespace => write!(f, "ThinkingDoneEatingWhitespace"),
            Self::CollectingContent => write!(f, "CollectingContent"),
        }
    }
}

/// Result of parsing a token chunk.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParsedChunk {
    pub thinking: String,
    pub content: String,
}

impl ParsedChunk {
    pub fn is_empty(&self) -> bool {
        self.thinking.is_empty() && self.content.is_empty()
    }
}

/// Parser event types (internal)
#[derive(Debug, Clone)]
enum ParseEvent {
    Thinking(String),
    Content(String),
}

/// Streaming parser for `<think>` tags.
///
/// Ported from Ollama's Go implementation with full parity.
pub struct ThinkingParser {
    state: ThinkingState,
    opening_tag: String,
    closing_tag: String,
    /// Accumulator buffer for partial content
    buffer: String,
}

impl ThinkingParser {
    pub fn new() -> Self {
        Self {
            state: ThinkingState::LookingForOpening,
            opening_tag: THINK_OPEN.to_string(),
            closing_tag: THINK_CLOSE.to_string(),
            buffer: String::new(),
        }
    }

    /// Create parser starting in thinking mode (for implicit thinking models).
    /// Use this when the prompt already ends with `<think>`.
    pub fn new_in_thinking_mode() -> Self {
        Self {
            state: ThinkingState::CollectingThinking,
            opening_tag: THINK_OPEN.to_string(),
            closing_tag: THINK_CLOSE.to_string(),
            buffer: String::new(),
        }
    }

    /// Create parser with custom tags.
    pub fn with_tags(opening: &str, closing: &str) -> Self {
        Self {
            state: ThinkingState::LookingForOpening,
            opening_tag: opening.to_string(),
            closing_tag: closing.to_string(),
            buffer: String::new(),
        }
    }

    /// Process incoming token and return parsed chunk.
    ///
    /// Returns thinking content and non-thinking content that should be
    /// immediately sent to the user. Internally buffers if needed to
    /// disambiguate partial tags.
    pub fn process_token(&mut self, token: &str) -> ParsedChunk {
        self.buffer.push_str(token);

        let events = self.parse_events();

        let mut thinking = String::new();
        let mut content = String::new();

        for event in events {
            match event {
                ParseEvent::Thinking(s) => thinking.push_str(&s),
                ParseEvent::Content(s) => content.push_str(&s),
            }
        }

        ParsedChunk { thinking, content }
    }

    /// Parse and emit all unambiguous events from the buffer.
    fn parse_events(&mut self) -> Vec<ParseEvent> {
        let mut all = Vec::new();

        loop {
            let (events, keep_looping) = self.eat();
            all.extend(events);
            if !keep_looping {
                break;
            }
        }

        all
    }

    /// Consume buffer and return unambiguous events.
    /// Returns (events, should_continue_looping)
    fn eat(&mut self) -> (Vec<ParseEvent>, bool) {
        let mut events = Vec::new();
        let buf_str = self.buffer.clone();

        if buf_str.is_empty() {
            return (events, false);
        }

        match self.state {
            ThinkingState::LookingForOpening => {
                let trimmed = buf_str.trim_start();

                if trimmed.starts_with(&self.opening_tag) {
                    // Found opening tag
                    let after = trimmed
                        .strip_prefix(&self.opening_tag)
                        .unwrap_or("")
                        .trim_start()
                        .to_string();

                    self.buffer.clear();
                    self.buffer.push_str(&after);

                    if after.is_empty() {
                        self.state = ThinkingState::ThinkingStartedEatingWhitespace;
                    } else {
                        self.state = ThinkingState::CollectingThinking;
                    }
                    (events, true)
                } else if self.opening_tag.starts_with(trimmed) && !trimmed.is_empty() {
                    // Partial opening tag seen, keep accumulating
                    (events, false)
                } else if trimmed.is_empty() {
                    // Whitespace only, keep accumulating
                    (events, false)
                } else {
                    // Non-whitespace content before opening tag - skip thinking entirely
                    self.state = ThinkingState::CollectingContent;
                    let content = std::mem::take(&mut self.buffer);
                    events.push(ParseEvent::Content(content));
                    (events, false)
                }
            }

            ThinkingState::ThinkingStartedEatingWhitespace => {
                let trimmed = buf_str.trim_start().to_string();
                self.buffer.clear();

                if trimmed.is_empty() {
                    (events, false)
                } else {
                    self.state = ThinkingState::CollectingThinking;
                    self.buffer.push_str(&trimmed);
                    (events, true)
                }
            }

            ThinkingState::CollectingThinking => {
                if buf_str.contains(&self.closing_tag) {
                    // Found closing tag
                    let parts: Vec<&str> = buf_str.splitn(2, &self.closing_tag).collect();
                    let thinking = parts[0].trim_end().to_string();
                    let remaining = parts
                        .get(1)
                        .map(|s| s.trim_start())
                        .unwrap_or("")
                        .to_string();

                    self.buffer.clear();

                    if !thinking.is_empty() {
                        events.push(ParseEvent::Thinking(thinking));
                    }

                    if remaining.is_empty() {
                        self.state = ThinkingState::ThinkingDoneEatingWhitespace;
                    } else {
                        self.state = ThinkingState::CollectingContent;
                        self.buffer.push_str(&remaining);
                    }

                    (events, true)
                } else if let Some(overlap_len) = overlap(&buf_str, &self.closing_tag) {
                    // Partial closing tag at end - buffer ambiguous part
                    let before_partial = &buf_str[..buf_str.len() - overlap_len];
                    let trailing_ws_len = trailing_whitespace_len(before_partial);
                    let ambiguous_start = before_partial.len() - trailing_ws_len;

                    let unambiguous = &buf_str[..ambiguous_start];
                    let ambiguous = &buf_str[ambiguous_start..];

                    self.buffer.clear();
                    self.buffer.push_str(ambiguous);

                    if !unambiguous.is_empty() {
                        events.push(ParseEvent::Thinking(unambiguous.to_string()));
                    }

                    (events, false)
                } else {
                    // Pure thinking content, but withhold trailing whitespace
                    let ws_len = trailing_whitespace_len(&buf_str);
                    let ambiguous_start = buf_str.len() - ws_len;

                    let unambiguous = &buf_str[..ambiguous_start];
                    let ambiguous = &buf_str[ambiguous_start..];

                    self.buffer.clear();
                    self.buffer.push_str(ambiguous);

                    if !unambiguous.is_empty() {
                        events.push(ParseEvent::Thinking(unambiguous.to_string()));
                    }

                    (events, false)
                }
            }

            ThinkingState::ThinkingDoneEatingWhitespace => {
                let trimmed = buf_str.trim_start().to_string();
                self.buffer.clear();

                if !trimmed.is_empty() {
                    self.state = ThinkingState::CollectingContent;
                    self.buffer.push_str(&trimmed);
                }

                (events, !trimmed.is_empty())
            }

            ThinkingState::CollectingContent => {
                // In content mode, just emit everything
                self.buffer.clear();
                if !buf_str.is_empty() {
                    events.push(ParseEvent::Content(buf_str));
                }
                (events, false)
            }
        }
    }

    /// Flush any remaining buffered content.
    /// Call this when the stream is done to emit any buffered data.
    pub fn flush(&mut self) -> ParsedChunk {
        if self.buffer.is_empty() {
            return ParsedChunk::default();
        }

        let buf = std::mem::take(&mut self.buffer);

        match self.state {
            ThinkingState::CollectingThinking | ThinkingState::ThinkingStartedEatingWhitespace => {
                ParsedChunk {
                    thinking: buf,
                    content: String::new(),
                }
            }
            ThinkingState::LookingForOpening => {
                // If we never found an opening tag, all buffered content goes to content
                // (this handles whitespace-only buffers)
                ParsedChunk {
                    thinking: String::new(),
                    content: buf,
                }
            }
            _ => ParsedChunk {
                thinking: String::new(),
                content: buf,
            },
        }
    }

    pub fn state(&self) -> ThinkingState {
        self.state
    }

    pub fn is_in_thinking_mode(&self) -> bool {
        matches!(
            self.state,
            ThinkingState::CollectingThinking | ThinkingState::ThinkingStartedEatingWhitespace
        )
    }
}

impl Default for ThinkingParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Find longest overlap between suffix of s and prefix of delim.
/// Returns the overlap length, or None if no overlap.
fn overlap(s: &str, delim: &str) -> Option<usize> {
    let max = std::cmp::min(delim.len(), s.len());
    (1..=max).rev().find(|&i| s.ends_with(&delim[..i]))
}

/// Count trailing whitespace bytes in a string.
/// Properly handles UTF-8 by iterating backwards over chars.
fn trailing_whitespace_len(s: &str) -> usize {
    let mut total = 0;
    for c in s.chars().rev() {
        if c.is_whitespace() {
            total += c.len_utf8();
        } else {
            break;
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_think_block() {
        let mut parser = ThinkingParser::new();
        let result = parser.process_token("<think> internal </think> world");
        assert_eq!(result.thinking, "internal");
        assert_eq!(result.content, "world");
    }

    #[test]
    fn handles_content_only() {
        let mut parser = ThinkingParser::new();
        let result = parser.process_token("just plain content");
        assert_eq!(result.thinking, "");
        assert_eq!(result.content, "just plain content");
    }

    #[test]
    fn content_before_think_nerfs_thinking() {
        let mut parser = ThinkingParser::new();
        let result = parser.process_token("  abc <think>def</think> ghi");
        // Content before <think> means thinking is skipped
        assert_eq!(result.thinking, "");
        assert_eq!(result.content, "  abc <think>def</think> ghi");
    }

    #[test]
    fn nested_think_in_content() {
        let mut parser = ThinkingParser::new();
        let result = parser.process_token("<think>a</think><think>b</think>c");
        // Only first <think> block is treated as thinking
        assert_eq!(result.thinking, "a");
        assert_eq!(result.content, "<think>b</think>c");
    }

    #[test]
    fn partial_opening_tag() {
        let mut parser = ThinkingParser::new();

        let r1 = parser.process_token("  <th");
        assert_eq!(r1.content, "");
        assert_eq!(r1.thinking, "");
        assert_eq!(parser.state(), ThinkingState::LookingForOpening);

        let r2 = parser.process_token("in");
        assert_eq!(r2.content, "");
        assert_eq!(r2.thinking, "");

        let r3 = parser.process_token("k>a");
        assert_eq!(r3.thinking, "a");
        assert_eq!(r3.content, "");
        assert_eq!(parser.state(), ThinkingState::CollectingThinking);
    }

    #[test]
    fn partial_closing_tag() {
        let mut parser = ThinkingParser::new();

        let r1 = parser.process_token("<think>abc</th");
        assert_eq!(r1.thinking, "abc");
        assert_eq!(r1.content, "");
        assert_eq!(parser.state(), ThinkingState::CollectingThinking);

        let r2 = parser.process_token("ink>def");
        assert_eq!(r2.thinking, "");
        assert_eq!(r2.content, "def");
        assert_eq!(parser.state(), ThinkingState::CollectingContent);
    }

    #[test]
    fn partial_closing_tag_fakeout() {
        let mut parser = ThinkingParser::new();

        let r1 = parser.process_token("<think>abc</th");
        assert_eq!(r1.thinking, "abc");

        // </thing> is not the closing tag
        let r2 = parser.process_token("ing>def");
        assert_eq!(r2.thinking, "</thing>def");
        assert_eq!(r2.content, "");
        assert_eq!(parser.state(), ThinkingState::CollectingThinking);

        let r3 = parser.process_token("ghi</thi");
        assert_eq!(r3.thinking, "ghi");

        let r4 = parser.process_token("nk>jkl");
        assert_eq!(r4.content, "jkl");
        assert_eq!(parser.state(), ThinkingState::CollectingContent);
    }

    #[test]
    fn whitespace_after_thinking_tag() {
        let mut parser = ThinkingParser::new();
        let result = parser.process_token("  <think>abc</think>\n\ndef");
        assert_eq!(result.thinking, "abc");
        assert_eq!(result.content, "def");
    }

    #[test]
    fn whitespace_handling_incremental() {
        let mut parser = ThinkingParser::new();

        let r1 = parser.process_token("  <think>abc</think>");
        assert_eq!(r1.thinking, "abc");
        assert_eq!(r1.content, "");
        assert_eq!(parser.state(), ThinkingState::ThinkingDoneEatingWhitespace);

        let r2 = parser.process_token("\n\ndef");
        assert_eq!(r2.thinking, "");
        assert_eq!(r2.content, "def");
        assert_eq!(parser.state(), ThinkingState::CollectingContent);
    }

    #[test]
    fn token_by_token() {
        let mut parser = ThinkingParser::new();

        let r1 = parser.process_token("<think>");
        assert_eq!(r1.thinking, "");
        assert_eq!(r1.content, "");
        assert_eq!(
            parser.state(),
            ThinkingState::ThinkingStartedEatingWhitespace
        );

        let r2 = parser.process_token("\n");
        assert_eq!(r2.thinking, "");
        assert_eq!(r2.content, "");

        let r3 = parser.process_token("</think>");
        assert_eq!(r3.thinking, "");
        assert_eq!(parser.state(), ThinkingState::ThinkingDoneEatingWhitespace);

        let r4 = parser.process_token("\n\n");
        assert_eq!(r4.content, "");

        let r5 = parser.process_token("Hi");
        assert_eq!(r5.content, "Hi");
        assert_eq!(parser.state(), ThinkingState::CollectingContent);

        let r6 = parser.process_token(" there");
        assert_eq!(r6.content, " there");
    }

    #[test]
    fn leading_thinking_whitespace() {
        let mut parser = ThinkingParser::new();

        let r1 = parser.process_token("  <think>   \t ");
        assert_eq!(r1.thinking, "");
        assert_eq!(
            parser.state(),
            ThinkingState::ThinkingStartedEatingWhitespace
        );

        let r2 = parser.process_token("  these are some ");
        // Trailing space is buffered, so only "these are some" is emitted
        assert_eq!(r2.thinking, "these are some");
        assert_eq!(parser.state(), ThinkingState::CollectingThinking);

        let r3 = parser.process_token("thoughts </think>  ");
        // Buffered " " from r2 is now emitted with "thoughts"
        assert_eq!(r3.thinking, " thoughts");
        assert_eq!(parser.state(), ThinkingState::ThinkingDoneEatingWhitespace);

        let r4 = parser.process_token("  more content");
        assert_eq!(r4.content, "more content");
        assert_eq!(parser.state(), ThinkingState::CollectingContent);
    }

    #[test]
    fn implicit_thinking_mode() {
        let mut parser = ThinkingParser::new_in_thinking_mode();

        let r1 = parser.process_token("some reasoning");
        assert_eq!(r1.thinking, "some reasoning");
        assert_eq!(r1.content, "");

        let r2 = parser.process_token("</think>answer");
        assert_eq!(r2.thinking, "");
        assert_eq!(r2.content, "answer");
    }

    #[test]
    fn flush_empties_buffer() {
        let mut parser = ThinkingParser::new();
        let result = parser.process_token("text<thi");

        // "text" is non-whitespace before <think>, so thinking is skipped
        // and all content is output immediately
        assert_eq!(result.content, "text<thi");
        assert_eq!(result.thinking, "");
        assert_eq!(parser.state(), ThinkingState::CollectingContent);

        // Buffer should be empty after
        let flushed = parser.flush();
        assert_eq!(flushed.content, "");
    }

    #[test]
    fn flush_in_thinking_mode() {
        let mut parser = ThinkingParser::new_in_thinking_mode();
        let result = parser.process_token("partial thinking");

        // In thinking mode without closing tag, content is output immediately
        assert_eq!(result.thinking, "partial thinking");
        assert_eq!(result.content, "");

        // Flush should be empty
        let flushed = parser.flush();
        assert_eq!(flushed.thinking, "");
    }

    #[test]
    fn flush_uncommitted_thinking() {
        let mut parser = ThinkingParser::new_in_thinking_mode();
        let _ = parser.process_token("abc ");

        // Trailing whitespace is buffered
        let flushed = parser.flush();
        assert_eq!(flushed.thinking, " ");
    }

    #[test]
    fn trailing_whitespace_buffered() {
        let mut parser = ThinkingParser::new_in_thinking_mode();

        // Trailing whitespace should be buffered (ambiguous)
        let r1 = parser.process_token("thinking ");
        assert_eq!(r1.thinking, "thinking");

        // More content disambiguates
        let r2 = parser.process_token("more");
        assert_eq!(r2.thinking, " more");
    }

    #[test]
    fn custom_tags() {
        let mut parser = ThinkingParser::with_tags("<reasoning>", "</reasoning>");

        let result = parser.process_token("<reasoning>thoughts</reasoning>answer");
        assert_eq!(result.thinking, "thoughts");
        assert_eq!(result.content, "answer");
    }

    #[test]
    fn overlap_function() {
        assert_eq!(overlap("hello", "<tool"), None);
        assert_eq!(overlap("hello<tool", "<tool>"), Some(5));
        assert_eq!(overlap("hello<to", "<tool>"), Some(3));
        assert_eq!(overlap("abc</th", "</think>"), Some(4));
        assert_eq!(overlap("abc</think", "</think>"), Some(7));
    }

    #[test]
    fn trailing_whitespace_len_function() {
        assert_eq!(trailing_whitespace_len("hello"), 0);
        assert_eq!(trailing_whitespace_len("hello "), 1);
        assert_eq!(trailing_whitespace_len("hello  \t\n"), 4);
        assert_eq!(trailing_whitespace_len("  "), 2);
        assert_eq!(trailing_whitespace_len(""), 0);
    }
}
