//! 工作助手流式事件。
//!
//! agent 模块用 `tokio::sync::mpsc` 传递这些事件，commands 层再桥接到
//! Tauri `ipc::Channel`。本文件纯 serde，不依赖 tauri，保持 agent 可单测。

use serde::{Deserialize, Serialize};
use work_review_core::database::MemorySearchItem;

/// 工作助手流式事件（经 Tauri `ipc::Channel` 推送给前端）。
///
/// 前端按 `type` 字段分发：`stepStart` / `stepResult` / `token` / `done` / `error`。
///
/// 注意：内部标签（`tag = "type"`）不支持包裹原始类型的 newtype 变体
/// （serde 会在序列化时报错），因此 Token / Error 必须用 struct 变体，
/// 恰好也与前端读取的 `event.token` / `event.error` 字段对齐。
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum StreamEvent {
    /// 工具步骤开始（每个 tool_call 执行前推送）。
    StepStart { tool: String, label: String },
    /// 工具步骤完成，携带本次新增的引用记录。
    StepResult {
        tool: String,
        hits: usize,
        references: Vec<MemorySearchItem>,
    },
    /// LLM 文本增量（token 流式）。后端做了小批量合并，一个事件可含多个字符。
    Token { token: String },
    /// 终态：完整答案 + 合并后的全部引用 + 用到的工具标签。
    /// 字段级 camelCase：枚举顶层的 rename_all 只作用于变体名，
    /// 不重命名字段；前端读取的是 event.toolLabels。
    #[serde(rename_all = "camelCase")]
    Done {
        answer: String,
        references: Vec<MemorySearchItem>,
        tool_labels: Vec<String>,
    },
    /// 错误终态。
    Error { error: String },
}

/// 工具名 → 默认中文标签（前端可按 tool 名覆盖为 i18n 文案）。
///
/// 放在这里而不是前端，是因为 executor 推送 `StepStart` 时需要立即给一个 label，
/// 否则前端在 i18n 未命中时会空白。
pub fn default_tool_label(tool: &str) -> &'static str {
    match tool {
        "search_memory" => "记忆检索",
        "analyze_intents" => "意图分析",
        "aggregate_stats" => "统计聚合",
        "category_search" => "分类检索",
        "trend_comparison" => "趋势对比",
        _ => "处理中",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 线格式契约：前端 Ask.svelte 按 event.type 分发、读 event.token / event.error 字段。
    /// 内部标签 + newtype(String) 会直接序列化失败，必须保持 struct 变体。
    #[test]
    fn 流式事件序列化应匹配前端读取的字段() {
        let token = serde_json::to_value(StreamEvent::Token {
            token: "你好".to_string(),
        })
        .expect("Token 事件必须可序列化");
        assert_eq!(token["type"], "token");
        assert_eq!(token["token"], "你好");

        let error = serde_json::to_value(StreamEvent::Error {
            error: "boom".to_string(),
        })
        .expect("Error 事件必须可序列化");
        assert_eq!(error["type"], "error");
        assert_eq!(error["error"], "boom");

        let step = serde_json::to_value(StreamEvent::StepStart {
            tool: "search_memory".to_string(),
            label: "记忆检索".to_string(),
        })
        .expect("StepStart 事件必须可序列化");
        assert_eq!(step["type"], "stepStart");
        assert_eq!(step["tool"], "search_memory");

        let done = serde_json::to_value(StreamEvent::Done {
            answer: "答案".to_string(),
            references: vec![],
            tool_labels: vec!["search_memory".to_string()],
        })
        .expect("Done 事件必须可序列化");
        assert_eq!(done["type"], "done");
        assert_eq!(done["answer"], "答案");
        assert_eq!(done["toolLabels"][0], "search_memory");
    }
}
