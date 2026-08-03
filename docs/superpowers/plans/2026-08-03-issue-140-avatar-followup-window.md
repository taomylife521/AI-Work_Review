# Issue #140 Avatar Follow-up Window Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复隐藏桌宠本体后“接上次继续”通知仍被压缩到紧凑高度的问题，使展开通知恢复完整原生窗口高度并补齐回归测试。

**Architecture:** 保持前端通知卡片和窗口展开调用链不变，只调整 Rust 侧 `avatar_window_size` 的状态优先级。`expanded=true` 时始终使用展开高度；`body_hidden` 仅影响非展开状态，从而恢复历史版本的动态窗口尺寸契约。

**Tech Stack:** Rust 2021、Tauri 2、Rust 内置测试框架、Node.js 源码契约测试、Vite/Svelte 4。

## Global Constraints

- 不新增依赖。
- 不修改前端通知卡片结构、视觉样式或按钮排列。
- 不改变隐藏桌宠在无跟进通知状态下的紧凑窗口高度。
- 不修改跟进通知触发条件、业务动作、版本号或发布说明。
- 所有代码注释使用简体中文。
- 严格执行测试驱动：生产代码修改前必须先运行并确认新增测试按预期失败。
- 单次后台测试命令最大运行时间为 60 秒。

---

## File Structure

- Modify: `src-tauri/src/avatar_engine.rs`
  - 增加 `expanded=true && body_hidden=true` 回归测试。
  - 调整 `avatar_window_size` 的高度状态优先级。
- Verify only: `src/lib/components/Avatar/avatarWindow.test.js`
  - 确认前端仍会随跟进通知出现和消失调用原生窗口展开命令。
- Verify only: `src/lib/components/Avatar/avatarOutline.test.js`
  - 确认跟进卡片原有内容滚动和操作结构契约未被破坏。

---

### Task 1: 用回归测试修复隐藏本体时的展开窗口高度

**Files:**
- Modify: `src-tauri/src/avatar_engine.rs:759-779`
- Test: `src-tauri/src/avatar_engine.rs:1008-1030`

**Interfaces:**
- Consumes: `fn avatar_window_size(scale: f64, expanded: bool, body_hidden: bool) -> (f64, f64)`
- Produces: 保持函数签名不变；当 `expanded=true` 时返回展开高度，不受 `body_hidden` 覆盖。

- [ ] **Step 1: 写入能够复现 Issue #140 的失败测试**

在现有“桌宠窗口在展开模式下应比紧凑模式更宽更高”测试之后增加：

```rust
#[test]
fn 隐藏桌宠本体时展开通知仍应使用完整展开尺寸() {
    let (hidden_compact_w, hidden_compact_h) = avatar_window_size(0.9, false, true);
    let (hidden_expanded_w, hidden_expanded_h) = avatar_window_size(0.9, true, true);
    let visible_expanded = avatar_window_size(0.9, true, false);

    assert_eq!((hidden_compact_w, hidden_compact_h), (248.4, 99.0));
    assert_eq!((hidden_expanded_w, hidden_expanded_h), (342.0, 396.0));
    assert_eq!((hidden_expanded_w, hidden_expanded_h), visible_expanded);
}
```

该测试同时锁定四项行为：隐藏本体的非展开状态继续紧凑、隐藏本体的展开状态恢复完整宽高、展开状态与是否显示本体无关、尺寸继续应用默认缩放。

- [ ] **Step 2: 运行新增测试并确认按预期失败**

Run：

```bash
timeout 60 cargo test -p work-review --bin work-review \
  avatar_engine::tests::隐藏桌宠本体时展开通知仍应使用完整展开尺寸 \
  -- --exact --nocapture
```

如果系统没有 GNU `timeout`，使用：

```bash
python3 -c 'import subprocess; subprocess.run(["cargo", "test", "-p", "work-review", "--bin", "work-review", "avatar_engine::tests::隐藏桌宠本体时展开通知仍应使用完整展开尺寸", "--", "--exact", "--nocapture"], timeout=60, check=True)'
```

Expected：测试断言失败，实际展开高度为 `99.0`，期望高度为 `396.0`。如果测试通过或因名称、编译错误失败，先修正测试，直到它因现有尺寸逻辑而失败。

- [ ] **Step 3: 实现最小尺寸优先级修复**

将 `avatar_window_size` 的高度选择改为：

```rust
fn avatar_window_size(scale: f64, expanded: bool, body_hidden: bool) -> (f64, f64) {
    let normalized_scale = normalize_avatar_scale(scale);
    let base_width = if expanded {
        AVATAR_WINDOW_EXPANDED_BASE_WIDTH
    } else {
        AVATAR_WINDOW_BASE_WIDTH
    };
    // 跟进通知展开时必须优先保留完整窗口高度；隐藏本体只压缩非展开状态。
    let base_height = if expanded {
        AVATAR_WINDOW_EXPANDED_BASE_HEIGHT
    } else if body_hidden {
        AVATAR_WINDOW_BODY_HIDDEN_BASE_HEIGHT
    } else {
        AVATAR_WINDOW_BASE_HEIGHT
    };
    (
        ((base_width * normalized_scale) * 10.0).round() / 10.0,
        ((base_height * normalized_scale) * 10.0).round() / 10.0,
    )
}
```

不得修改函数参数、缩放归一化、四舍五入方式或调用方。

- [ ] **Step 4: 运行新增测试并确认通过**

Run：

```bash
python3 -c 'import subprocess; subprocess.run(["cargo", "test", "-p", "work-review", "--bin", "work-review", "avatar_engine::tests::隐藏桌宠本体时展开通知仍应使用完整展开尺寸", "--", "--exact", "--nocapture"], timeout=60, check=True)'
```

Expected：`1 passed; 0 failed`。

- [ ] **Step 5: 运行 avatar_engine 尺寸与位置相关回归测试**

Run：

```bash
python3 -c 'import subprocess; subprocess.run(["cargo", "test", "-p", "work-review", "--bin", "work-review", "avatar_engine::tests::桌宠", "--", "--nocapture"], timeout=60, check=True)'
```

Expected：匹配到的桌宠窗口尺寸、缩放、展开和位置钳制测试全部通过。

- [ ] **Step 6: 格式化并检查差异**

Run：

```bash
cargo fmt --all -- --check
git diff --check
git diff -- src-tauri/src/avatar_engine.rs
```

Expected：格式化检查退出码为 `0`，无空白错误，差异只包含新增测试和最小尺寸逻辑修改。

- [ ] **Step 7: 提交代码修复**

```bash
git add src-tauri/src/avatar_engine.rs
git commit -m "fix(avatar): preserve expanded height when body is hidden"
```

---

### Task 2: 执行项目验证并完成独立代码审查

**Files:**
- Verify: `src-tauri/src/avatar_engine.rs`
- Verify: `src/lib/components/Avatar/avatarWindow.test.js`
- Verify: `src/lib/components/Avatar/avatarOutline.test.js`

**Interfaces:**
- Consumes: Task 1 修复后的 `avatar_window_size` 行为。
- Produces: 可追溯的测试、构建和独立审查结果；不新增生产接口。

- [ ] **Step 1: 运行桌宠前端契约测试**

Run：

```bash
node --test \
  src/lib/components/Avatar/avatarWindow.test.js \
  src/lib/components/Avatar/avatarOutline.test.js
```

Expected：两个测试文件全部通过，确认前端展开调用和卡片结构未受影响。

- [ ] **Step 2: 运行 work-review Rust 测试目标**

Run：

```bash
python3 -c 'import subprocess; subprocess.run(["cargo", "test", "-p", "work-review", "--bin", "work-review"], timeout=60, check=True)'
```

Expected：命令在 60 秒内完成且所有测试通过。若超时，记录为“超时”，随后保留已完成的定向测试结果，不能声称全量 Rust 测试通过。

- [ ] **Step 3: 运行 Rust 格式化和编译检查**

Run：

```bash
cargo fmt --all -- --check
python3 -c 'import subprocess; subprocess.run(["cargo", "check", "-p", "work-review"], timeout=60, check=True)'
```

Expected：格式化和编译检查退出码均为 `0`。

- [ ] **Step 4: 运行前端生产构建**

Run：

```bash
npm run build
```

Expected：Vite 构建成功，退出码为 `0`；不新增构建错误。

- [ ] **Step 5: 检查仓库状态和最终差异**

Run：

```bash
git status --short
git diff HEAD^ --check
git show --stat --oneline HEAD
git diff HEAD^ HEAD -- src-tauri/src/avatar_engine.rs
```

Expected：工作区干净，修复提交只修改 `src-tauri/src/avatar_engine.rs`，不存在未预期文件。

- [ ] **Step 6: 请求独立代码审查**

向独立审查代理提供：

```text
需求：Issue #140。隐藏桌宠本体时，跟进通知展开必须使用完整展开高度；无通知时仍使用紧凑高度。
范围：只修改 avatar_engine.rs 的尺寸状态优先级和 Rust 回归测试，不调整前端。
审查重点：状态矩阵是否正确、缩放/四舍五入是否保持、展开后的窗口位置钳制是否仍生效、测试是否真正捕获旧实现。
```

审查代理不得修改文件，只返回按严重级别排列的问题。Critical 和 Important 问题必须修复并重新执行受影响测试；若无问题，记录审查通过。

- [ ] **Step 7: 最终验证**

在处理审查反馈后重新运行：

```bash
python3 -c 'import subprocess; subprocess.run(["cargo", "test", "-p", "work-review", "--bin", "work-review", "avatar_engine::tests::隐藏桌宠本体时展开通知仍应使用完整展开尺寸", "--", "--exact", "--nocapture"], timeout=60, check=True)'
node --test \
  src/lib/components/Avatar/avatarWindow.test.js \
  src/lib/components/Avatar/avatarOutline.test.js
cargo fmt --all -- --check
git status --short
```

Expected：定向 Rust 测试通过、桌宠前端契约测试通过、格式化检查通过、工作区干净。
