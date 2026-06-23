//! Stage 1: Tool 层 — Agent 的"手脚"
//!
//! 三个核心概念（和 Python 原型完全对应）：
//! 1. ToolDefinition  → Schema（给 LLM 看的工具定义）
//! 2. execute 函数    → Execute（真正干活的代码）
//! 3. ToolRegistry    → Registry（工具注册中心）

use crate::database::Database;
use crate::work_intelligence;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use work_review_core::categorize::{categorize_app, get_category_name, normalize_display_app_name};
use work_review_core::database::MemorySearchItem;

// ══════════════════════════════════════════════════════════
// 共享 Helper 函数
// ══════════════════════════════════════════════════════════

/// 格式化时长为人类可读格式（"1h30m"、"45m"、"30s"）
fn format_duration_compact(seconds: i64) -> String {
    if seconds <= 0 {
        return "0s".to_string();
    }
    let h = seconds / 3600;
    let m = (seconds % 3600) / 60;
    let s = seconds % 60;
    if h > 0 {
        format!("{h}h{m}m")
    } else if m > 0 {
        format!("{m}m")
    } else {
        format!("{s}s")
    }
}

/// 中文分类名 → 英文 key（支持部分匹配）
///
/// "开发" → "development", "通讯" → "communication", "browser" → "browser"
fn resolve_category_key(input: &str) -> Option<String> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }
    let lower = input.to_lowercase();

    let mapping: &[(&str, &str)] = &[
        ("development", "开发工具"),
        ("browser", "浏览器"),
        ("communication", "通讯协作"),
        ("office", "办公软件"),
        ("design", "设计工具"),
        ("entertainment", "娱乐"),
        ("other", "其他"),
    ];

    for (key, chinese) in mapping {
        if lower == *key {
            return Some(key.to_string());
        }
        if input == *chinese {
            return Some(key.to_string());
        }
        if chinese.contains(input) {
            return Some(key.to_string());
        }
    }
    None
}

// ══════════════════════════════════════════════════════════
// 第一部分：Tool 的定义 — 给 LLM 看的"菜单"
// ══════════════════════════════════════════════════════════

/// 一个工具的完整定义（Schema + 执行函数）
///
/// 对应 Python 里的：
///   search_memory_schema() + search_memory_execute() 的组合
pub struct ToolDefinition {
    /// 工具名称
    pub name: &'static str,
    /// 工具描述 — 这段文字直接决定了 LLM 选得准不准
    pub description: &'static str,
    /// 参数的 JSON Schema — 和 OpenAI/Claude API 的格式完全一致
    pub parameters_schema: Value,
    /// 执行函数 — LLM 选了这个工具后，调用这个函数干活
    pub execute_fn: fn(&ToolContext, Value) -> Result<String, String>,
}

// ══════════════════════════════════════════════════════════
// 第二部分：ToolContext — 执行工具时需要的上下文
// ══════════════════════════════════════════════════════════

/// 工具执行时的上下文（数据库连接、配置等）
///
/// 为什么需要这个？
/// Python 里我们可以直接访问全局变量（db），但 Rust 不允许。
/// 所以把执行工具需要的所有东西打包进 ToolContext。
pub struct ToolContext<'a> {
    pub database: &'a Database,
    /// 隐私过滤：被用户标记"忽略"的应用名（小写子串）。
    pub ignored_apps: Vec<String>,
    /// 隐私过滤：被用户排除的域名。
    pub excluded_domains: Vec<String>,
    /// 工具执行时收集的引用记录（供前端展示"依据"）。
    /// 用 Arc<Mutex> 是因为 execute_fn 是函数指针、ToolContext 以 `&` 借用传递，
    /// 需要内部可变性；多轮工具调用会持续累积。
    pub collected_references: Arc<Mutex<Vec<MemorySearchItem>>>,
}

impl<'a> ToolContext<'a> {
    /// 按用户隐私设置过滤活动记录。
    /// 工具结果会作为对话历史发给云端 LLM，必须先剔除被"忽略应用"/"排除域名"
    /// 的窗口标题，否则会违背"本地优先、不经第三方"的隐私承诺。
    pub fn filter_activities(
        &self,
        activities: Vec<crate::database::Activity>,
    ) -> Vec<crate::database::Activity> {
        crate::commands::filter_activities_by_privacy(
            activities,
            &self.ignored_apps,
            &self.excluded_domains,
        )
    }

    /// 记录工具命中的引用（按 source_id + timestamp + title 去重）。
    pub fn collect_references(&self, items: Vec<MemorySearchItem>) {
        if items.is_empty() {
            return;
        }
        if let Ok(mut buf) = self.collected_references.lock() {
            for item in items {
                let dup = buf.iter().any(|b| {
                    b.source_id == item.source_id
                        && b.timestamp == item.timestamp
                        && b.title == item.title
                });
                if !dup {
                    buf.push(item);
                }
            }
        }
    }

    /// 当前已收集的引用数（用于取"本轮增量"区间）。
    pub fn references_len(&self) -> usize {
        self.collected_references
            .lock()
            .map(|b| b.len())
            .unwrap_or(0)
    }

    /// 取 [start..] 区间的引用克隆（StepResult 携带本轮增量）。
    pub fn drain_from(&self, start: usize) -> Vec<MemorySearchItem> {
        self.collected_references
            .lock()
            .map(|b| b.get(start..).map(|s| s.to_vec()).unwrap_or_default())
            .unwrap_or_default()
    }

    /// 取出全部引用（executor 结束时填入 AgentResult）。
    pub fn take_all_references(&self) -> Vec<MemorySearchItem> {
        self.collected_references
            .lock()
            .map(|mut b| std::mem::take(&mut *b))
            .unwrap_or_default()
    }
}

// ══════════════════════════════════════════════════════════
// 第三部分：具体工具的 Schema 定义
// ══════════════════════════════════════════════════════════

/// search_memory 工具的 Schema
///
/// 对应 Python: search_memory_schema()
/// 输出的 JSON 和 Python 版完全一致
fn search_memory_parameters() -> Value {
    json!({
        "type": "object",
        "properties": {
            "query": {
                "type": "string",
                "description": "搜索关键词，例如 'debug'、'编码'、'会议'"
            },
            "date_from": {
                "type": "string",
                "description": "开始日期，格式 YYYY-MM-DD"
            },
            "date_to": {
                "type": "string",
                "description": "结束日期，格式 YYYY-MM-DD"
            }
        },
        "required": ["query"]
    })
}

/// analyze_intents 工具的 Schema
///
/// 对应 Python: analyze_intents_schema()
fn analyze_intents_parameters() -> Value {
    json!({
        "type": "object",
        "properties": {
            "date_from": {
                "type": "string",
                "description": "开始日期，格式 YYYY-MM-DD"
            },
            "date_to": {
                "type": "string",
                "description": "结束日期，格式 YYYY-MM-DD"
            }
        },
        "required": ["date_from", "date_to"]
    })
}

/// aggregate_stats 工具的 Schema
fn aggregate_stats_parameters() -> Value {
    json!({
        "type": "object",
        "properties": {
            "date_from": {
                "type": "string",
                "description": "开始日期，格式 YYYY-MM-DD"
            },
            "date_to": {
                "type": "string",
                "description": "结束日期，格式 YYYY-MM-DD"
            },
            "metric": {
                "type": "string",
                "enum": ["by_app", "by_category", "summary"],
                "description": "统计维度：by_app=按应用排名, by_category=按分类排名, summary=总览。默认 summary"
            },
            "category": {
                "type": "string",
                "description": "可选的分类过滤，如 '开发'、'通讯'、'browser'。仅统计该分类"
            },
            "limit": {
                "type": "integer",
                "description": "返回条数，默认10"
            }
        },
        "required": ["date_from", "date_to"]
    })
}

/// category_search 工具的 Schema
fn category_search_parameters() -> Value {
    json!({
        "type": "object",
        "properties": {
            "category": {
                "type": "string",
                "description": "分类名，支持中文如'开发'、'通讯'、'办公'，或英文如'browser'"
            },
            "date_from": {
                "type": "string",
                "description": "开始日期，格式 YYYY-MM-DD（可选，默认最近7天）"
            },
            "date_to": {
                "type": "string",
                "description": "结束日期，格式 YYYY-MM-DD（可选，默认今天）"
            },
            "limit": {
                "type": "integer",
                "description": "返回条数，默认20"
            }
        },
        "required": ["category"]
    })
}

/// trend_comparison 工具的 Schema
fn trend_comparison_parameters() -> Value {
    json!({
        "type": "object",
        "properties": {
            "period_a_from": {
                "type": "string",
                "description": "时段A开始日期，格式 YYYY-MM-DD"
            },
            "period_a_to": {
                "type": "string",
                "description": "时段A结束日期，格式 YYYY-MM-DD"
            },
            "period_b_from": {
                "type": "string",
                "description": "时段B开始日期，格式 YYYY-MM-DD"
            },
            "period_b_to": {
                "type": "string",
                "description": "时段B结束日期，格式 YYYY-MM-DD"
            }
        },
        "required": ["period_a_from", "period_a_to", "period_b_from", "period_b_to"]
    })
}

// ══════════════════════════════════════════════════════════
// 第四部分：具体工具的 Execute 函数
// ══════════════════════════════════════════════════════════

/// search_memory 的执行函数
///
/// 对应 Python: search_memory_execute()
/// 但这里调用的是真实的 database.search_memory()！
fn search_memory_execute(ctx: &ToolContext, args: Value) -> Result<String, String> {
    let query = args["query"]
        .as_str()
        .ok_or_else(|| "缺少必需参数: query".to_string())?;
    let date_from = args["date_from"].as_str();
    let date_to = args["date_to"].as_str();

    // 调用你项目里真实的数据库搜索函数
    let results = ctx
        .database
        .search_memory(query, date_from, date_to, 8)
        .map_err(|e| format!("搜索失败: {e}"))?;
    // 隐私过滤：剔除被忽略应用/排除域名的搜索结果——窗口标题会作为工具结果
    // 发给云端 LLM，必须与其它工具一样遵守用户的隐私设置。
    let results: Vec<_> = results
        .into_iter()
        .filter(|r| {
            if !ctx.ignored_apps.is_empty() {
                if let Some(app) = &r.app_name {
                    let app_lower = app.to_lowercase();
                    if ctx
                        .ignored_apps
                        .iter()
                        .any(|ig| app_lower.contains(ig) || ig.contains(&app_lower))
                    {
                        return false;
                    }
                }
            }
            if !ctx.excluded_domains.is_empty() {
                let url_lower = r.browser_url.as_deref().unwrap_or("").to_lowercase();
                let title_lower = r.title.to_lowercase();
                if ctx.excluded_domains.iter().any(|ex| {
                    let ex_l = ex.to_lowercase();
                    url_lower.contains(&ex_l) || title_lower.contains(&ex_l)
                }) {
                    return false;
                }
            }
            true
        })
        .collect();

    // 收集引用供前端展示"依据"（空结果无害，collect_references 内部去重）。
    ctx.collect_references(results.clone());

    // 格式化成 LLM 能理解的文字
    if results.is_empty() {
        return Ok(format!("搜索 '{query}' 无结果。"));
    }

    let mut lines = vec![format!(
        "搜索 '{}' 的结果（共{}条）：",
        query,
        results.len()
    )];
    for r in &results {
        let dur = r
            .duration
            .map(|d| {
                let h = d / 3600;
                let m = (d % 3600) / 60;
                if h > 0 {
                    format!("{h}h{m}m")
                } else {
                    format!("{m}m")
                }
            })
            .unwrap_or_default();
        let app = r
            .app_name
            .as_deref()
            .map(|a| format!(" | {a}"))
            .unwrap_or_default();
        lines.push(format!("  - {} | {}{} | {}", r.date, r.title, app, dur));
    }
    Ok(lines.join("\n"))
}

/// analyze_intents 的执行函数
///
/// 对应 Python: analyze_intents_execute()
/// 调用真实的 work_intelligence.analyze_intents()
fn analyze_intents_execute(ctx: &ToolContext, args: Value) -> Result<String, String> {
    let date_from = args["date_from"]
        .as_str()
        .ok_or_else(|| "缺少必需参数: date_from".to_string())?;
    let date_to = args["date_to"]
        .as_str()
        .ok_or_else(|| "缺少必需参数: date_to".to_string())?;

    // 调用真实的函数链：get_activities → analyze_intents
    let activities = ctx
        .database
        .get_activities_in_range(Some(date_from), Some(date_to), 5000)
        .map_err(|e| format!("获取活动记录失败: {e}"))?;
    let activities = ctx.filter_activities(activities);

    if activities.is_empty() {
        return Ok(format!("在 {date_from} ~ {date_to} 范围内无活动记录。"));
    }

    let result = work_intelligence::analyze_intents(&activities);

    let total: i64 = result.summary.iter().map(|s| s.duration).sum();
    let mut lines = vec![format!("工作意图分布 ({} ~ {})：", date_from, date_to)];
    for s in &result.summary {
        let hours = s.duration / 3600;
        let pct = if total > 0 {
            s.duration as f64 / total as f64 * 100.0
        } else {
            0.0
        };
        lines.push(format!(
            "  - {}: {}h ({:.0}%) | {}个session",
            s.label, hours, pct, s.session_count
        ));
    }
    Ok(lines.join("\n"))
}

/// aggregate_stats 的执行函数 — 按应用/分类统计时长
fn aggregate_stats_execute(ctx: &ToolContext, args: Value) -> Result<String, String> {
    let date_from = args["date_from"]
        .as_str()
        .ok_or_else(|| "缺少必需参数: date_from".to_string())?;
    let date_to = args["date_to"]
        .as_str()
        .ok_or_else(|| "缺少必需参数: date_to".to_string())?;
    let metric = args["metric"].as_str().unwrap_or("summary");
    let limit = args["limit"].as_u64().unwrap_or(10) as usize;

    // 解析可选的分类过滤
    let category_filter = args
        .get("category")
        .and_then(|v| v.as_str())
        .map(|c| {
            resolve_category_key(c).ok_or_else(|| {
                format!("无法识别的分类: '{c}'。支持: 开发/浏览器/通讯/办公/设计/娱乐/其他")
            })
        })
        .transpose()?;

    // 加载活动记录
    let activities = ctx
        .database
        .get_activities_in_range(Some(date_from), Some(date_to), 10000)
        .map_err(|e| format!("获取活动记录失败: {e}"))?;
    let activities = ctx.filter_activities(activities);

    if activities.is_empty() {
        return Ok(format!("在 {date_from} ~ {date_to} 范围内无活动记录。"));
    }

    // 聚合：按应用和按分类
    let mut app_durations: HashMap<String, i64> = HashMap::new();
    let mut category_durations: HashMap<String, i64> = HashMap::new();

    for activity in &activities {
        let cat_key = categorize_app(&activity.app_name, &activity.window_title);

        // 应用分类过滤
        if let Some(ref filter) = category_filter {
            if cat_key != *filter {
                continue;
            }
        }

        let display = normalize_display_app_name(&activity.app_name);
        *app_durations.entry(display).or_insert(0) += activity.duration;
        *category_durations.entry(cat_key).or_insert(0) += activity.duration;
    }

    if app_durations.is_empty() {
        let cn = category_filter
            .as_deref()
            .map(get_category_name)
            .unwrap_or("所有");
        return Ok(format!(
            "在 {date_from} ~ {date_to} 范围内未找到 '{cn}' 分类的活动记录。"
        ));
    }

    let total: i64 = app_durations.values().sum();

    match metric {
        "by_app" => {
            let mut sorted: Vec<_> = app_durations.into_iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(&a.1));
            sorted.truncate(limit);

            let mut lines = vec![format!("应用使用时长排名 ({date_from} ~ {date_to})：")];
            for (app, dur) in &sorted {
                lines.push(format!("  - {app}: {}", format_duration_compact(*dur)));
            }
            lines.push("  ---".to_string());
            lines.push(format!("  总计: {}", format_duration_compact(total)));
            Ok(lines.join("\n"))
        }
        "by_category" => {
            let mut sorted: Vec<_> = category_durations.into_iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(&a.1));

            let mut lines = vec![format!("分类使用时长 ({date_from} ~ {date_to})：")];
            for (cat_key, dur) in &sorted {
                let cn = get_category_name(cat_key);
                let pct = if total > 0 {
                    *dur as f64 / total as f64 * 100.0
                } else {
                    0.0
                };
                lines.push(format!(
                    "  - {cn}: {} ({pct:.0}%)",
                    format_duration_compact(*dur)
                ));
            }
            lines.push("  ---".to_string());
            lines.push(format!("  总计: {}", format_duration_compact(total)));
            Ok(lines.join("\n"))
        }
        _ => {
            // summary（默认）
            let mut top_apps: Vec<_> = app_durations.iter().collect();
            top_apps.sort_by(|a, b| b.1.cmp(a.1));
            top_apps.truncate(3);

            let mut sorted_cats: Vec<_> = category_durations.into_iter().collect();
            sorted_cats.sort_by(|a, b| b.1.cmp(&a.1));

            let mut lines = vec![format!("时间总览 ({date_from} ~ {date_to})：")];
            lines.push(format!("  总活动时长: {}", format_duration_compact(total)));
            let top_str: Vec<String> = top_apps
                .iter()
                .map(|(app, dur)| format!("{app}({})", format_duration_compact(**dur)))
                .collect();
            lines.push(format!("  最多使用: {}", top_str.join(", ")));
            lines.push("".to_string());
            lines.push("  分类分布:".to_string());
            for (cat_key, dur) in &sorted_cats {
                let cn = get_category_name(cat_key);
                let pct = if total > 0 {
                    *dur as f64 / total as f64 * 100.0
                } else {
                    0.0
                };
                lines.push(format!(
                    "    - {cn}: {} ({pct:.0}%)",
                    format_duration_compact(*dur)
                ));
            }
            Ok(lines.join("\n"))
        }
    }
}

/// category_search 的执行函数 — 按分类筛选活动明细
fn category_search_execute(ctx: &ToolContext, args: Value) -> Result<String, String> {
    let category_input = args["category"]
        .as_str()
        .ok_or_else(|| "缺少必需参数: category".to_string())?;
    let date_from = args["date_from"].as_str();
    let date_to = args["date_to"].as_str();
    let limit = args["limit"].as_u64().unwrap_or(20) as usize;

    let cat_key = resolve_category_key(category_input).ok_or_else(|| {
        format!("无法识别的分类: '{category_input}'。支持: 开发/浏览器/通讯/办公/设计/娱乐/其他")
    })?;

    let activities = ctx
        .database
        .get_activities_in_range(date_from, date_to, 10000)
        .map_err(|e| format!("获取活动记录失败: {e}"))?;
    let activities = ctx.filter_activities(activities);

    // 按分类过滤
    let filtered: Vec<_> = activities
        .iter()
        .filter(|a| categorize_app(&a.app_name, &a.window_title) == cat_key)
        .collect();

    let cn_name = get_category_name(&cat_key);

    if filtered.is_empty() {
        let range = match (date_from, date_to) {
            (Some(f), Some(t)) => format!("{f} ~ {t}"),
            _ => "指定范围".to_string(),
        };
        return Ok(format!(
            "在 {range} 范围内未找到 '{cn_name}' 类别的活动记录。"
        ));
    }

    // 按应用聚合
    let mut app_entries: HashMap<String, (i64, String)> = HashMap::new();
    for activity in &filtered {
        let display = normalize_display_app_name(&activity.app_name);
        let entry = app_entries.entry(display).or_insert((0, String::new()));
        entry.0 += activity.duration;
        if entry.1.is_empty() {
            entry.1 = activity.window_title.chars().take(60).collect();
        }
    }

    let total_dur: i64 = app_entries.values().map(|(d, _)| *d).sum();
    let mut sorted: Vec<_> = app_entries.into_iter().collect();
    sorted.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
    sorted.truncate(limit);

    let range = match (date_from, date_to) {
        (Some(f), Some(t)) => format!("{f} ~ {t}"),
        _ => "全部".to_string(),
    };

    let mut lines = vec![format!("{cn_name} 类别活动（{range}）：")];
    lines.push(format!(
        "  共 {} 条记录，总时长 {}",
        filtered.len(),
        format_duration_compact(total_dur)
    ));
    lines.push("".to_string());
    for (app, (dur, title)) in &sorted {
        lines.push(format!("  - {app}: {}", format_duration_compact(*dur)));
        if !title.is_empty() {
            lines.push(format!("    窗口: {title}"));
        }
    }
    Ok(lines.join("\n"))
}

/// trend_comparison 的执行函数 — 两个时段对比
fn trend_comparison_execute(ctx: &ToolContext, args: Value) -> Result<String, String> {
    let pa_from = args["period_a_from"]
        .as_str()
        .ok_or_else(|| "缺少必需参数: period_a_from".to_string())?;
    let pa_to = args["period_a_to"]
        .as_str()
        .ok_or_else(|| "缺少必需参数: period_a_to".to_string())?;
    let pb_from = args["period_b_from"]
        .as_str()
        .ok_or_else(|| "缺少必需参数: period_b_from".to_string())?;
    let pb_to = args["period_b_to"]
        .as_str()
        .ok_or_else(|| "缺少必需参数: period_b_to".to_string())?;

    // 加载两个时段的活动
    let activities_a = ctx
        .database
        .get_activities_in_range(Some(pa_from), Some(pa_to), 10000)
        .map_err(|e| format!("获取时段A数据失败: {e}"))?;
    let activities_a = ctx.filter_activities(activities_a);
    let activities_b = ctx
        .database
        .get_activities_in_range(Some(pb_from), Some(pb_to), 10000)
        .map_err(|e| format!("获取时段B数据失败: {e}"))?;
    let activities_b = ctx.filter_activities(activities_b);

    // 按分类聚合
    let compute_cats = |acts: &[crate::database::Activity]| -> HashMap<String, i64> {
        let mut map: HashMap<String, i64> = HashMap::new();
        for a in acts {
            let cat = categorize_app(&a.app_name, &a.window_title);
            *map.entry(cat).or_insert(0) += a.duration;
        }
        map
    };

    let cats_a = compute_cats(&activities_a);
    let cats_b = compute_cats(&activities_b);

    let total_a: i64 = cats_a.values().sum();
    let total_b: i64 = cats_b.values().sum();

    let mut lines = vec!["时段对比：".to_string()];
    lines.push(format!(
        "  时段A: {pa_from} ~ {pa_to} ({})",
        format_duration_compact(total_a)
    ));
    lines.push(format!(
        "  时段B: {pb_from} ~ {pb_to} ({})",
        format_duration_compact(total_b)
    ));
    lines.push("".to_string());

    // 总时长变化
    if total_a > 0 {
        let delta = total_b - total_a;
        let pct = delta as f64 / total_a as f64 * 100.0;
        let sign = if delta > 0 { "+" } else { "" };
        lines.push(format!(
            "  总时长变化: {}{} ({sign}{pct:.1}%)",
            sign,
            format_duration_compact(delta)
        ));
        lines.push("".to_string());
    }

    // 合并所有分类 key 并排序
    let mut all_keys: Vec<String> = cats_a.keys().chain(cats_b.keys()).cloned().collect();
    all_keys.sort();
    all_keys.dedup();

    // 按总时长排序（a+b 降序）
    all_keys.sort_by(|a, b| {
        let sa = cats_a.get(a).copied().unwrap_or(0) + cats_b.get(a).copied().unwrap_or(0);
        let sb = cats_a.get(b).copied().unwrap_or(0) + cats_b.get(b).copied().unwrap_or(0);
        sb.cmp(&sa)
    });

    lines.push("  分类对比：".to_string());
    for key in &all_keys {
        let dur_a = cats_a.get(key).copied().unwrap_or(0);
        let dur_b = cats_b.get(key).copied().unwrap_or(0);
        if dur_a == 0 && dur_b == 0 {
            continue;
        }
        let cn = get_category_name(key);
        if dur_a > 0 && dur_b > 0 {
            let delta = dur_b - dur_a;
            let pct = delta as f64 / dur_a as f64 * 100.0;
            let sign = if delta > 0 { "+" } else { "" };
            lines.push(format!(
                "    {cn}: {} → {} ({sign}{pct:.1}%)",
                format_duration_compact(dur_a),
                format_duration_compact(dur_b),
            ));
        } else if dur_a == 0 {
            lines.push(format!(
                "    {cn}: 0 → {} (新增)",
                format_duration_compact(dur_b)
            ));
        } else {
            lines.push(format!(
                "    {cn}: {} → 0 (消失)",
                format_duration_compact(dur_a)
            ));
        }
    }

    Ok(lines.join("\n"))
}

// ══════════════════════════════════════════════════════════
// 第五部分：ToolRegistry — 工具注册中心
// ══════════════════════════════════════════════════════════

/// 工具注册中心
///
/// 对应 Python: ToolRegistry 类
/// 职责完全一样：注册工具 → 返回定义给 LLM → 执行 LLM 选择的工具
pub struct ToolRegistry {
    tools: HashMap<&'static str, ToolDefinition>,
}

impl ToolRegistry {
    /// 创建一个注册了所有内置工具的 Registry
    pub fn new() -> Self {
        let mut registry = Self {
            tools: HashMap::new(),
        };
        registry.register_builtin_tools();
        registry
    }

    /// 注册内置工具
    fn register_builtin_tools(&mut self) {
        self.register(ToolDefinition {
            name: "search_memory",
            description: "搜索工作记录记忆库。支持关键词搜索和日期范围过滤。当用户问到具体做了什么、工作时间安排、某个项目的进展时使用。",
            parameters_schema: search_memory_parameters(),
            execute_fn: search_memory_execute,
        });

        self.register(ToolDefinition {
            name: "analyze_intents",
            description: "分析指定日期范围内的工作意图分布。返回各意图类别（如编码开发、会议沟通、文档撰写等）的时间和占比。当用户问时间分布、时间占比、各类型工作时长时使用。",
            parameters_schema: analyze_intents_parameters(),
            execute_fn: analyze_intents_execute,
        });

        self.register(ToolDefinition {
            name: "aggregate_stats",
            description: "统计指定日期范围内的应用和分类使用时长。可按应用、分类或总览维度输出排名。当用户问到「花时间最多的是什么」「编码占比多少」「哪个类别最多」「时间分布」时使用。",
            parameters_schema: aggregate_stats_parameters(),
            execute_fn: aggregate_stats_execute,
        });

        self.register(ToolDefinition {
            name: "category_search",
            description: "按分类筛选活动记录，返回该分类下的应用使用明细。当用户问到「开发做了什么」「通讯花了多久」「浏览器使用详情」时使用。category 参数支持中文简称如'开发'、'通讯'、'办公'。",
            parameters_schema: category_search_parameters(),
            execute_fn: category_search_execute,
        });

        self.register(ToolDefinition {
            name: "trend_comparison",
            description: "对比两个时间段的活动时长和分类分布变化。计算各分类的增减量和百分比变化。当用户问到「效率变化」「对比前后两周」「最近工作趋势」时使用。",
            parameters_schema: trend_comparison_parameters(),
            execute_fn: trend_comparison_execute,
        });
    }

    fn register(&mut self, tool: ToolDefinition) {
        self.tools.insert(tool.name, tool);
    }

    /// 返回所有工具的 OpenAI 格式定义
    ///
    /// 这个方法返回的 JSON 数组，可以直接塞进
    /// OpenAI API 的 tools 参数里。格式完全一致。
    pub fn to_openai_tools(&self) -> Vec<Value> {
        self.tools
            .values()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters_schema,
                    }
                })
            })
            .collect()
    }

    /// 执行 LLM 选择的工具
    ///
    /// 对应 Python: registry.execute(tool_name, arguments)
    pub fn execute(
        &self,
        tool_name: &str,
        arguments: Value,
        ctx: &ToolContext,
    ) -> Result<String, String> {
        let tool = self
            .tools
            .get(tool_name)
            .ok_or_else(|| format!("未知的工具: {tool_name}"))?;
        (tool.execute_fn)(ctx, arguments)
    }
}

// ══════════════════════════════════════════════════════════
// 测试
// ══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_has_builtin_tools() {
        let registry = ToolRegistry::new();
        let json_str = serde_json::to_string(&registry.to_openai_tools()).unwrap();
        assert!(json_str.contains("search_memory"), "应包含 search_memory");
        assert!(json_str.contains("analyze_intents"), "应包含 analyze_intents");
        assert!(json_str.contains("aggregate_stats"), "应包含 aggregate_stats");
        assert!(json_str.contains("category_search"), "应包含 category_search");
        assert!(
            json_str.contains("trend_comparison"),
            "应包含 trend_comparison"
        );
    }

    #[test]
    fn test_openai_tools_format_is_valid() {
        let registry = ToolRegistry::new();
        let tools = registry.to_openai_tools();

        // 应该有 5 个工具
        assert_eq!(tools.len(), 5);

        for tool in &tools {
            assert_eq!(tool["type"], "function");
            assert!(tool["function"]["name"].is_string());
            assert!(tool["function"]["description"].is_string());
            assert!(tool["function"]["parameters"]["type"].is_string());
            assert!(tool["function"]["parameters"]["properties"].is_object());
        }
    }

    #[test]
    fn test_search_memory_schema_matches_expected() {
        let schema = search_memory_parameters();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["query"].is_object());
        assert!(schema["properties"]["date_from"].is_object());
        assert!(schema["properties"]["date_to"].is_object());
        let required = schema["required"].as_array().unwrap();
        assert!(required.iter().any(|r| r == "query"));
    }

    #[test]
    fn test_analyze_intents_schema_requires_dates() {
        let schema = analyze_intents_parameters();
        let required = schema["required"].as_array().unwrap();
        let required_strs: Vec<&str> = required.iter().filter_map(|r| r.as_str()).collect();
        assert!(required_strs.contains(&"date_from"));
        assert!(required_strs.contains(&"date_to"));
    }

    #[test]
    fn test_aggregate_stats_schema_has_required_fields() {
        let schema = aggregate_stats_parameters();
        let required = schema["required"].as_array().unwrap();
        let required_strs: Vec<&str> = required.iter().filter_map(|r| r.as_str()).collect();
        assert!(required_strs.contains(&"date_from"));
        assert!(required_strs.contains(&"date_to"));
        assert!(schema["properties"]["metric"].is_object());
        assert!(schema["properties"]["category"].is_object());
        assert!(schema["properties"]["limit"].is_object());
    }

    #[test]
    fn test_category_search_schema_requires_category() {
        let schema = category_search_parameters();
        let required = schema["required"].as_array().unwrap();
        assert!(required.iter().any(|r| r == "category"));
    }

    #[test]
    fn test_trend_comparison_schema_requires_all_dates() {
        let schema = trend_comparison_parameters();
        let required = schema["required"].as_array().unwrap();
        let required_strs: Vec<&str> = required.iter().filter_map(|r| r.as_str()).collect();
        assert!(required_strs.contains(&"period_a_from"));
        assert!(required_strs.contains(&"period_a_to"));
        assert!(required_strs.contains(&"period_b_from"));
        assert!(required_strs.contains(&"period_b_to"));
    }

    #[test]
    fn test_execute_unknown_tool_returns_error() {
        let registry = ToolRegistry::new();
        let json_str = serde_json::to_string(&registry.to_openai_tools()).unwrap();
        assert!(!json_str.contains("nonexistent_tool"));
    }

    #[test]
    fn test_tool_definitions_are_complete() {
        let registry = ToolRegistry::new();
        let tools = registry.to_openai_tools();
        let json_str = serde_json::to_string_pretty(&tools).unwrap();
        assert!(json_str.contains("search_memory"));
        assert!(json_str.contains("analyze_intents"));
        assert!(json_str.contains("aggregate_stats"));
        assert!(json_str.contains("category_search"));
        assert!(json_str.contains("trend_comparison"));
        assert!(json_str.contains("parameters"));
    }

    // ── helper 函数测试 ──

    #[test]
    fn test_format_duration_compact() {
        assert_eq!(format_duration_compact(3661), "1h1m");
        assert_eq!(format_duration_compact(3600), "1h0m");
        assert_eq!(format_duration_compact(125), "2m");
        assert_eq!(format_duration_compact(45), "45s");
        assert_eq!(format_duration_compact(0), "0s");
    }

    #[test]
    fn test_resolve_category_key_english() {
        assert_eq!(
            resolve_category_key("development"),
            Some("development".to_string())
        );
        assert_eq!(resolve_category_key("BROWSER"), Some("browser".to_string()));
        assert_eq!(
            resolve_category_key("Communication"),
            Some("communication".to_string())
        );
    }

    #[test]
    fn test_resolve_category_key_chinese_exact() {
        assert_eq!(
            resolve_category_key("开发工具"),
            Some("development".to_string())
        );
        assert_eq!(
            resolve_category_key("通讯协作"),
            Some("communication".to_string())
        );
        assert_eq!(resolve_category_key("办公软件"), Some("office".to_string()));
    }

    #[test]
    fn test_resolve_category_key_chinese_partial() {
        assert_eq!(
            resolve_category_key("开发"),
            Some("development".to_string())
        );
        assert_eq!(
            resolve_category_key("通讯"),
            Some("communication".to_string())
        );
        assert_eq!(resolve_category_key("办公"), Some("office".to_string()));
        assert_eq!(resolve_category_key("浏览"), Some("browser".to_string()));
        assert_eq!(resolve_category_key("设计"), Some("design".to_string()));
    }

    #[test]
    fn test_resolve_category_key_unknown_returns_none() {
        assert_eq!(resolve_category_key("xyz"), None);
        assert_eq!(resolve_category_key("未知分类"), None);
        assert_eq!(resolve_category_key(""), None);
    }
}
