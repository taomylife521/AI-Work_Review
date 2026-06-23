//! Stage 2: Model 层 — Agent 的"嘴巴"
//!
//! 职责：把统一的消息格式翻译成各家 API 的请求格式，
//!       把各家 API 的响应翻译回统一格式。
//!
//! 对应 Python: 02_model.py 里的 Message/ToolCall/LlmResponse/Provider

use crate::config::{AiProvider, ModelConfig};
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

// ══════════════════════════════════════════════════════════
// 第一部分：统一的消息格式
// ══════════════════════════════════════════════════════════
// 对应 Python: class Message / ToolCall / LlmResponse

/// LLM 想调用的工具
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

/// 停止原因
#[derive(Debug, Clone, PartialEq)]
pub enum StopReason {
    Stop,
    ToolCall,
    MaxTokens,
}

/// LLM 的统一响应 — 不管底层是什么提供商
#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub stop_reason: StopReason,
}

/// 统一的消息格式 — Agent 内部只用这个
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// 工具名称（仅 tool role 消息使用，Gemini 需要）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Message {
    pub fn user(content: &str) -> Self {
        Self {
            role: "user".into(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn assistant(content: &str) -> Self {
        Self {
            role: "assistant".into(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn assistant_with_tool_calls(calls: &[ToolCall]) -> Self {
        let tool_calls_json: Vec<Value> = calls
            .iter()
            .map(|tc| {
                json!({
                    "id": tc.id,
                    "type": "function",
                    "function": {
                        "name": tc.name,
                        "arguments": tc.arguments.to_string()
                    }
                })
            })
            .collect();
        Self {
            role: "assistant".into(),
            content: None,
            tool_calls: Some(Value::Array(tool_calls_json)),
            tool_call_id: None,
            name: None,
        }
    }

    pub fn tool_result_named(tool_call_id: &str, content: &str, name: Option<&str>) -> Self {
        Self {
            role: "tool".into(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
            name: name.map(|n| n.to_string()),
        }
    }
}

// ══════════════════════════════════════════════════════════
// 第二部分：chat 函数 — 统一的 LLM 调用入口
// ══════════════════════════════════════════════════════════

/// 统一的 LLM 调用函数（支持 tool-calling）
///
/// 这是你现有的 `generate_text_answer_with_model` 的升级版：
/// - 旧版：只能发 system + user，收纯文字
/// - 新版：支持发 messages + tools，收文字或 tool_calls
///
/// 对应 Python: provider.chat(messages, tools) -> LlmResponse
pub async fn chat_with_tools(
    model_config: &ModelConfig,
    system_prompt: &str,
    messages: &[Message],
    tools: &[Value],
) -> Result<LlmResponse, AppError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| AppError::Unknown(e.to_string()))?;

    // 构造完整的 messages 数组：system + 用户对话历史
    let mut full_messages = vec![json!({
        "role": "system",
        "content": system_prompt
    })];
    for msg in messages {
        full_messages.push(serde_json::to_value(msg).unwrap_or_default());
    }

    // 根据提供商分发
    match model_config.provider {
        AiProvider::Ollama => chat_ollama(&client, model_config, &full_messages, tools).await,
        AiProvider::Claude => chat_claude(&client, model_config, &full_messages, tools).await,
        AiProvider::Gemini => chat_gemini(&client, model_config, &full_messages, tools).await,
        _ => chat_openai_compatible(&client, model_config, &full_messages, tools).await,
    }
}

// ══════════════════════════════════════════════════════════
// 第三部分：各家 Provider 的实现 — 格式翻译
// ══════════════════════════════════════════════════════════

/// OpenAI 兼容格式（覆盖 8 个提供商：OpenAI/SiliconFlow/DeepSeek/Qwen/Zhipu/Moonshot/Doubao/MiniMax）
///
/// 面试要点：这些提供商都用相同的 API 格式，所以一个实现覆盖全部。
async fn chat_openai_compatible(
    client: &reqwest::Client,
    model_config: &ModelConfig,
    messages: &[Value],
    tools: &[Value],
) -> Result<LlmResponse, AppError> {
    let endpoint = model_config.endpoint.trim().trim_end_matches('/');
    let url = if endpoint.ends_with("/chat/completions") {
        endpoint.to_string()
    } else {
        format!("{endpoint}/chat/completions")
    };

    let mut body = json!({
        "model": model_config.model,
        "messages": messages,
        "max_tokens": 1600,
        "temperature": 0.2
    });

    // 只有提供了工具定义时才加 tools 参数
    if !tools.is_empty() {
        body["tools"] = json!(tools);
    }

    let mut request = client.post(&url).json(&body);
    if let Some(api_key) = &model_config.api_key {
        if !api_key.is_empty() {
            request = request.header("Authorization", format!("Bearer {api_key}"));
        }
    }

    let response = request.send().await?;
    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(AppError::Analysis(format!("LLM 调用失败: {error_text}")));
    }

    let result: Value = response.json().await?;
    parse_openai_response(&result)
}

/// Ollama 格式
async fn chat_ollama(
    client: &reqwest::Client,
    model_config: &ModelConfig,
    messages: &[Value],
    tools: &[Value],
) -> Result<LlmResponse, AppError> {
    let ollama_base = model_config.endpoint.trim().trim_end_matches('/');
    let url = if ollama_base.ends_with("/api/chat") {
        ollama_base.to_string()
    } else {
        format!("{ollama_base}/api/chat")
    };

    let mut body = json!({
        "model": model_config.model,
        "messages": messages,
        "stream": false
    });

    // Ollama 也支持 tools 参数（模型支持的话）
    if !tools.is_empty() {
        body["tools"] = json!(tools);
    }

    let response = client.post(&url).json(&body).send().await?;
    if !response.status().is_success() {
        return Err(AppError::Analysis(format!(
            "Ollama 调用失败: {}",
            response.status()
        )));
    }

    let result: Value = response.json().await?;
    // Ollama 的响应格式和 OpenAI 类似
    parse_openai_response(&result)
}

/// Claude (Anthropic) 格式
async fn chat_claude(
    client: &reqwest::Client,
    model_config: &ModelConfig,
    messages: &[Value],
    tools: &[Value],
) -> Result<LlmResponse, AppError> {
    let api_key = model_config
        .api_key
        .as_deref()
        .ok_or_else(|| AppError::Analysis("Claude 需要 API Key，请在设置中配置".to_string()))?;

    let endpoint = model_config.endpoint.trim().trim_end_matches('/');
    let url = if endpoint.ends_with("/messages") {
        endpoint.to_string()
    } else {
        format!("{endpoint}/messages")
    };

    // Claude 的消息格式：去掉 system（放在顶层），转换 tool 消息格式
    let claude_messages: Vec<Value> = messages
        .iter()
        .filter(|m| m["role"].as_str() != Some("system"))
        .map(|m| {
            match m["role"].as_str() {
                // assistant + tool_calls → Claude content blocks with tool_use
                Some("assistant") if m["tool_calls"].is_array() => {
                    let mut content_blocks: Vec<Value> = vec![];
                    // 如果有文字内容，先加文字 block
                    if let Some(text) = m["content"].as_str() {
                        if !text.is_empty() {
                            content_blocks.push(json!({"type": "text", "text": text}));
                        }
                    }
                    // 加 tool_use blocks
                    if let Some(calls) = m["tool_calls"].as_array() {
                        for call in calls {
                            content_blocks.push(json!({
                                "type": "tool_use",
                                "id": call["id"],
                                "name": call["function"]["name"],
                                "input": serde_json::from_str::<Value>(
                                    call["function"]["arguments"].as_str().unwrap_or("{}")
                                ).unwrap_or(json!({}))
                            }));
                        }
                    }
                    json!({"role": "assistant", "content": content_blocks})
                }
                // tool result → Claude user message with tool_result content block
                Some("tool") => {
                    json!({
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": m["tool_call_id"],
                            "content": m["content"]
                        }]
                    })
                }
                // user / plain assistant → 直接传
                _ => m.clone(),
            }
        })
        .collect();

    let system_content = messages
        .iter()
        .find(|m| m["role"].as_str() == Some("system"))
        .and_then(|m| m["content"].as_str())
        .unwrap_or("");

    // Claude 的工具定义格式不同：用 input_schema 而不是 parameters
    let claude_tools: Vec<Value> = tools
        .iter()
        .map(|t| {
            json!({
                "name": t["function"]["name"],
                "description": t["function"]["description"],
                "input_schema": t["function"]["parameters"]
            })
        })
        .collect();

    let mut body = json!({
        "model": model_config.model,
        "max_tokens": 1600,
        "system": system_content,
        "messages": claude_messages,
    });
    if !claude_tools.is_empty() {
        body["tools"] = json!(claude_tools);
    }

    let response = client
        .post(&url)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .header("x-api-key", api_key)
        .json(&body)
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(AppError::Analysis(format!("Claude 调用失败: {error_text}")));
    }

    let result: Value = response.json().await?;
    parse_claude_response(&result)
}

/// Gemini 格式
async fn chat_gemini(
    client: &reqwest::Client,
    model_config: &ModelConfig,
    messages: &[Value],
    tools: &[Value],
) -> Result<LlmResponse, AppError> {
    let endpoint = model_config.endpoint.trim().trim_end_matches('/');
    let api_key = model_config
        .api_key
        .as_deref()
        .ok_or_else(|| AppError::Analysis("Gemini 需要 API Key，请在设置中配置".to_string()))?;
    let url = format!("{endpoint}/models/{}:generateContent", model_config.model);

    // Gemini 格式：contents + systemInstruction + tools
    let mut contents = vec![];
    let mut system_instruction = None;

    for msg in messages {
        match msg["role"].as_str() {
            Some("system") => {
                system_instruction = Some(json!({"parts": [{"text": msg["content"]}] }));
            }
            Some("user") => {
                contents.push(json!({
                    "role": "user",
                    "parts": [{"text": msg["content"]}]
                }));
            }
            Some("assistant") if msg["tool_calls"].is_array() => {
                // assistant + tool_calls → Gemini functionCall parts
                let mut parts: Vec<Value> = vec![];
                if let Some(text) = msg["content"].as_str() {
                    if !text.is_empty() {
                        parts.push(json!({"text": text}));
                    }
                }
                if let Some(calls) = msg["tool_calls"].as_array() {
                    for call in calls {
                        let args: Value = serde_json::from_str(
                            call["function"]["arguments"].as_str().unwrap_or("{}"),
                        )
                        .unwrap_or(json!({}));
                        parts.push(json!({
                            "functionCall": {
                                "name": call["function"]["name"],
                                "args": args
                            }
                        }));
                    }
                }
                if !parts.is_empty() {
                    contents.push(json!({"role": "model", "parts": parts}));
                }
            }
            Some("assistant") => {
                contents.push(json!({
                    "role": "model",
                    "parts": [{"text": msg["content"].as_str().unwrap_or("")}]
                }));
            }
            Some("tool") => {
                // tool result → Gemini functionResponse
                let fn_name = msg
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                contents.push(json!({
                    "role": "function",
                    "parts": [{
                        "functionResponse": {
                            "name": fn_name,
                            "response": {
                                "result": msg["content"]
                            }
                        }
                    }]
                }));
            }
            _ => {}
        }
    }

    // Gemini 的工具定义格式：functionDeclarations
    let gemini_tools: Vec<Value> = tools
        .iter()
        .map(|t| {
            json!({
                "name": t["function"]["name"],
                "description": t["function"]["description"],
                "parameters": t["function"]["parameters"]
            })
        })
        .collect();

    let mut body = json!({
        "contents": contents,
    });
    if let Some(sys) = system_instruction {
        body["systemInstruction"] = sys;
    }
    if !gemini_tools.is_empty() {
        body["tools"] = json!([{"function_declarations": gemini_tools}]);
    }

    let response = client
        .post(&url)
        .header("x-goog-api-key", api_key)
        .json(&body)
        .send()
        .await?;
    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(AppError::Analysis(format!(
            "Gemini 调用失败: {}",
            error_text.chars().take(300).collect::<String>()
        )));
    }

    let result: Value = response.json().await?;
    parse_gemini_response(&result)
}

// ══════════════════════════════════════════════════════════
// 第四部分：响应解析 — 各家格式 → 统一格式
// ══════════════════════════════════════════════════════════

/// 解析 OpenAI 格式的响应（Ollama 也用这个）
fn parse_openai_response(result: &Value) -> Result<LlmResponse, AppError> {
    let choice = &result["choices"][0];
    let msg = &choice["message"];

    // 解析 tool_calls
    let tool_calls = if let Some(tcs) = msg["tool_calls"].as_array() {
        let parsed: Vec<ToolCall> = tcs
            .iter()
            .filter_map(|tc| {
                let id = tc["id"].as_str()?.to_string();
                let name = tc["function"]["name"].as_str()?.to_string();
                let args_str = tc["function"]["arguments"].as_str().unwrap_or("{}");
                let arguments = serde_json::from_str(args_str).unwrap_or(json!({}));
                Some(ToolCall {
                    id,
                    name,
                    arguments,
                })
            })
            .collect();
        if parsed.is_empty() {
            None
        } else {
            Some(parsed)
        }
    } else {
        None
    };

    // 判断 stop_reason
    let stop_reason = match choice["finish_reason"].as_str() {
        Some("tool_calls") => StopReason::ToolCall,
        Some("length") => StopReason::MaxTokens,
        _ => {
            if tool_calls.is_some() {
                StopReason::ToolCall
            } else {
                StopReason::Stop
            }
        }
    };

    let content = msg["content"].as_str().map(|s| s.to_string());

    Ok(LlmResponse {
        content,
        tool_calls,
        stop_reason,
    })
}

/// 解析 Claude 格式的响应
fn parse_claude_response(result: &Value) -> Result<LlmResponse, AppError> {
    let content_blocks = result["content"].as_array();

    let mut text_content = String::new();
    let mut tool_calls = Vec::new();

    if let Some(blocks) = content_blocks {
        for block in blocks {
            match block["type"].as_str() {
                Some("text") => {
                    if let Some(t) = block["text"].as_str() {
                        text_content.push_str(t);
                    }
                }
                Some("tool_use") => {
                    tool_calls.push(ToolCall {
                        id: block["id"].as_str().unwrap_or("").to_string(),
                        name: block["name"].as_str().unwrap_or("").to_string(),
                        arguments: block["input"].clone(),
                        //            ↑ Claude 的参数已经是 object，不需要 JSON.parse
                    });
                }
                _ => {}
            }
        }
    }

    let stop_reason = match result["stop_reason"].as_str() {
        Some("tool_use") => StopReason::ToolCall,
        Some("max_tokens") => StopReason::MaxTokens,
        _ => {
            if !tool_calls.is_empty() {
                StopReason::ToolCall
            } else {
                StopReason::Stop
            }
        }
    };

    Ok(LlmResponse {
        content: if text_content.is_empty() {
            None
        } else {
            Some(text_content)
        },
        tool_calls: if tool_calls.is_empty() {
            None
        } else {
            Some(tool_calls)
        },
        stop_reason,
    })
}

/// 解析 Gemini 格式的响应
fn parse_gemini_response(result: &Value) -> Result<LlmResponse, AppError> {
    let parts = result["candidates"][0]["content"]["parts"].as_array();

    let mut text_content = String::new();
    let mut tool_calls = Vec::new();

    if let Some(parts_arr) = parts {
        for part in parts_arr {
            if let Some(text) = part["text"].as_str() {
                text_content.push_str(text);
            }
            if let Some(fc) = part.get("functionCall") {
                tool_calls.push(ToolCall {
                    id: format!("gemini_{}", tool_calls.len()),
                    name: fc["name"].as_str().unwrap_or("").to_string(),
                    arguments: fc["args"].clone(),
                });
            }
        }
    }

    let stop_reason = if !tool_calls.is_empty() {
        StopReason::ToolCall
    } else {
        StopReason::Stop
    };

    Ok(LlmResponse {
        content: if text_content.is_empty() {
            None
        } else {
            Some(text_content)
        },
        tool_calls: if tool_calls.is_empty() {
            None
        } else {
            Some(tool_calls)
        },
        stop_reason,
    })
}

// ══════════════════════════════════════════════════════════
// 测试
// ══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_user() {
        let msg = Message::user("你好");
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content.as_deref(), Some("你好"));
        assert!(msg.tool_calls.is_none());
        assert!(msg.tool_call_id.is_none());
    }

    #[test]
    fn test_message_tool_result() {
        let msg = Message::tool_result_named("call_123", "结果数据", None);
        assert_eq!(msg.role, "tool");
        assert_eq!(msg.tool_call_id.as_deref(), Some("call_123"));
    }

    #[test]
    fn test_parse_openai_response_with_tool_call() {
        // 模拟 OpenAI 返回一个 tool_call
        let response = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_abc123",
                        "type": "function",
                        "function": {
                            "name": "analyze_intents",
                            "arguments": "{\"date_from\":\"2026-06-01\",\"date_to\":\"2026-06-09\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });

        let parsed = parse_openai_response(&response).unwrap();

        assert_eq!(parsed.stop_reason, StopReason::ToolCall);
        assert!(parsed.content.is_none());

        let tc = &parsed.tool_calls.unwrap()[0];
        assert_eq!(tc.id, "call_abc123");
        assert_eq!(tc.name, "analyze_intents");
        assert_eq!(tc.arguments["date_from"], "2026-06-01");
    }

    #[test]
    fn test_parse_openai_response_text_only() {
        let response = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "你好！我是你的工作助手。"
                },
                "finish_reason": "stop"
            }]
        });

        let parsed = parse_openai_response(&response).unwrap();
        assert_eq!(parsed.stop_reason, StopReason::Stop);
        assert_eq!(parsed.content.as_deref(), Some("你好！我是你的工作助手。"));
        assert!(parsed.tool_calls.is_none());
    }

    #[test]
    fn test_parse_claude_response_with_tool_use() {
        // 模拟 Claude 返回一个 tool_use
        let response = json!({
            "content": [
                {"type": "text", "text": "让我查一下..."},
                {"type": "tool_use", "id": "toolu_xyz", "name": "search_memory", "input": {"query": "debug"}}
            ],
            "stop_reason": "tool_use"
        });

        let parsed = parse_claude_response(&response).unwrap();

        assert_eq!(parsed.stop_reason, StopReason::ToolCall);
        assert_eq!(parsed.content.as_deref(), Some("让我查一下..."));

        let tc = &parsed.tool_calls.unwrap()[0];
        assert_eq!(tc.id, "toolu_xyz");
        assert_eq!(tc.name, "search_memory");
        // Claude 的 input 已经是 object，不需要 JSON.parse
        assert_eq!(tc.arguments["query"], "debug");
    }

    #[test]
    fn test_parse_gemini_response_with_function_call() {
        // 模拟 Gemini 返回一个 functionCall
        let response = json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "functionCall": {
                            "name": "analyze_intents",
                            "args": {"date_from": "2026-05-01", "date_to": "2026-05-31"}
                        }
                    }]
                }
            }]
        });

        let parsed = parse_gemini_response(&response).unwrap();

        assert_eq!(parsed.stop_reason, StopReason::ToolCall);
        let tc = &parsed.tool_calls.unwrap()[0];
        assert_eq!(tc.name, "analyze_intents");
        assert_eq!(tc.arguments["date_from"], "2026-05-01");
    }

    #[test]
    fn test_openai_arguments_are_string_but_parsed_to_object() {
        // OpenAI 的 arguments 是字符串，parse_openai_response 要 JSON.parse 它
        let response = json!({
            "choices": [{
                "message": {
                    "tool_calls": [{
                        "id": "call_test",
                        "type": "function",
                        "function": {
                            "name": "search_memory",
                            "arguments": "{\"query\":\"编码\",\"date_from\":\"2026-06-01\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });

        let parsed = parse_openai_response(&response).unwrap();
        let tc = &parsed.tool_calls.unwrap()[0];
        // arguments 应该被解析成 object，不是字符串
        assert!(tc.arguments.is_object());
        assert_eq!(tc.arguments["query"], "编码");
    }
}
