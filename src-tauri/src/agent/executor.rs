//! Stage 3: Agent Loop — Agent 的"大脑"
//!
//! 核心循环：LLM 自主决定调什么工具、调几次、什么时候回答。
//!
//! 对应 Python: 03_agent_loop.py 里的 agent_run() 函数
//! 架构位置：在 Tools (Stage 1) 和 Model (Stage 2) 之上

use super::events::{default_tool_label, StreamEvent};
use super::model::{self, Message, StopReason};
use super::tools::ToolRegistry;
use crate::config::ModelConfig;
use crate::database::Database;
use crate::error::AppError;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::mpsc;
use work_review_core::database::MemorySearchItem;

// ══════════════════════════════════════════════════════════
// Agent 执行结果
// ══════════════════════════════════════════════════════════

/// Agent 的执行结果
#[derive(Debug)]
pub struct AgentResult {
    /// 最终回答
    pub answer: String,
    /// 工具调用记录
    pub tool_labels: Vec<String>,
    /// 工具执行收集的引用记录（供前端展示"依据"）
    pub references: Vec<MemorySearchItem>,
}

// ══════════════════════════════════════════════════════════
// Agent 执行器 — 核心循环
// ══════════════════════════════════════════════════════════

/// 默认最大迭代次数
const DEFAULT_MAX_ITERATIONS: usize = 8;

/// 默认 system prompt
const DEFAULT_SYSTEM_PROMPT: &str =
    "你是 Work Review 的工作助手。你可以回答任何问题。对于工作相关问题，优先使用工具查询用户的真实工作记录。对于非工作问题，直接用你的知识回答。\
     请使用简体中文回答，先给结论再给依据。不要编造不存在的事实。";

/// Agent 执行器
///
/// 对应 Python 的 agent_run() 函数
pub struct AgentExecutor;

impl AgentExecutor {
    /// 运行 Agent 循环
    ///
    /// 这是整个 Agent 的心脏。逻辑和 Python 版完全一致：
    /// ```
    /// for i in 0..max_iterations:
    ///     response = llm.chat(messages, tools)
    ///     if response 是最终回答 → 返回
    ///     if response 是工具调用 → 执行工具，结果追加到 messages，继续
    /// 超过 max_iterations → 强制结束
    /// ```
    ///
    /// `event_tx` 用于流式推送步骤进度（StepStart/StepResult）与终态（Done）。
    /// 为 None 时退化为静默执行（单测 / 非流式调用方）。
    pub async fn run(
        question: &str,
        model_config: &ModelConfig,
        database: &Database,
        system_prompt: Option<&str>,
        history: &[Message],
        max_iterations: Option<usize>,
        ignored_apps: Vec<String>,
        excluded_domains: Vec<String>,
        event_tx: Option<mpsc::Sender<StreamEvent>>,
    ) -> Result<AgentResult, AppError> {
        let sys = system_prompt.unwrap_or(DEFAULT_SYSTEM_PROMPT);
        let max_iter = max_iterations.unwrap_or(DEFAULT_MAX_ITERATIONS);

        // 工具注册中心（Stage 1）
        let registry = ToolRegistry::new();
        let tools = registry.to_openai_tools();
        let tool_context = super::tools::ToolContext {
            database,
            ignored_apps,
            excluded_domains,
            collected_references: Arc::new(Mutex::new(Vec::new())),
        };

        // 构造初始消息：历史 + 当前问题
        let mut messages: Vec<Message> = history.to_vec();
        messages.push(Message::user(question));

        let mut tool_labels = Vec::new();
        let start = Instant::now();

        for _ in 0..max_iter {
            // ── 第 1 步：调用 LLM（Stage 2） ──
            let response = model::chat_with_tools(model_config, sys, &messages, &tools)
                .await
                .map_err(|e| AppError::Analysis(format!("Agent 调用失败: {e}")))?;

            // ── 第 2 步：判断 LLM 的意图 ──
            match response.stop_reason {
                StopReason::Stop => {
                    // LLM 给出最终回答 → 循环结束
                    let content = response.content.unwrap_or_default();
                    let references = tool_context.take_all_references();
                    emit_done(&event_tx, &content, &references, &tool_labels);
                    return Ok(AgentResult {
                        answer: content,
                        tool_labels,
                        references,
                    });
                }

                StopReason::ToolCall => {
                    // provider 声明要调用工具却未给出实际 tool_calls（某些 OpenAI 兼容中转
                    // 网关在边缘情况下会如此）→ 直接终止，避免 messages 不变导致循环空转、
                    // 白白消耗最多 8 轮 API 配额与 30s 用户等待。
                    let calls_missing = response
                        .tool_calls
                        .as_ref()
                        .is_none_or(|calls| calls.is_empty());
                    if calls_missing {
                        let content = response.content.clone().unwrap_or_else(|| {
                            "模型未返回可执行的工具调用，请稍后重试。".to_string()
                        });
                        let references = tool_context.take_all_references();
                        emit_done(&event_tx, &content, &references, &tool_labels);
                        return Ok(AgentResult {
                            answer: content,
                            tool_labels,
                            references,
                        });
                    }

                    // LLM 想调工具 → 执行
                    if let Some(calls) = &response.tool_calls {
                        // ① 记录 assistant 的工具调用
                        messages.push(Message::assistant_with_tool_calls(calls));

                        // ② 逐个执行工具
                        for tc in calls {
                            if !tool_labels.contains(&tc.name) {
                                tool_labels.push(tc.name.clone());
                            }

                            // 步骤开始：推送 StepStart，并记录引用基线以取本轮增量
                            emit_event(
                                &event_tx,
                                StreamEvent::StepStart {
                                    tool: tc.name.clone(),
                                    label: default_tool_label(&tc.name).to_string(),
                                },
                            );
                            let ref_base = tool_context.references_len();

                            // 执行工具（Stage 1）
                            let result = match registry.execute(
                                &tc.name,
                                tc.arguments.clone(),
                                &tool_context,
                            ) {
                                Ok(r) => r,
                                Err(e) => format!("工具执行失败: {e}"),
                            };

                            // 步骤结束：推送 StepResult（携带本轮新增引用）
                            let new_refs = tool_context.drain_from(ref_base);
                            emit_event(
                                &event_tx,
                                StreamEvent::StepResult {
                                    tool: tc.name.clone(),
                                    hits: new_refs.len(),
                                    references: new_refs,
                                },
                            );

                            // ③ 追加工具结果到对话历史（携带工具名，Gemini 需要）
                            messages.push(Message::tool_result_named(
                                &tc.id,
                                &result,
                                Some(&tc.name),
                            ));
                        }
                    }
                    // 继续循环 → LLM 下一轮能看到工具结果
                }

                StopReason::MaxTokens => {
                    // Token 用完了，用已有内容回答
                    let content = response
                        .content
                        .unwrap_or_else(|| "回答被截断，请尝试缩短问题。".to_string());
                    let references = tool_context.take_all_references();
                    emit_done(&event_tx, &content, &references, &tool_labels);
                    return Ok(AgentResult {
                        answer: content,
                        tool_labels,
                        references,
                    });
                }
            }

            // 安全检查：如果循环超过 30 秒，强制结束
            if start.elapsed().as_secs() > 30 {
                let content = "处理超时，请尝试更具体的问题。".to_string();
                let references = tool_context.take_all_references();
                emit_done(&event_tx, &content, &references, &tool_labels);
                return Ok(AgentResult {
                    answer: content,
                    tool_labels,
                    references,
                });
            }
        }

        // ── 超过最大迭代次数 ──
        let content = "抱歉，处理这个问题需要过多步骤。请尝试更具体地描述。".to_string();
        let references = tool_context.take_all_references();
        emit_done(&event_tx, &content, &references, &tool_labels);
        Ok(AgentResult {
            answer: content,
            tool_labels,
            references,
        })
    }
}

/// 推送一个流式事件；channel 满/关闭都不影响主流程。
fn emit_event(tx: &Option<mpsc::Sender<StreamEvent>>, evt: StreamEvent) {
    if let Some(tx) = tx {
        let _ = tx.try_send(evt);
    }
}

/// 推送终态 Done 事件（携带完整答案、引用、工具标签）。
fn emit_done(
    tx: &Option<mpsc::Sender<StreamEvent>>,
    answer: &str,
    references: &[MemorySearchItem],
    tool_labels: &[String],
) {
    emit_event(
        tx,
        StreamEvent::Done {
            answer: answer.to_string(),
            references: references.to_vec(),
            tool_labels: tool_labels.to_vec(),
        },
    );
}

// ══════════════════════════════════════════════════════════
// 测试
// ══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::super::model::ToolCall;
    use super::*;

    #[test]
    fn test_max_iterations_default() {
        assert_eq!(DEFAULT_MAX_ITERATIONS, 8);
    }

    #[test]
    fn test_message_construction() {
        let user_msg = Message::user("今天做了什么");
        assert_eq!(user_msg.role, "user");
        assert_eq!(user_msg.content.as_deref(), Some("今天做了什么"));

        let tool_msg = Message::tool_result("call_123", "结果");
        assert_eq!(tool_msg.role, "tool");
        assert_eq!(tool_msg.tool_call_id.as_deref(), Some("call_123"));

        let tc = ToolCall {
            id: "call_456".to_string(),
            name: "search_memory".to_string(),
            arguments: serde_json::json!({"query": "debug"}),
        };
        let assistant_msg = Message::assistant_with_tool_calls(&[tc]);
        assert_eq!(assistant_msg.role, "assistant");
        assert!(assistant_msg.tool_calls.is_some());
    }
}
