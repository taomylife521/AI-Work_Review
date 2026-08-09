# Overview Typography Consistency Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 统一概览页“常驻网站”与“应用使用”的文字行高、颜色和字体选择。

**Architecture:** 通过语义类和共享 CSS 选择器复用既有应用排行排版契约，不改变数据与布局结构。全局字体栈改为系统 UI 字体优先，并为紧凑模式标题补齐行高。

**Tech Stack:** Svelte 4、Tailwind CSS 3、Node.js Test Runner、Vite

## Global Constraints

- 不修改文案、数据结构、交互行为或响应式网格。
- 网站行与应用排行对应层级统一使用 `line-height: 1.35`。
- 正文字体不得再以 `SF Pro Display` 为第一选择。
- 保留用户工作区中的其他未提交改动，不自动提交。

---

### Task 1: 排版回归测试与最小实现

**Files:**
- Modify: `src/routes/OverviewEditorial.test.js`
- Modify: `src/UiVisualStyle.test.js`
- Modify: `src/routes/Overview.svelte`
- Modify: `src/app.css`
- Modify: `tailwind.config.js`

**Interfaces:**
- Consumes: `.app-usage-chart__name`、`.app-usage-chart__meta`、`.app-usage-chart__duration` 既有排版契约。
- Produces: `.overview-domain-name` 以及与应用排行共享的域名排版规则。

- [ ] **Step 1: 编写失败测试**

```js
assert.match(css, /\.app-usage-chart__name,\s*\.overview-domain-name\s*\{[^}]*line-height:\s*1\.35/);
assert.match(css, /\.app-shell\.ui-style-c\s+\.page-section-title\s*\{[^}]*line-height:\s*1\.35/);
assert.match(rootRule, /font-family:\s*-apple-system,\s*BlinkMacSystemFont,\s*"SF Pro Text"/);
```

- [ ] **Step 2: 运行测试并确认因缺少共享规则而失败**

Run: `node --test src/routes/OverviewEditorial.test.js src/UiVisualStyle.test.js`

Expected: FAIL，失败信息指向共享选择器、紧凑标题行高或字体顺序。

- [ ] **Step 3: 实现共享排版规则**

```css
.app-usage-chart__name,
.overview-domain-name {
  font-size: 0.8125rem;
  font-weight: 600;
  line-height: 1.35;
}
```

同时把 meta、来源和时长并入对应共享规则，将全局与 Tailwind 字体栈改为系统 UI 字体优先，并为 `ui-style-c` 标题增加 `line-height: 1.35`。

- [ ] **Step 4: 运行专项测试、全量测试和构建**

Run: `node --test src/routes/OverviewEditorial.test.js src/UiVisualStyle.test.js`

Run: `node --test src/**/*.test.js`

Run: `npm run build`

Expected: 本任务专项测试全部通过；若全量测试存在既有无关失败，记录具体测试；构建退出码为 `0`。

- [ ] **Step 5: 浏览器验证**

在 `http://127.0.0.1:5173/#/` 读取最终计算样式，确认标题行高和全局字体栈符合设计，并检查桌面视口没有文字重叠。
