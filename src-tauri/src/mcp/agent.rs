use crate::core::settings_v2::McpSettings;
use crate::core::types::{ChatMessage, GenerateRequest, StreamMessage, ToolChoice};
use crate::generate::tool_call_parser::{Tool, ToolFunction};
use crate::inference::engine::EngineSessionInfo;
use crate::inference::llamacpp::http_client::{self, ChatCompletionMessageResult};
use crate::mcp::runtime::{McpRuntimeState, call_tool, list_tools};
use std::collections::HashMap;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone)]
struct ToolRoute {
    server_id: String,
    tool_name: String,
}

fn slug(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    out.trim_matches('_').to_string()
}

fn build_round_tools(
    descriptors: &[crate::mcp::types::McpToolDescriptor],
) -> (Vec<Tool>, HashMap<String, ToolRoute>) {
    let mut tools = Vec::new();
    let mut routes = HashMap::new();
    for descriptor in descriptors {
        let mut alias = format!(
            "mcp_{}_{}",
            slug(&descriptor.server_id),
            slug(&descriptor.name)
        );
        if routes.contains_key(&alias) {
            alias = format!("{}_{}", alias, routes.len());
        }
        routes.insert(
            alias.clone(),
            ToolRoute {
                server_id: descriptor.server_id.clone(),
                tool_name: descriptor.name.clone(),
            },
        );
        tools.push(Tool {
            function: ToolFunction {
                name: alias,
                description: Some(format!(
                    "[{}] {}",
                    descriptor.server_id,
                    descriptor.description.clone().unwrap_or_default()
                )),
                parameters: Some(descriptor.input_schema.clone()),
            },
        });
    }
    (tools, routes)
}

fn render_tool_call_blocks(calls: &[http_client::ToolCallMessage]) -> String {
    let mut out = String::new();
    for call in calls {
        if !out.is_empty() {
            out.push('\n');
        }
        let args = serde_json::to_string(&serde_json::Value::Object(call.arguments.clone()))
            .unwrap_or_else(|_| "{}".to_string());
        out.push_str("<tool_call>\n{\"name\": \"");
        out.push_str(&call.name);
        out.push_str("\", \"arguments\": ");
        out.push_str(&args);
        out.push_str("}\n</tool_call>");
    }
    out
}

fn append_assistant_messages(
    messages: &mut Vec<ChatMessage>,
    response: &ChatCompletionMessageResult,
) -> Option<String> {
    if !response.tool_calls.is_empty() {
        let mut content = String::new();
        if !response.content.trim().is_empty() {
            content.push_str(response.content.trim_end());
            content.push('\n');
        }
        content.push_str(&render_tool_call_blocks(&response.tool_calls));
        messages.push(ChatMessage {
            role: "assistant".to_string(),
            content,
        });
        if !response.content.trim().is_empty() {
            return Some(response.content.clone());
        }
        return None;
    }

    if !response.content.trim().is_empty() {
        messages.push(ChatMessage {
            role: "assistant".to_string(),
            content: response.content.clone(),
        });
        return Some(response.content.clone());
    }
    None
}

fn emit_content(app: &AppHandle, content: impl Into<String>) {
    let content = content.into();
    if content.is_empty() {
        return;
    }
    let _ = app.emit(
        "message",
        StreamMessage {
            thinking: String::new(),
            content,
        },
    );
}

pub async fn run_agent_loop(
    app: &AppHandle,
    session: &EngineSessionInfo,
    mut req: GenerateRequest,
    runtime_state: &McpRuntimeState,
    mcp_settings: &McpSettings,
) -> Result<(), String> {
    let max_rounds = req
        .mcp
        .as_ref()
        .and_then(|m| m.max_tool_rounds)
        .unwrap_or(mcp_settings.max_tool_rounds)
        .clamp(1, 16);
    let mut messages = req
        .messages
        .clone()
        .ok_or_else(|| "chat messages are required for MCP agent loop".to_string())?;
    let mut stream_started = false;

    for round in 0..=max_rounds {
        let tools_available = list_tools(runtime_state)
            .await
            .map_err(|err| format!("failed to discover MCP tools: {err}"))?;
        if tools_available.is_empty() {
            return Err("no active MCP tools available".to_string());
        }
        let (tool_defs, tool_routes) = build_round_tools(&tools_available);
        req.messages = Some(messages.clone());
        req.prompt.clear();
        req.tools = if tool_defs.is_empty() {
            None
        } else {
            Some(tool_defs)
        };
        req.tool_choice = Some(ToolChoice::Mode("auto".to_string()));

        let response = http_client::chat_completion_once(session, req.clone()).await?;
        if !stream_started {
            let _ = app.emit("message_start", ());
            stream_started = true;
        }
        if let Some(content) = append_assistant_messages(&mut messages, &response) {
            emit_content(app, content);
        }
        if response.tool_calls.is_empty() {
            break;
        }
        if round == max_rounds {
            let _ = app.emit(
                "tooling_log",
                serde_json::json!({
                    "category": "MCP_DEBUG",
                    "message": "Agent loop reached max_tool_rounds limit",
                    "details": { "max_tool_rounds": max_rounds },
                }),
            );
            break;
        }

        let _ = app.emit(
            "tooling_log",
            serde_json::json!({
                "category": "MCP_DEBUG",
                "message": format!("Tool step {}/{}", round + 1, max_rounds),
                "details": { "requested_calls": response.tool_calls.len() },
            }),
        );

        for call in response.tool_calls {
            let Some(route) = tool_routes.get(&call.name).cloned() else {
                let error = format!("Tool routing error: unknown alias '{}'", call.name);
                messages.push(ChatMessage {
                    role: "tool".to_string(),
                    content: error.clone(),
                });
                continue;
            };
            let args = if call.arguments.is_empty() {
                None
            } else {
                Some(call.arguments.clone())
            };
            match call_tool(
                app,
                runtime_state,
                mcp_settings,
                Some(route.server_id.clone()),
                route.tool_name.clone(),
                args,
                None,
            )
            .await
            {
                Ok(result) => {
                    let content =
                        serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string());
                    messages.push(ChatMessage {
                        role: "tool".to_string(),
                        content,
                    });
                }
                Err(err) => {
                    let tool_error = format!("Tool execution error: {err}");
                    messages.push(ChatMessage {
                        role: "tool".to_string(),
                        content: tool_error.clone(),
                    });
                }
            }
        }
    }

    if stream_started {
        let _ = app.emit("message_done", ());
    }
    Ok(())
}
