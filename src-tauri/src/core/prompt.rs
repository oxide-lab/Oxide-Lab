//! Prompt builder module for creating prompts from chat templates.
//! This module provides functionality to build prompts from chat message histories
//! using Jinja-style chat templates extracted from tokenizers.

use crate::{log_template, log_template_error};
use minijinja::{Environment, Value, context};
use once_cell::sync::OnceCell;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Represents a chat message with role and content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Prompt builder for creating prompts from chat templates
pub struct PromptBuilder {
    chat_template: Option<String>,
    bos_token: Option<String>,
}

/// Нормализует чат-шаблоны, написанные в стилистике Jinja2/Python,
/// в совместимый с MiniJinja вид. Основной кейс — методы строк вида
/// `foo.startswith("bar")`/`foo.endswith("bar")`, которые MiniJinja не
/// поддерживает напрямую. Мы переписываем их в фильтры
/// `foo|starts_with("bar")`/`foo|ends_with("bar")`.
pub fn normalize_chat_template(raw: &str) -> String {
    static STARTS_RE: OnceCell<Regex> = OnceCell::new();
    static ENDS_RE: OnceCell<Regex> = OnceCell::new();
    static SPLIT_RE: OnceCell<Regex> = OnceCell::new();
    static STRIP_RE: OnceCell<Regex> = OnceCell::new();
    static LSTRIP_RE: OnceCell<Regex> = OnceCell::new();
    static RSTRIP_RE: OnceCell<Regex> = OnceCell::new();
    static LOWER_RE: OnceCell<Regex> = OnceCell::new();
    static UPPER_RE: OnceCell<Regex> = OnceCell::new();
    static REPLACE_RE: OnceCell<Regex> = OnceCell::new();

    let starts_re =
        STARTS_RE.get_or_init(|| Regex::new(r"\.startswith\s*\(").expect("valid startswith regex"));
    let ends_re =
        ENDS_RE.get_or_init(|| Regex::new(r"\.endswith\s*\(").expect("valid endswith regex"));
    let split_re = SPLIT_RE.get_or_init(|| Regex::new(r"\.split\s*\(").expect("valid split regex"));
    let strip_re = STRIP_RE.get_or_init(|| Regex::new(r"\.strip\s*\(").expect("valid strip regex"));
    let lstrip_re =
        LSTRIP_RE.get_or_init(|| Regex::new(r"\.lstrip\s*\(").expect("valid lstrip regex"));
    let rstrip_re =
        RSTRIP_RE.get_or_init(|| Regex::new(r"\.rstrip\s*\(").expect("valid rstrip regex"));
    let lower_re = LOWER_RE.get_or_init(|| Regex::new(r"\.lower\s*\(").expect("valid lower regex"));
    let upper_re = UPPER_RE.get_or_init(|| Regex::new(r"\.upper\s*\(").expect("valid upper regex"));
    let replace_re =
        REPLACE_RE.get_or_init(|| Regex::new(r"\.replace\s*\(").expect("valid replace regex"));

    let tmp = starts_re.replace_all(raw, "|starts_with(");
    let tmp = ends_re.replace_all(&tmp, "|ends_with(");
    let tmp = split_re.replace_all(&tmp, "|split(");
    let tmp = strip_re.replace_all(&tmp, "|strip(");
    let tmp = lstrip_re.replace_all(&tmp, "|lstrip(");
    let tmp = rstrip_re.replace_all(&tmp, "|rstrip(");
    let tmp = lower_re.replace_all(&tmp, "|lower(");
    let tmp = upper_re.replace_all(&tmp, "|upper(");
    let tmp = replace_re.replace_all(&tmp, "|replace(");
    // Поддерживаем индексирование после split-фильтра: expr|split('x')[0] -> (expr|split('x'))[0]
    let split_index_re = Regex::new(r"([^\s\{]+)\|split\(([^)]*)\)\s*\[([^\]]+)\]")
        .expect("valid split index regex");
    let mut current = tmp.into_owned();
    loop {
        let replaced = split_index_re
            .replace(&current, "($1|split($2))[$3]")
            .into_owned();
        if replaced == current {
            break current;
        }
        current = replaced;
    }
}

/// Регистрирует общие фильтры для работы с шаблонами чата.
pub fn configure_chat_template_environment<'a>(env: &mut Environment<'a>) {
    env.add_filter("starts_with", |s: &str, needle: &str| s.starts_with(needle));
    env.add_filter("ends_with", |s: &str, needle: &str| s.ends_with(needle));
    env.add_filter("strip", |s: &str, set: Option<&str>| match set {
        Some(chars) => s.trim_matches(|c| chars.contains(c)).to_string(),
        None => s.trim().to_string(),
    });
    env.add_filter("lstrip", |s: &str, set: Option<&str>| match set {
        Some(chars) => s.trim_start_matches(|c| chars.contains(c)).to_string(),
        None => s.trim_start().to_string(),
    });
    env.add_filter("rstrip", |s: &str, set: Option<&str>| match set {
        Some(chars) => s.trim_end_matches(|c| chars.contains(c)).to_string(),
        None => s.trim_end().to_string(),
    });
}

/// Нормализует и проверяет шаблон: возвращает нормализованный вариант или текст ошибки.
pub fn normalize_and_validate(raw: &str) -> Result<String, String> {
    let normalized = normalize_chat_template(raw);
    let mut env = Environment::new();
    configure_chat_template_environment(&mut env);
    env.add_template("tpl", &normalized)
        .map_err(|e| e.to_string())?;
    Ok(normalized)
}

impl PromptBuilder {
    /// Create a new prompt builder with an optional chat template
    pub fn new(chat_template: Option<String>) -> Self {
        Self {
            chat_template,
            bos_token: None,
        }
    }

    /// Create with explicit bos token variable (for templates that reference it)
    pub fn with_bos(mut self, bos_token: Option<String>) -> Self {
        self.bos_token = bos_token;
        self
    }

    /// Check if a chat template is available
    pub fn has_template(&self) -> bool {
        self.chat_template
            .as_ref()
            .is_some_and(|t| !t.trim().is_empty())
    }

    /// Render a prompt from chat messages using the chat template
    pub fn render_prompt(&self, messages: Vec<ChatMessage>) -> Result<String, String> {
        let tpl = match &self.chat_template {
            Some(s) if !s.trim().is_empty() => s.clone(),
            _ => return Err("chat_template not available".into()),
        };
        let tpl = normalize_chat_template(&tpl);

        // Log input
        log_template!("render: msgs={}, tpl_len={}", messages.len(), tpl.len());

        let mut env = Environment::new();
        configure_chat_template_environment(&mut env);
        env.add_template("tpl", &tpl).map_err(|e| e.to_string())?;
        let tmpl = env.get_template("tpl").map_err(|e| e.to_string())?;

        // Create minijinja context
        let msgs_val: Vec<Value> = messages.iter().map(Value::from_serialize).collect();
        // Inject optional bos_token if provided (needed by many LLaMA/Gemma templates)
        let rendered = if let Some(bos) = &self.bos_token {
            tmpl.render(context! {
                messages => msgs_val,
                add_generation_prompt => true,
                tools => Vec::<String>::new(),
                bos_token => bos,
            })
        } else {
            tmpl.render(context! {
                messages => msgs_val,
                add_generation_prompt => true,
                tools => Vec::<String>::new(),
            })
        }
        .map_err(|e| e.to_string())?;

        log_template!(
            "render ok, prefix=<<<{}>>>",
            rendered.chars().take(120).collect::<String>()
        );
        Ok(rendered)
    }

    /// Build a prompt using fallback formatting when no template is available
    pub fn build_fallback_prompt(&self, messages: Vec<ChatMessage>) -> String {
        let mut text = String::new();

        // Process each message in the history
        for m in messages {
            if m.role == "user" {
                // For user messages, strip any special command prefixes but keep content
                let payload = m.content.trim();
                text += &format!("{}{}\n", "user\n", payload);
            } else {
                text += &format!("{}{}\n", "assistant\n", m.content.trim());
            }
        }

        // Open assistant for current step response
        text += "assistant\n";
        text
    }

    /// Build a prompt with support for special control commands
    pub fn build_prompt_with_control(
        &self,
        messages: Vec<ChatMessage>,
        _control: Option<&str>,
    ) -> String {
        // Try to render with template first
        if self.has_template() {
            match self.render_prompt(messages.clone()) {
                Ok(rendered) => {
                    // Backend no longer injects empty think blocks for no_think control;
                    // render logic for think blocks moved to frontend parser/renderer.
                    return rendered;
                }
                Err(e) => {
                    log_template_error!("render failed: {}", e);
                    // Fall through to fallback
                }
            }
        }

        // Fallback to custom formatting
        self.build_fallback_prompt(messages)
    }
}

impl Default for PromptBuilder {
    fn default() -> Self {
        Self::new(None)
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_and_validate, normalize_chat_template};

    #[test]
    fn rewrites_py_string_methods_to_filters() {
        let tpl = r#"{{ foo.startswith("<") }} {{ bar.endswith(">") }} {{ baz.split(", ") }}"#;
        let normalized = normalize_chat_template(tpl);

        assert!(normalized.contains(r#"foo|starts_with("<")"#));
        assert!(normalized.contains(r#"bar|ends_with(">")"#));
        assert!(normalized.contains(r#"baz|split(", ")"#));
    }

    #[test]
    fn keeps_unrelated_content_intact() {
        let tpl =
            r#"{% for message in messages %}{{ message.role }}: {{ message.content }}{% endfor %}"#;
        assert_eq!(normalize_chat_template(tpl), tpl);
    }

    #[test]
    fn wraps_split_filter_for_indexing() {
        let tpl = r#"{{ content.split("</think>")[0].split("<think>")[-1] }}"#;
        let normalized = normalize_chat_template(tpl);
        assert!(normalized.contains(r#"((content|split("</think>"))[0]|split("<think>"))[-1]"#));
    }

    #[test]
    fn normalize_and_validate_accepts_tooling_template() {
        let tpl = r###"{%- if tools %}
    {{- '<|im_start|>system\n' }}
    {%- if messages[0].role == 'system' %}
        {{- messages[0].content + '\n\n' }}
    {%- endif %}
    {{- "# Tools\n\nYou may call one or more functions to assist with the user query.\n\nYou are provided with function signatures within <tools></tools> XML tags:\n<tools>" }}
    {%- for tool in tools %}
        {{- "\n" }}
        {{- tool | tojson }}
    {%- endfor %}
    {{- "\n</tools>\n\nFor each function call, return a json object with function name and arguments within <tool_call></tool_call> XML tags:\n<tool_call>\n{\"name\": <function-name>, \"arguments\": <args-json-object>}\n</tool_call><|im_end|>\n" }}
{%- else %}
    {%- if messages[0].role == 'system' %}
        {{- '<|im_start|>system\n' + messages[0].content + '<|im_end|>\n' }}
    {%- endif %}
{%- endif %}
{%- set ns = namespace(multi_step_tool=true, last_query_index=messages|length - 1) %}
{%- for message in messages[::-1] %}
    {%- set index = (messages|length - 1) - loop.index0 %}
    {%- if ns.multi_step_tool and message.role == "user" and message.content is string and not(message.content.startswith('<tool_response>') and message.content.endswith('</tool_response>')) %}
        {%- set ns.multi_step_tool = false %}
        {%- set ns.last_query_index = index %}
    {%- endif %}
{%- endfor %}
{%- for message in messages %}
    {%- if message.content is string %}
        {%- set content = message.content %}
    {%- else %}
        {%- set content = '' %}
    {%- endif %}
    {%- if (message.role == "user") or (message.role == "system" and not loop.first) %}
        {{- '<|im_start|>' + message.role + '\n' + content + '<|im_end|>' + '\n' }}
    {%- elif message.role == "assistant" %}
        {%- set reasoning_content = '' %}
        {%- if message.reasoning_content is string %}
            {%- set reasoning_content = message.reasoning_content %}
        {%- else %}
            {%- if '</think>' in content %}
                {%- set reasoning_content = content.split('</think>')[0].rstrip('\n').split('<think>')[-1].lstrip('\n') %}
                {%- set content = content.split('</think>')[-1].lstrip('\n') %}
            {%- endif %}
        {%- endif %}
        {%- if loop.index0 > ns.last_query_index %}
            {%- if loop.last or (not loop.last and reasoning_content) %}
                {{- '<|im_start|>' + message.role + '\n<think>\n' + reasoning_content.strip('\n') + '\n</think>\n\n' + content.lstrip('\n') }}
            {%- else %}
                {{- '<|im_start|>' + message.role + '\n' + content }}
            {%- endif %}
        {%- else %}
            {{- '<|im_start|>' + message.role + '\n' + content }}
        {%- endif %}
        {%- if message.tool_calls %}
            {%- for tool_call in message.tool_calls %}
                {%- if (loop.first and content) or (not loop.first) %}
                    {{- '\n' }}
                {%- endif %}
                {%- if tool_call.function %}
                    {%- set tool_call = tool_call.function %}
                {%- endif %}
                {{- '<tool_call>\n{\"name\": "' }}
                {{- tool_call.name }}
                {{- '", "arguments": ' }}
                {%- if tool_call.arguments is string %}
                    {{- tool_call.arguments }}
                {%- else %}
                    {{- tool_call.arguments | tojson }}
                {%- endif %}
                {{- '}\n</tool_call>' }}
            {%- endfor %}
        {%- endif %}
        {{- '<|im_end|>\n' }}
    {%- elif message.role == "tool" %}
        {%- if loop.first or (messages[loop.index0 - 1].role != "tool") %}
            {{- '<|im_start|>user' }}
        {%- endif %}
        {{- '\n<tool_response>\n' }}
        {{- content }}
        {{- '\n</tool_response>' }}
        {%- if loop.last or (messages[loop.index0 + 1].role != "tool") %}
            {{- '<|im_end|>\n' }}
        {%- endif %}
    {%- endif %}
{%- endfor %}
{%- if add_generation_prompt %}
    {{- '<|im_start|>assistant\n' }}
    {%- if enable_thinking is defined and enable_thinking is false %}
        {{- '<think>\n\n</think>\n\n' }}
    {%- endif %}
{%- endif %}"###;

        let normalized_tpl = normalize_chat_template(tpl);
        let normalized = normalize_and_validate(tpl);
        assert!(
            normalized.is_ok(),
            "normalize failed: {}\n--- normalized ---\n{}",
            normalized.err().unwrap(),
            normalized_tpl
        );
    }
}

