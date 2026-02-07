//! Streaming parser for tool calls in LLM output.
//!
//! State machine that extracts function calls from model output.
//! Based on Ollama's tools/tools.go implementation.
//!
//! Key behaviors:
//! - Looks for tool calling tags (e.g., `{`, `[`, or custom tags)
//! - Parses function name and JSON arguments in streaming fashion
//! - Buffers partial JSON until complete object is found
//! - For `{` or `[` tags, only parses if first non-whitespace matches

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Tool definition for function calling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub function: ToolFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunction {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub parameters: Option<serde_json::Value>,
}

/// Parsed tool call result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: HashMap<String, serde_json::Value>,
    pub index: usize,
}

/// Parser state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolsState {
    /// Looking for tool calling tag
    LookingForTag,
    /// Found tag, parsing tool calls
    ToolCalling,
    /// Done parsing (either found all tools or gave up)
    Done,
}

/// Result of parsing a chunk.
#[derive(Debug, Clone, Default)]
pub struct ParseResult {
    pub calls: Vec<ToolCall>,
    pub content: String,
}

/// Streaming parser for tool calls.
pub struct ToolCallParser {
    tag: String,
    tools: Vec<Tool>,
    state: ToolsState,
    buffer: Vec<u8>,
    call_count: usize,
}

impl ToolCallParser {
    /// Create a new parser with the given tools and tag.
    pub fn new(tools: Vec<Tool>, tag: &str) -> Self {
        Self {
            tag: tag.to_string(),
            tools,
            state: ToolsState::LookingForTag,
            buffer: Vec::new(),
            call_count: 0,
        }
    }

    /// Create parser with default JSON object tag `{`.
    pub fn with_json_tag(tools: Vec<Tool>) -> Self {
        Self::new(tools, "{")
    }

    /// Process incoming string and return parsed tool calls and remaining content.
    pub fn add(&mut self, s: &str) -> ParseResult {
        if self.state == ToolsState::Done {
            return ParseResult {
                calls: vec![],
                content: s.to_string(),
            };
        }

        self.buffer.extend_from_slice(s.as_bytes());

        let mut result = ParseResult::default();

        if self.state == ToolsState::LookingForTag {
            let (idx, found) = self.find_tag();

            if idx == -1 {
                // No tag found, return buffer as content
                result.content = String::from_utf8_lossy(&self.buffer).to_string();
                self.buffer.clear();
            } else {
                // Return content before tag
                let idx = idx as usize;
                result.content = String::from_utf8_lossy(&self.buffer[..idx]).to_string();
                self.buffer = self.buffer[idx..].to_vec();
            }

            // For { or [ tags, only parse if first non-whitespace matches
            if (self.tag == "{" || self.tag == "[") && !result.content.trim().is_empty() {
                self.state = ToolsState::Done;
                let remaining = String::from_utf8_lossy(&self.buffer).to_string();
                result.content.push_str(&remaining);
                self.buffer.clear();
                return result;
            }

            if !found {
                return result;
            }

            self.state = ToolsState::ToolCalling;
        }

        // Parse tool calls
        while let Some(call) = self.parse_tool_call() {
            result.calls.push(call);
        }

        // Check if done
        if self.is_done() {
            self.state = ToolsState::Done;
            result
                .content
                .push_str(&String::from_utf8_lossy(&self.buffer));
            self.buffer.clear();
        }

        result
    }

    /// Search buffer for tag, returns (index, found).
    /// Index is position of tag or partial match, found indicates complete tag.
    fn find_tag(&self) -> (i32, bool) {
        let tag_bytes = self.tag.as_bytes();

        // Check for complete tag
        if let Some(i) = self
            .buffer
            .windows(tag_bytes.len())
            .position(|w| w == tag_bytes)
        {
            return (i as i32, true);
        }

        // Check for partial suffix overlap
        let max = std::cmp::min(self.buffer.len(), tag_bytes.len());
        for i in (1..=max).rev() {
            if self.buffer.ends_with(&tag_bytes[..i]) {
                return ((self.buffer.len() - i) as i32, false);
            }
        }

        (-1, false)
    }

    /// Try to parse a complete tool call from buffer.
    fn parse_tool_call(&mut self) -> Option<ToolCall> {
        let (tool, end) = self.find_tool()?;

        let (args, args_end) = self.find_arguments(tool)?;

        let final_end = std::cmp::max(end, args_end);

        let call = ToolCall {
            id: format!("call_{}", self.call_count),
            function: ToolCallFunction {
                name: tool.function.name.clone(),
                arguments: args,
                index: self.call_count,
            },
        };

        self.call_count += 1;
        self.buffer = self.buffer[final_end..].to_vec();

        Some(call)
    }

    /// Find first matching tool name in buffer.
    fn find_tool(&self) -> Option<(&Tool, usize)> {
        if self.buffer.is_empty() {
            return None;
        }

        let buf_str = String::from_utf8_lossy(&self.buffer);

        // Check for partial match at end (need to wait for more data)
        let mut longest_name = 0;
        for t in &self.tools {
            longest_name = longest_name.max(t.function.name.len());
        }

        for i in 1..=std::cmp::min(buf_str.len(), longest_name) {
            let tail = &buf_str[buf_str.len() - i..];
            for t in &self.tools {
                if t.function.name.len() > tail.len() && t.function.name.starts_with(tail) {
                    // Partial match at end, wait for more data
                    return None;
                }
            }
        }

        // Find first occurrence of longest matching tool name
        let mut best_tool: Option<&Tool> = None;
        let mut best_start = usize::MAX;
        let mut best_end = 0;

        for t in &self.tools {
            if let Some(pos) = buf_str.find(&t.function.name)
                && (pos < best_start
                    || (pos == best_start && t.function.name.len() > best_end - best_start))
            {
                best_tool = Some(t);
                best_start = pos;
                best_end = pos + t.function.name.len();
            }
        }

        best_tool.map(|t| (t, best_end))
    }

    /// Find JSON arguments object in buffer.
    fn find_arguments(&self, _tool: &Tool) -> Option<(HashMap<String, serde_json::Value>, usize)> {
        if self.buffer.is_empty() {
            return None;
        }

        let buf_str = String::from_utf8_lossy(&self.buffer);

        let mut start = None;
        let mut braces = 0;
        let mut in_string = false;
        let mut escaped = false;

        for (i, c) in buf_str.char_indices() {
            if escaped {
                escaped = false;
                continue;
            }

            if c == '\\' {
                escaped = true;
                continue;
            }

            if c == '"' {
                in_string = !in_string;
                continue;
            }

            if in_string {
                continue;
            }

            if c == '{' {
                if braces == 0 {
                    start = Some(i);
                }
                braces += 1;
            } else if c == '}' {
                braces -= 1;
                if braces == 0 && start.is_some() {
                    let start_idx = start.unwrap();
                    let object_str = &buf_str[start_idx..=i];

                    // Try to parse as JSON
                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(object_str) {
                        // Extract arguments from various formats
                        if let Some(args) = self.extract_arguments(&data) {
                            return Some((args, i + 1));
                        }
                        // If no structured format, use the whole object
                        if let Some(obj) = data.as_object() {
                            let args: HashMap<String, serde_json::Value> =
                                obj.clone().into_iter().collect();
                            return Some((args, i + 1));
                        }
                    }

                    // Not valid JSON, reset and keep looking
                    start = None;
                }

                if braces < 0 {
                    braces = 0;
                }
            }
        }

        None
    }

    /// Extract arguments from various JSON formats (e.g., nested "arguments" or "parameters").
    fn extract_arguments(
        &self,
        data: &serde_json::Value,
    ) -> Option<HashMap<String, serde_json::Value>> {
        // Check for {"name": "...", "arguments": {...}} format
        if data.get("name").is_some() {
            if let Some(args) = data.get("arguments") {
                if let Some(obj) = args.as_object() {
                    return Some(obj.clone().into_iter().collect());
                }
                // Handle string-encoded arguments
                if let Some(s) = args.as_str()
                    && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(s)
                    && let Some(obj) = parsed.as_object()
                {
                    return Some(obj.clone().into_iter().collect());
                }
            }
            if let Some(params) = data.get("parameters")
                && let Some(obj) = params.as_object()
            {
                return Some(obj.clone().into_iter().collect());
            }
            // Has name but no args = empty args
            return Some(HashMap::new());
        }

        None
    }

    /// Check if parsing is complete.
    fn is_done(&self) -> bool {
        if self.tag != "{" && self.tag != "[" {
            return false;
        }

        let mut count = 0i32;
        let (open, close) = if self.tag == "{" {
            (b'{', b'}')
        } else {
            (b'[', b']')
        };

        for &b in &self.buffer {
            if b == open {
                count += 1;
            } else if b == close {
                count -= 1;
                if count == 0 {
                    return true;
                }
            }
        }

        false
    }

    /// Get current buffer contents (for debugging).
    pub fn get_buffer(&self) -> String {
        String::from_utf8_lossy(&self.buffer).to_string()
    }

    /// Check if parser is in done state.
    pub fn is_finished(&self) -> bool {
        self.state == ToolsState::Done
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tool(name: &str) -> Tool {
        Tool {
            function: ToolFunction {
                name: name.to_string(),
                description: None,
                parameters: None,
            },
        }
    }

    #[test]
    fn test_no_tools_passthrough() {
        let mut parser = ToolCallParser::with_json_tag(vec![]);
        let result = parser.add("Hello world");
        assert!(result.calls.is_empty());
        assert_eq!(result.content, "Hello world");
    }

    #[test]
    fn test_content_before_json_disables_parsing() {
        let tools = vec![make_tool("get_weather")];
        let mut parser = ToolCallParser::with_json_tag(tools);

        let result = parser.add("Sure! {\"name\": \"get_weather\"}");
        assert!(result.calls.is_empty());
        assert!(result.content.contains("Sure!"));
        assert!(parser.is_finished());
    }

    #[test]
    fn test_simple_tool_call() {
        let tools = vec![make_tool("get_weather")];
        let mut parser = ToolCallParser::with_json_tag(tools);

        let result = parser.add("{\"name\": \"get_weather\", \"arguments\": {\"city\": \"NYC\"}}");
        assert_eq!(result.calls.len(), 1);
        assert_eq!(result.calls[0].function.name, "get_weather");
        assert_eq!(
            result.calls[0].function.arguments.get("city").unwrap(),
            &serde_json::json!("NYC")
        );
    }

    #[test]
    fn test_streaming_partial_json() {
        let tools = vec![make_tool("get_weather")];
        let mut parser = ToolCallParser::with_json_tag(tools);

        // Send partial JSON
        let r1 = parser.add("{\"name\": \"get_");
        assert!(r1.calls.is_empty());

        let r2 = parser.add("weather\", \"arguments\": {\"city\":");
        assert!(r2.calls.is_empty());

        let r3 = parser.add(" \"NYC\"}}");
        assert_eq!(r3.calls.len(), 1);
        assert_eq!(r3.calls[0].function.name, "get_weather");
    }

    #[test]
    fn test_multiple_tool_calls() {
        let tools = vec![make_tool("func_a"), make_tool("func_b")];
        let mut parser = ToolCallParser::with_json_tag(tools);

        let input = "{\"name\": \"func_a\", \"arguments\": {}}{\"name\": \"func_b\", \"arguments\": {\"x\": 1}}";
        let result = parser.add(input);

        assert_eq!(result.calls.len(), 2);
        assert_eq!(result.calls[0].function.name, "func_a");
        assert_eq!(result.calls[1].function.name, "func_b");
        assert_eq!(result.calls[0].function.index, 0);
        assert_eq!(result.calls[1].function.index, 1);
    }

    #[test]
    fn test_whitespace_before_json() {
        let tools = vec![make_tool("test_func")];
        let mut parser = ToolCallParser::with_json_tag(tools);

        let result = parser.add("  \n\t{\"name\": \"test_func\", \"arguments\": {}}");
        assert_eq!(result.calls.len(), 1);
    }

    #[test]
    fn test_tool_not_in_list() {
        let tools = vec![make_tool("allowed_func")];
        let mut parser = ToolCallParser::with_json_tag(tools);

        let result = parser.add("{\"name\": \"not_allowed\", \"arguments\": {}}");
        // No matching tool, should not parse
        assert!(result.calls.is_empty());
    }

    #[test]
    fn test_nested_json() {
        let tools = vec![make_tool("complex_func")];
        let mut parser = ToolCallParser::with_json_tag(tools);

        let input = r#"{"name": "complex_func", "arguments": {"data": {"nested": [1, 2, 3]}}}"#;
        let result = parser.add(input);

        assert_eq!(result.calls.len(), 1);
        let args = &result.calls[0].function.arguments;
        assert!(args.contains_key("data"));
    }
}
