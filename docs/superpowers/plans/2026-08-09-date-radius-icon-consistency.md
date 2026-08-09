# 日期导航、圆角与应用图标一致性实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use test-driven-development to implement every task with a verified RED-GREEN cycle. Independent tasks may be dispatched in parallel when their file sets do not overlap.

**Goal:** 为日报增加连续上一天导航，统一全应用圆角体系，并修复 Cent Browser 图标获取与缩放。

**Architecture:** 日期运算提取为纯函数，日报请求使用日期快照和请求编号隔离。圆角由五档语义令牌统一管理，局部任意值迁移到令牌。图标链路按“采集名称归一化 -> 原生提取 -> Alpha 画布规范化 -> 前端显示/兜底”分层修复。

**Tech Stack:** Svelte 4、Node.js test runner、Rust、Tauri 2、`image` 0.25、Playwright 1.61。

## 全局约束

- 代码注释与新增文档使用简体中文。
- 不新增依赖，不修改现有数据库结构。
- 保留圆点、开关、头像、进度条和短状态徽标的全圆角。
- 普通按钮、输入框、卡片、菜单和主框架只使用 4/6/8/12px。
- 单元测试命令单次最长运行 60 秒。
- 不触碰当前工作区中既有的语义记忆相关修改。

---

### 任务一：日报上一天导航与请求隔离

**Files:**
- Create: `src/routes/report/reportDateNavigation.js`
- Create: `src/routes/report/reportDateNavigation.test.js`
- Create: `src/routes/report/ReportDateNavigation.test.js`
- Modify: `src/routes/report/Report.svelte`
- Modify: `src/lib/i18n/locales/zh-CN.js`
- Modify: `src/lib/i18n/locales/zh-TW.js`
- Modify: `src/lib/i18n/locales/en.js`
- Modify: `src/lib/i18n/locales/ar.js`

**Interfaces:**
- Produces: `shiftLocalIsoDate(dateValue: string, offsetDays: number): string`
- Consumes: `selectedDate`、`reportRequestId`、日报缓存和 Tauri `invoke`。

- [ ] 写失败测试，断言 `2026-08-04 -> 2026-08-03`、跨月、跨年和闰年结果。
- [ ] 运行 `node --test src/routes/report/reportDateNavigation.test.js src/routes/report/ReportDateNavigation.test.js`，确认因函数或按钮缺失失败。
- [ ] 实现纯日期函数，在日期组最左侧增加本地化“上一天”按钮：

```js
export function shiftLocalIsoDate(dateValue, offsetDays) {
  const next = new Date(`${dateValue}T12:00:00`);
  next.setDate(next.getDate() + offsetDays);
  return `${next.getFullYear()}-${String(next.getMonth() + 1).padStart(2, '0')}-${String(next.getDate()).padStart(2, '0')}`;
}
```

- [ ] 重构 `loadReport()` 捕获 `targetDate`、`targetLocale`、`targetCacheKey`，在每个 `await` 后检查 `requestId`，仅最新请求清理加载状态。
- [ ] 加入四语言 `report.previousDay`，运行任务测试确认通过。

### 任务二：统一全应用圆角

**Files:**
- Create: `src/RadiusConsistency.test.js`
- Modify: `src/app.css`
- Modify: `src/routes/**/*.svelte`
- Modify: `src/lib/components/**/*.svelte`
- Modify: existing visual-style tests whose assertions encode obsolete large radii

**Interfaces:**
- Produces: `--radius-xs`、`--radius-sm`、`--radius-md`、`--radius-lg`、`--radius-full`。
- Consumes: existing Tailwind utilities and A/B/C theme selectors.

- [ ] 写失败静态测试，断言五档令牌存在，禁止结构容器使用 `>12px` 任意圆角，并禁止日报日期组使用 `999px`。
- [ ] 运行 `node --test src/RadiusConsistency.test.js`，确认旧的 20/26/32px 与日期胶囊触发失败。
- [ ] 将共享按钮、输入、卡片、菜单、设置导航和应用框架迁移到 4/6/8/12px；A/B/C 删除圆角尺度覆盖。
- [ ] 将设置页内部嵌套卡改为分隔行或无背景分组，保留选中态、危险区和焦点环。
- [ ] 审查 `rounded-full`/`999px`，仅保留圆点、开关、头像、加载圈、进度条和短徽标。
- [ ] 运行圆角测试及 `SurfaceConsistency`、`UiVisualStyle`、设置/日报/时间线布局测试确认通过。

### 任务三：修复 Cent Browser 图标链路

**Files:**
- Modify: `src/lib/utils/appDisplay.js`
- Modify: `src/lib/utils/appDisplay.test.js`
- Modify: `src/lib/stores/iconCache.js`
- Modify: `crates/core/src/categorize.rs`
- Modify: `src-tauri/src/monitor.rs`
- Modify: `src-tauri/src/commands/system.rs`

**Interfaces:**
- Produces: `Cent Browser` 统一名称和规范化后的 128px PNG Base64。
- Consumes: `get_app_icon`、活动的 `app_name/executable_path`、现有 `image` crate。

- [ ] 写失败 JavaScript 测试，断言 `centbrowser + 新标签页` 不强制字母兜底。
- [ ] 写失败 Rust 测试，断言三个 Cent 别名映射一致，macOS 名称评分大于零，Alpha 小图被居中放大到统一安全区。
- [ ] 分别运行 Node 与精确 Rust 测试，确认预期失败。
- [ ] 移除“小写进程名 + 不同标题”这一过宽兜底条件，只保留明确安装器规则。
- [ ] 补齐采集、分类和 macOS 别名；实现共享 Alpha 裁切/128px 安全画布编码，升级 macOS、Windows 和前端缓存版本。
- [ ] 运行精确测试确认通过。

### 任务四：统一时间线图标呈现与视觉回归

**Files:**
- Modify: `src/routes/timeline/Timeline.svelte`
- Modify: `src/routes/timeline/TimelineLayout.test.js`
- Modify: `src/routes/report/Report.svelte`
- Modify: `src/app.css`

**Interfaces:**
- Consumes: `resolveAppIconSrc()` 返回的原生或兜底 URL。
- Produces: 列表、详情和窄屏一致的图标安全区及加载失败兜底。

- [ ] 写失败布局测试，断言图片使用相对安全区、`object-fit: contain`、图片态无分类色双底板且有错误回退。
- [ ] 运行 `node --test src/routes/timeline/TimelineLayout.test.js`，确认旧固定 1.9rem 图片尺寸触发失败。
- [ ] 为图片态添加稳定类名和加载失败状态，列表/详情共用尺寸规则，移动端不再单独改变图标占比。
- [ ] 运行相关 Node 测试、完整 `node --test`、精确 Rust 测试与 `npm run build`。
- [ ] 启动本地开发服务，使用 Playwright 检查桌面与移动视口的日报、设置、时间线及明暗主题；修复发现的溢出或重叠后重新截图验证。
