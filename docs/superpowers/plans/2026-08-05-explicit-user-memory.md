# 用户显式长期记忆实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让用户通过明确的“记住、修改、忘掉”指令和确认流程管理可控的结构化长期记忆。

**Architecture:** 独立 SQLite 表保存用户拥有的结构化记忆；普通对话只读召回，明确记忆意图才注册写工具；写操作复用现有 Action Confirmation，设置页提供手动 CRUD。

**Tech Stack:** Rust、SQLite、Tauri、Svelte、Vitest、Cargo test

## Global Constraints

- 禁止普通聊天静默写入。
- 新增、修改、删除必须确认；设置页手动 CRUD 除外。
- 忘记使用硬删除。
- 第一版不使用 Embedding，不做自动学习和跨设备同步。
- 测试命令最长 60 秒。
- 所有修改最终合并为一个提交。

---

### Task 1: 增加长期记忆数据模型和 CRUD

**Files:**
- Modify: `crates/core/src/database.rs`
- Test: `crates/core/src/database.rs`

**Interfaces:**
- Produces: `AssistantUserMemory`、创建、列表、搜索、更新、删除、清空和召回方法。
- Enforces: 唯一 key、revision 乐观锁、过期过滤、硬删除和敏感内容拒绝。

- [ ] 写入 CRUD、冲突、revision、过期、召回策略和硬删除失败测试。
- [ ] 实现最小数据库接口。
- [ ] 运行定向测试。

### Task 2: 增加 Tauri 命令和 Agent 工具

**Files:**
- Create: `src-tauri/src/commands/user_memory.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/src/agent/tools.rs`
- Modify: `src-tauri/src/commands/ask.rs`
- Test: `src-tauri/src/agent/tools.rs`
- Test: `src-tauri/src/commands/ask.rs`

**Interfaces:**
- Consumes: Task 1 数据库接口。
- Produces: 设置页 CRUD 命令、四个 Agent 工具、显式意图门控和长期记忆 Prompt 召回。

- [ ] 写入工具确认、意图门控、敏感内容拒绝和召回预算失败测试。
- [ ] 实现命令和工具。
- [ ] 将写工具接入现有确认 ActionBridge。
- [ ] 运行定向测试。

### Task 3: 增加设置页长期记忆管理

**Files:**
- Create: `src/routes/settings/components/AssistantMemoryManager.svelte`
- Modify: `src/routes/settings/components/SettingsAI.svelte`
- Create: `src/routes/settings/components/AssistantMemoryManager.test.js`
- Modify: `src/lib/i18n/locales/zh-CN.js`
- Modify: `src/lib/i18n/locales/zh-TW.js`
- Modify: `src/lib/i18n/locales/en.js`
- Modify: `src/lib/i18n/locales/ar.js`

- [ ] 写入列表、手动新增、编辑、删除、清空和关闭不删除数据测试。
- [ ] 实现独立管理组件并接入 AI 设置。
- [ ] 运行前端定向测试。

### Task 4: 长期记忆回归验证

- [ ] 运行数据库定向测试。
- [ ] 运行 Agent 工具和 Prompt 定向测试。
- [ ] 运行前端管理组件测试。
- [ ] 运行完整前端测试、Rust 测试和构建。
- [ ] 检查长期记忆内容未进入工具摘要、日志和 localStorage 步骤。
