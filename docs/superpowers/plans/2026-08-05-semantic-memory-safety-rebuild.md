# 语义记忆隐私一致性与索引重建实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让活动删除、隐私变化和 Embedding 模型变化立即使旧语义索引失效，并提供安全的全量重建与 FTS 降级。

**Architecture:** 使用单例索引状态、Embedding/隐私指纹和全量失效策略。向量检索仅在状态为 ready 且指纹匹配时启用，否则降级 FTS；索引与召回均执行完整隐私过滤。

**Tech Stack:** Rust、SQLite、Tauri、Svelte、Vitest、Cargo test

## Global Constraints

- 不自动开启语义记忆。
- 不引入新依赖、Generation、事件溯源或 ANN。
- 删除和隐私收紧必须 fail-closed。
- 测试命令最长 60 秒。
- 所有修改最终合并为一个提交。

---

### Task 1: 增加索引状态和数据库失效事务

**Files:**
- Modify: `crates/core/src/database.rs`
- Test: `crates/core/src/database.rs`

**Interfaces:**
- Produces: `SemanticMemoryIndexState`、读取/更新状态、清空并标记待重建的方法。
- Produces: 所有活动删除方法在同一事务中失效语义索引和摘要缓存。

- [ ] 写入状态 CRUD、删除失效和回滚测试。
- [ ] 运行定向测试确认失败。
- [ ] 实现表结构、状态方法和删除事务。
- [ ] 运行定向测试确认通过。

### Task 2: 增加指纹、完整隐私过滤和安全重建

**Files:**
- Modify: `src-tauri/src/commands/semantic_memory.rs`
- Modify: `src-tauri/src/commands/shared.rs`
- Modify: `src-tauri/src/commands/stats.rs`
- Test: `src-tauri/src/commands/semantic_memory.rs`

**Interfaces:**
- Consumes: Task 1 的状态与失效方法。
- Produces: 指纹计算、重建开始/失败/完成状态、查询 fail-closed 和 FTS 降级。

- [ ] 写入指纹稳定性、隐私过滤、状态不匹配禁用向量的失败测试。
- [ ] 实现指纹和完整隐私判定。
- [ ] 配置隐私/Embedding 变化时失效索引。
- [ ] 修正旧记录清理失败仍返回成功的问题。
- [ ] 运行定向测试。

### Task 3: 更新索引状态 UI

**Files:**
- Modify: `src/routes/settings/components/SettingsAI.svelte`
- Modify: `src/SemanticMemoryIntegration.test.js`
- Modify: `src/lib/i18n/locales/zh-CN.js`
- Modify: `src/lib/i18n/locales/zh-TW.js`
- Modify: `src/lib/i18n/locales/en.js`
- Modify: `src/lib/i18n/locales/ar.js`

- [ ] 写入需要重建、失败和进度展示测试。
- [ ] 实现状态展示与重建按钮语义。
- [ ] 运行前端定向测试。

### Task 4: 语义记忆回归验证

- [ ] 运行语义记忆 Rust 定向测试。
- [ ] 运行 core 数据库定向测试。
- [ ] 运行前端语义记忆测试。
- [ ] 运行 `git diff --check`。
