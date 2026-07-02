//! Stage 5: Orchestrator — Agent 的"指挥官"
//!
//! 路由决策：简单 → FastPath，复杂 → AgentPath
//! 降级策略：Agent 失败 → FastPath → FallbackPath
//!
//! 对应 Python: 05_orchestrator.py 里的 Orchestrator 类

use super::events::StreamEvent;
use super::executor::AgentExecutor;
use super::model::Message;
use crate::config::ModelConfig;
use crate::database::Database;
use crate::error::AppError;
use tokio::sync::mpsc;
use work_review_core::database::MemorySearchItem;

// ══════════════════════════════════════════════════════════
// 路径类型
// ══════════════════════════════════════════════════════════

/// 查询路径
#[derive(Debug, Clone, PartialEq)]
pub enum QueryPath {
    /// 直接回答（闲聊/求助）
    Direct,
    /// 规则快速路径（简单时间查询）
    Fast,
    /// Agent 循环（复杂查询）
    Agent,
    /// 无模型兜底（模板回答）
    Fallback,
}

/// 路由决策结果
#[derive(Debug)]
pub struct RouteDecision {
    pub path: QueryPath,
    #[allow(dead_code)] // 路由原因：保留用于调试输出，业务逻辑未读取
    pub reason: String,
}

// ══════════════════════════════════════════════════════════
// 路由决策函数
// ══════════════════════════════════════════════════════════

/// 路由决策 — 根据问题内容判断走哪条路径
///
/// 对应 Python: route_query()
/// 面试核心：这个函数决定了每个请求的命运。
/// 规则越简单越好——复杂的判断交给 Agent 自己做。
pub fn route_query(question: &str, has_model: bool) -> RouteDecision {
    let q = question.trim().to_lowercase();

    // ── 规则 1：闲聊 / 纯问答 → 直接回答 ──
    let greetings = ["你好", "嗨", "hello", "hi", "你能做什么", "帮助", "help"];
    if greetings.iter().any(|g| q.contains(g)) && q.len() < 20 {
        return RouteDecision {
            path: QueryPath::Direct,
            reason: "简短问候/求助".to_string(),
        };
    }

    // ── 有模型 → 交给 Agent（相信模型）──
    // 模型自行判断问题是否工作相关、是否调用工作记录工具、如何组织回答。
    // 不再用关键词规则强行分类，避免"今天天气怎么样"被时间词误判为工作查询。
    if has_model {
        return RouteDecision {
            path: QueryPath::Agent,
            reason: "交给模型判断意图".to_string(),
        };
    }

    // ── 无模型（基础模板模式）→ 完整覆盖工作查询的统计模板 ──
    // 没有模型可用，只能用规则兜底。fast_answer 对任何带时间范围的工作查询都给统一统计
    // （活动总览 / 分类分布 / Top 应用 / 相关记录），所以这里尽量放宽触发，让"我这周主要做了什么"
    // "今天怎么样""最近忙啥"等工作查询都能拿到统计；仅明显非工作领域（天气/股票/新闻…）放行到 Fallback。
    let non_work_signals = [
        "天气",
        "股票",
        "新闻",
        "笑话",
        "写诗",
        "算命",
        "星座",
        "汇率",
        "翻译成",
    ];
    if non_work_signals.iter().any(|p| q.contains(p)) {
        return RouteDecision {
            path: QueryPath::Fallback,
            reason: "无模型且明显非工作领域，模板兜底".to_string(),
        };
    }
    let work_signals = [
        // 时间词（工作查询常带）
        "今天",
        "昨天",
        "前天",
        "本周",
        "这周",
        "上周",
        "本月",
        "这个月",
        "上月",
        "上个月",
        "最近",
        "这几天",
        "近期",
        // 工作/统计词
        "做了什么",
        "主要做了",
        "忙什么",
        "忙啥",
        "工作",
        "记录",
        "总结",
        "待办",
        "时长",
        "时间",
        "统计",
        "会话",
        "session",
        "效率",
        "占比",
        "比例",
        "分类",
        "应用",
        "进度",
        "进展",
        "回顾",
        "复盘",
        "整理",
    ];
    if work_signals.iter().any(|p| q.contains(p)) {
        return RouteDecision {
            path: QueryPath::Fast,
            reason: "无模型，工作查询走统计模板".to_string(),
        };
    }

    RouteDecision {
        path: QueryPath::Fallback,
        reason: "无模型，模板兜底".to_string(),
    }
}

// ══════════════════════════════════════════════════════════
// Orchestrator 结构体
// ══════════════════════════════════════════════════════════

/// Orchestrator 的处理结果
#[derive(Debug)]
pub struct OrchestratorResult {
    pub answer: String,
    pub used_ai: bool,
    pub tool_labels: Vec<String>,
    /// 工具执行收集的引用记录（Agent 路径来自 executor，其它路径为空）
    pub references: Vec<MemorySearchItem>,
}

/// Orchestrator — Agent 的"指挥官"
///
/// 把 Stage 1-4 的组件组装起来，加上路由决策。
pub struct Orchestrator;

impl Orchestrator {
    /// 处理用户请求的总入口
    ///
    /// 对应 Python: Orchestrator.handle()
    pub async fn handle(
        question: &str,
        model_config: Option<&ModelConfig>,
        database: &Database,
        history: &[Message],
        system_prompt: Option<&str>,
        ignored_apps: &[String],
        excluded_domains: &[String],
        event_tx: Option<mpsc::Sender<StreamEvent>>,
    ) -> Result<OrchestratorResult, AppError> {
        let has_model = model_config
            .map(|c| !c.endpoint.trim().is_empty() && !c.model.trim().is_empty())
            .unwrap_or(false);

        // ① 路由决策
        let decision = route_query(question, has_model);

        // ② 执行对应路径
        match decision.path {
            QueryPath::Direct => {
                let answer = direct_answer(question);
                let tool_labels = vec!["direct".to_string()];
                emit_done(&event_tx, &answer, &[], &tool_labels);
                Ok(OrchestratorResult {
                    answer,
                    used_ai: false,
                    tool_labels,
                    references: vec![],
                })
            }

            QueryPath::Fast => {
                // FastPath：用规则查数据 + 简单格式化
                let answer = fast_answer(question, database, ignored_apps, excluded_domains)?;
                let tool_labels = vec!["rule-based".to_string()];
                emit_done(&event_tx, &answer, &[], &tool_labels);
                Ok(OrchestratorResult {
                    answer,
                    used_ai: false,
                    tool_labels,
                    references: vec![],
                })
            }

            QueryPath::Agent => {
                let config = model_config
                    .ok_or_else(|| AppError::Analysis("Agent 路径需要模型配置".to_string()))?;

                // AgentPath：调用 Stage 3 的 AgentExecutor（透传事件通道）
                match AgentExecutor::run(
                    question,
                    config,
                    database,
                    system_prompt,
                    history,
                    None,
                    ignored_apps.to_vec(),
                    excluded_domains.to_vec(),
                    event_tx.clone(),
                )
                .await
                {
                    Ok(agent_result) => {
                        // Agent 内部各 return 点已 emit Done，此处不重复（避免双 Done）。
                        Ok(OrchestratorResult {
                            answer: agent_result.answer,
                            used_ai: true,
                            tool_labels: agent_result.tool_labels,
                            references: agent_result.references,
                        })
                    }
                    Err(_e) => {
                        // Agent 失败 → 降级到 FastPath（不暴露内部错误细节）
                        let answer =
                            fast_answer(question, database, ignored_apps, excluded_domains)?;
                        let tool_labels = vec!["降级查询".to_string()];
                        emit_done(&event_tx, &answer, &[], &tool_labels);
                        Ok(OrchestratorResult {
                            answer,
                            used_ai: false,
                            tool_labels,
                            references: vec![],
                        })
                    }
                }
            }

            QueryPath::Fallback => {
                let answer = fallback_answer(question);
                let tool_labels = vec!["fallback".to_string()];
                emit_done(&event_tx, &answer, &[], &tool_labels);
                Ok(OrchestratorResult {
                    answer,
                    used_ai: false,
                    tool_labels,
                    references: vec![],
                })
            }
        }
    }
}

/// 推送终态 Done 事件（channel 满/关闭都不影响主流程）。
fn emit_done(
    tx: &Option<mpsc::Sender<StreamEvent>>,
    answer: &str,
    references: &[MemorySearchItem],
    tool_labels: &[String],
) {
    if let Some(tx) = tx {
        let _ = tx.try_send(StreamEvent::Done {
            answer: answer.to_string(),
            references: references.to_vec(),
            tool_labels: tool_labels.to_vec(),
        });
    }
}

// ══════════════════════════════════════════════════════════
// 各路径的实现
// ══════════════════════════════════════════════════════════

/// DirectPath：直接回答
pub fn direct_answer(question: &str) -> String {
    let q = question.to_lowercase();
    let is_chinese = prefers_chinese_answer(question);
    if q.contains("你好") || q.contains("hi") || q.contains("hello") {
        return (if is_chinese {
            "你好！我是你的工作助手，可以帮你分析工作时间、查看记录、对比效率等。请问你想了解什么？"
        } else {
            "Hello! I'm your work assistant — I can help you analyze work time, review records, compare efficiency, and more. What would you like to know?"
        })
        .to_string();
    }
    if q.contains("你能做什么") || q.contains("帮助") || q.contains("help") {
        return (if is_chinese {
            "我可以帮你：\n1. 查看某天/某周的工作记录\n2. 分析时间分布（编码/会议/文档占比）\n3. 对比不同时间段的效率变化\n4. 搜索特定的工作内容\n请告诉我你想了解什么？"
        } else {
            "I can help you:\n1. Review work records for a day/week\n2. Analyze time distribution (coding/meetings/docs)\n3. Compare efficiency across periods\n4. Search for specific work items\nWhat would you like to know?"
        })
        .to_string();
    }
    (if is_chinese {
        "请告诉我你想了解的工作信息。"
    } else {
        "Tell me what work info you'd like to know."
    })
    .to_string()
}

/// FastPath：规则快速查询
pub fn fast_answer(
    question: &str,
    database: &Database,
    ignored_apps: &[String],
    excluded_domains: &[String],
) -> Result<String, AppError> {
    use work_review_core::categorize::{
        categorize_app, get_category_name, normalize_display_app_name,
    };

    // 复用 parse_temporal_range（你在 Stage 0 修复过的函数）
    let (date_from, date_to) = crate::commands::parse_temporal_range(question);

    // 策略：先按时间范围加载活动，再按分类聚合
    let activities = database
        .get_activities_in_range(date_from.as_deref(), date_to.as_deref(), 10000)
        .map_err(|e| AppError::Analysis(format!("查询失败: {e}")))?;
    // 应用隐私过滤：fast_answer 结果会直接展示给用户，不应出现被"忽略应用"/
    // "排除域名"的窗口标题（与其它统计命令保持一致）。
    let activities =
        crate::commands::filter_activities_by_privacy(activities, ignored_apps, excluded_domains);

    if activities.is_empty() {
        let is_chinese = prefers_chinese_answer(question);
        return Ok(if is_chinese {
            format!(
                "在 {} ~ {} 范围内未找到活动记录。",
                date_from.as_deref().unwrap_or("全部"),
                date_to.as_deref().unwrap_or("今天")
            )
        } else {
            format!(
                "No activity records found in {} ~ {}.",
                date_from.as_deref().unwrap_or("All"),
                date_to.as_deref().unwrap_or("Today")
            )
        });
    }

    // 按分类聚合
    let mut category_durations: std::collections::HashMap<String, i64> =
        std::collections::HashMap::new();
    let mut app_durations: std::collections::HashMap<String, i64> =
        std::collections::HashMap::new();

    for a in &activities {
        let cat = categorize_app(&a.app_name, &a.window_title);
        *category_durations.entry(cat).or_insert(0) += a.duration;
        let display = normalize_display_app_name(&a.app_name);
        *app_durations.entry(display).or_insert(0) += a.duration;
    }

    let total: i64 = activities.iter().map(|a| a.duration).sum();
    let mut sorted_cats: Vec<_> = category_durations.into_iter().collect();
    sorted_cats.sort_by(|a, b| b.1.cmp(&a.1));

    let mut sorted_apps: Vec<_> = app_durations.into_iter().collect();
    sorted_apps.sort_by(|a, b| b.1.cmp(&a.1));
    sorted_apps.truncate(5);

    // 格式化时长
    let fmt_dur = |s: i64| -> String {
        let h = s / 3600;
        let m = (s % 3600) / 60;
        if h > 0 {
            format!("{h}h{m}m")
        } else if m > 0 {
            format!("{m}m")
        } else {
            format!("{s}s")
        }
    };

    // 跟随用户提问语言（CJK -> 中文，否则英文）
    let is_chinese = prefers_chinese_answer(question);
    let (lbl_overview, lbl_records, lbl_category, lbl_top_apps, lbl_related) = if is_chinese {
        ("活动总览", "条记录，总时长", "分类分布", "使用最多的应用", "相关记录")
    } else {
        ("Activity overview", "records, total", "Category breakdown", "Top apps", "Related records")
    };

    let mut lines = vec![format!(
        "{} ~ {} {}：",
        date_from.as_deref().unwrap_or(if is_chinese { "全部" } else { "All" }),
        date_to.as_deref().unwrap_or(if is_chinese { "今天" } else { "Today" }),
        lbl_overview
    )];
    lines.push(format!(
        "{} {} {}",
        activities.len(),
        lbl_records,
        fmt_dur(total)
    ));
    lines.push("".to_string());

    // 分类分布
    lines.push(format!("{}：", lbl_category));
    for (cat_key, dur) in &sorted_cats {
        let cat_display = if is_chinese {
            get_category_name(cat_key).to_string()
        } else {
            match cat_key.as_str() {
                "development" => "Development",
                "browser" => "Browser",
                "communication" => "Communication",
                "office" => "Office",
                "design" => "Design",
                "entertainment" => "Leisure",
                "other" => "Other",
                _ => cat_key,
            }
            .to_string()
        };
        let pct = if total > 0 {
            *dur as f64 / total as f64 * 100.0
        } else {
            0.0
        };
        lines.push(format!("  - {cat_display}: {} ({pct:.0}%)", fmt_dur(*dur)));
    }

    // Top 5 应用
    lines.push("".to_string());
    lines.push(format!("{}：", lbl_top_apps));
    for (app, dur) in &sorted_apps {
        lines.push(format!("  - {app}: {}", fmt_dur(*dur)));
    }

    // 如果有 FTS 关键词命中的结果，也附上
    let fts_results = database
        .search_memory(question, date_from.as_deref(), date_to.as_deref(), 3)
        .unwrap_or_default();
    if !fts_results.is_empty() {
        lines.push("".to_string());
        lines.push(format!("{}：", lbl_related));
        for r in &fts_results {
            lines.push(format!("- {} | {}", r.date, r.title));
        }
    }

    Ok(lines.join("\n"))
}

fn prefers_chinese_answer(question: &str) -> bool {
    question.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
}

/// FallbackPath：无模型时的模板回答
fn fallback_answer(question: &str) -> String {
    if prefers_chinese_answer(question) {
        "我目前无法使用 AI 模型进行分析，但你可以尝试：\n\
         - 询问具体某天的工作记录\n\
         - 使用时间关键词（今天、昨天、本周等）\n\
         - 配置 AI 模型后可以获得更智能的分析"
            .to_string()
    } else {
        "I can't use an AI model for analysis right now, but you can try:\n\
         - Asking for work records from a specific day\n\
         - Using time keywords such as today, yesterday, or this week\n\
         - Configuring an AI model for smarter analysis"
            .to_string()
    }
}

// ══════════════════════════════════════════════════════════
// 测试
// ══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_greeting() {
        let d = route_query("你好", true);
        assert_eq!(d.path, QueryPath::Direct);
    }

    #[test]
    fn test_route_simple_time_query() {
        // 简化后：有模型即交给 Agent，由模型决定是否调用工作记录工具。
        let d = route_query("今天做了什么", true);
        assert_eq!(d.path, QueryPath::Agent);
    }

    #[test]
    fn test_route_simple_time_query_month() {
        let d = route_query("这个月的时间分布", true);
        assert_eq!(d.path, QueryPath::Agent);
    }

    #[test]
    fn test_route_non_work_weather_uses_agent() {
        // 非工作问题（天气）也交给模型，不再被时间词"今天"误判为工作查询。
        let d = route_query("今天天气怎么样", true);
        assert_eq!(d.path, QueryPath::Agent);
    }

    #[test]
    fn test_route_non_work_weather_no_model_fallback() {
        // 无模型时无法由模型判断，走模板兜底。
        let d = route_query("今天天气怎么样", false);
        assert_eq!(d.path, QueryPath::Fallback);
    }

    #[test]
    fn test_route_complex_comparison() {
        let d = route_query("对比上个月和这个月的工作效率", true);
        assert_eq!(d.path, QueryPath::Agent);
    }

    #[test]
    fn test_route_complex_why() {
        let d = route_query("为什么最近编码时间下降了", true);
        assert_eq!(d.path, QueryPath::Agent);
    }

    #[test]
    fn test_route_multi_time_periods() {
        // 这个问题同时命中"变化"（规则2）和"上月+这个月"（规则3）
        // 规则2先匹配，所以走 Agent 路径，理由是"复杂意图"
        let d = route_query("上个月和这个月有什么变化", true);
        assert_eq!(d.path, QueryPath::Agent);
        // 两个规则都可能命中，关键是走了 Agent 路径
    }

    #[test]
    fn test_route_pure_multi_time_periods() {
        // 简化后统一交给模型，不再按时间段数量分流。
        let d = route_query("上个月和这个月的工作记录", true);
        assert_eq!(d.path, QueryPath::Agent);
    }

    #[test]
    fn test_route_no_model_time_word_fast() {
        // 放宽后：含时间词的工作查询走 Fast（基础模板给统计），不再 Fallback。
        let d = route_query("对比上个月和这个月", false);
        assert_eq!(d.path, QueryPath::Fast);
    }

    #[test]
    fn test_route_no_model_work_query_fast() {
        // 无模型（基础模板）：明确工作查询走 FastPath 统计模板，得到有意义内容。
        let d = route_query("我这周主要做了什么", false);
        assert_eq!(d.path, QueryPath::Fast);
    }

    #[test]
    fn test_route_no_model_non_work_fallback() {
        // 无模型且非明确工作查询 → 模板兜底指引。
        let d = route_query("随便聊聊", false);
        assert_eq!(d.path, QueryPath::Fallback);
    }

    #[test]
    fn test_route_unknown_with_model() {
        let d = route_query("帮我看看效率情况", true);
        assert_eq!(d.path, QueryPath::Agent); // 兜底走 Agent
    }

    #[test]
    fn test_route_unknown_without_model() {
        // 放宽后："效率"是工作信号 → 无模型走 Fast（基础模板给统计）。
        let d = route_query("帮我看看效率情况", false);
        assert_eq!(d.path, QueryPath::Fast);
    }

    #[test]
    fn test_direct_answer_greeting() {
        let answer = direct_answer("你好");
        assert!(answer.contains("工作助手"));
    }

    #[test]
    fn test_fallback_answer() {
        let answer = fallback_answer("随便聊聊");
        assert!(answer.contains("AI 模型"));
    }

    #[test]
    fn test_fallback_answer_follows_english_question() {
        let answer = fallback_answer("Can you chat with me?");
        assert!(answer.contains("AI model"));
        assert!(!answer.contains("我目前无法使用"));
    }
}
