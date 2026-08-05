# 排除 Work Review 自身浮动窗口实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 阻止桌宠和通知气泡被 macOS 浮动窗口监控误计为 Work Review 活动时长。

**Architecture:** 在 `monitor.rs` 中读取 CGWindow 的窗口所有者 PID，并通过独立纯函数与当前 Work Review 进程 PID 精确比较，然后在 `get_overlay_windows()` 枚举阶段提前排除。前台窗口记录链路保持不变。

**Tech Stack:** Rust、macOS Core Graphics、Cargo 单元测试

## Global Constraints

- 仅排除 Work Review 自身的浮动窗口，不改变正常前台计时。
- 不新增依赖。
- 测试命令最长运行 60 秒。
- 所有修改最终合并为一个提交。

---

### Task 1: 增加自身应用识别与浮动窗口过滤

**Files:**
- Modify: `src-tauri/src/monitor.rs`
- Test: `src-tauri/src/monitor.rs`

**Interfaces:**
- Consumes: CGWindow 返回的 `kCGWindowOwnerPID` 数值和当前进程 PID。
- Produces: `is_current_process_owner(owner_pid: Option<f64>, current_process_id: u32) -> bool`，供浮动窗口枚举过滤使用。

- [ ] **Step 1: 写入失败测试**

测试当前进程 PID 返回 `true`，并测试其他 PID 或缺少 PID 时返回 `false`。

- [ ] **Step 2: 运行测试确认失败**

Run: `CC=/usr/bin/clang CXX=/usr/bin/clang++ cargo test --manifest-path src-tauri/Cargo.toml current_process_owner -- --nocapture`

Expected: 编译失败，提示 `is_current_process_owner` 尚未定义。

- [ ] **Step 3: 写入最小实现**

读取窗口所有者 PID，在 `get_overlay_windows()` 中与当前进程 PID 匹配后提前 `continue`。

- [ ] **Step 4: 运行定向测试确认通过**

Run: `CC=/usr/bin/clang CXX=/usr/bin/clang++ cargo test --manifest-path src-tauri/Cargo.toml current_process_owner -- --nocapture`

Expected: 新增测试全部通过。

- [ ] **Step 5: 运行回归验证**

Run: `CC=/usr/bin/clang CXX=/usr/bin/clang++ cargo test --manifest-path src-tauri/Cargo.toml`

Run: `npm run build`

Expected: Rust 测试和前端构建均成功。

- [ ] **Step 6: 审查并提交**

检查 `git diff --check`、改动范围和测试输出，将代码、测试、设计与计划合并为一个提交。
