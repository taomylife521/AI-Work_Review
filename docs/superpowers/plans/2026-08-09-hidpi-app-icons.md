# 高 DPI 应用图标实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use test-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将原生应用图标升级为单次高质量采样的 256px 输出，并移除导致非自然插值的前端渲染规则。

**Architecture:** Rust 后端继续集中负责 Alpha 裁边、等比缩放、透明画布和 PNG 编码，macOS 只提高进入该管线前的 ICNS 解码尺寸。前端不改变图标组件结构，只刷新缓存版本并恢复浏览器默认的高质量缩放。

**Tech Stack:** Rust、Tauri 2、`image` 0.25、Svelte 4、Node.js test runner、浏览器 Playwright API。

## Global Constraints

- 输出画布固定为 `256×256`，安全区最长边固定为 `208px`。
- macOS ICNS 解码上限固定为 `512px`。
- macOS、Windows、前端缓存版本分别为 `v4`、`v7`、`v4`。
- 前端图标持久化内容不得超过 `1_500_000` 字符。
- 不新增依赖，不修改 `get_app_icon` 接口，不改变时间线 CSS 容器尺寸。
- 单元测试命令单次最长运行 60 秒。
- 当前混合工作区不创建提交，不触碰语义记忆相关修改。

---

### Task 1: 高清原生图标规范化

**Files:**
- Modify: `src-tauri/src/commands/system.rs`

**Interfaces:**
- Consumes: macOS ICNS PNG、Windows HICON RGBA。
- Produces: `normalize_app_icon_rgba(image::RgbaImage) -> Option<Vec<u8>>` 返回 256px PNG 字节。

- [x] **Step 1: 写失败测试**

将现有规范化断言改为 256/208，并补充 macOS 解码尺寸及缓存版本断言：

```rust
assert_eq!(output.dimensions(), (256, 256));
assert_eq!(test_alpha_bounds(&output), Some((24, 76, 231, 179)));
assert_eq!(MACOS_ICON_SOURCE_SIZE, 512);
assert_eq!(MACOS_ICON_CACHE_VERSION, "v4");
assert_eq!(WINDOWS_ICON_CACHE_VERSION, "v7");
```

- [x] **Step 2: 运行测试确认红灯**

Run: `CC=/usr/bin/clang cargo test --manifest-path src-tauri/Cargo.toml commands::system::tests::高分辨率png图标应按alpha边界等比放入统一安全画布 -- --exact`

Expected: FAIL，实际输出仍为 `(128, 128)`。

Run: `cargo test --manifest-path src-tauri/Cargo.toml commands::system::tests::应升级原生图标磁盘缓存版本 -- --exact`

Expected: FAIL，缓存版本仍为 `v3/v6`。

- [x] **Step 3: 实现最小修复**

```rust
const APP_ICON_CANVAS_SIZE: u32 = 256;
const APP_ICON_SAFE_SIZE: u32 = 208;
const MACOS_ICON_SOURCE_SIZE: u32 = 512;
const MACOS_ICON_CACHE_VERSION: &str = "v4";
const WINDOWS_ICON_CACHE_VERSION: &str = "v7";
```

将 macOS `sips` 参数中的 `128` 替换为 `MACOS_ICON_SOURCE_SIZE.to_string()`，其余 Alpha 裁边和 Lanczos3 缩放流程保持不变。

- [x] **Step 4: 运行 Rust 图标测试确认绿灯**

Run: `CC=/usr/bin/clang cargo test --manifest-path src-tauri/Cargo.toml commands::system::tests::高分辨率png图标 -- --nocapture`

Expected: 规范化与透明输入测试全部 PASS。

Run: `CC=/usr/bin/clang cargo test --manifest-path src-tauri/Cargo.toml commands::system::tests::应升级原生图标磁盘缓存版本 -- --exact`

Expected: PASS。

### Task 2: 前端高质量缩放与缓存失效

**Files:**
- Modify: `src/app.css`
- Modify: `src/lib/stores/iconCache.js`
- Create: `src/lib/stores/iconCache.test.js`
- Modify: `src/lib/utils/appDisplay.test.js`

**Interfaces:**
- Consumes: 256px Base64 PNG。
- Produces: `work-review-app-icon-cache-v4` 缓存和默认浏览器插值。

- [x] **Step 1: 写失败测试**

```js
test('应用图标应使用浏览器默认高质量插值', async () => {
  const source = await readFile(new URL('../../app.css', import.meta.url), 'utf8');
  const rule = source.match(/\.app-icon\s*\{[^}]*\}/)?.[0] || '';
  assert.match(rule, /image-rendering:\s*auto/);
  assert.doesNotMatch(rule, /-webkit-optimize-contrast/);
});
```

并将缓存断言更新为：

```js
assert.match(source, /work-review-app-icon-cache-v4/);
assert.doesNotMatch(source, /work-review-app-icon-cache-v3/);
```

- [x] **Step 2: 运行测试确认红灯**

Run: `node --test src/lib/utils/appDisplay.test.js`

Expected: FAIL，样式仍含 `-webkit-optimize-contrast`，缓存仍为 v3。

- [x] **Step 3: 实现最小修复**

```css
.app-icon {
  image-rendering: auto;
  object-fit: contain;
}
```

将 `STORAGE_KEY` 更新为 `work-review-app-icon-cache-v4`。

按最近使用顺序筛选最多 36 项，并在序列化条目累计达到 `1_500_000` 字符前停止持久化；使用三个 70 万字符的模拟图标验证最终 JSON 不超过预算。

- [x] **Step 4: 运行前端专项测试确认绿灯**

Run: `node --test src/lib/stores/iconCache.test.js src/lib/utils/appDisplay.test.js src/routes/timeline/TimelineLayout.test.js`

Expected: 全部 PASS。

### Task 3: 回归与视觉验证

**Files:**
- Verify only: `src-tauri/src/commands/system.rs`
- Verify only: `src/app.css`
- Verify only: `src/lib/stores/iconCache.js`

**Interfaces:**
- Consumes: Task 1、Task 2 的最终实现。
- Produces: 自动化和高 DPI 视觉证据。

- [x] **Step 1: 运行完整相关测试**

Run: `node --test`

Expected: 除工作区既有、与本任务无关的语义记忆旧源码正则外，其余全部 PASS。

Run: `CC=/usr/bin/clang cargo test --manifest-path src-tauri/Cargo.toml commands::system::tests -- --nocapture`

Expected: 系统命令测试全部 PASS。

- [x] **Step 2: 运行构建和差异检查**

Run: `npm run build`

Expected: Vite 生产构建退出码 0。

Run: `git diff --check -- src-tauri/src/commands/system.rs src/app.css src/lib/stores/iconCache.js src/lib/utils/appDisplay.test.js`

Expected: 退出码 0，无输出。

- [ ] **Step 3: 执行 DPR 2 与 DPR 3 浏览器检查**

在 `http://127.0.0.1:5173/` 的时间线页分别设置 DPR 2、DPR 3，检查普通图标为 44 CSS px、详情图标为 51.2 CSS px；截图与 canvas 像素检查均须非空，图标不得溢出、拉伸或改变布局。

实际结果：浏览器预览确认当前环境 DPR 为 2；256px 输出覆盖详情图标 DPR 3 所需的 154px。浏览器安全策略拒绝加载独立 `data:` 对照页，因此未生成 DPR 3 浏览器截图，保留此项未勾选并在交付说明中披露。
