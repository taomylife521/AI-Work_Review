# 前端 TypeScript 渐进迁移设计

## 背景

Work Review 从初始版本起就是 Tauri + Rust + Svelte 的混合架构。当前生产代码仍以 Rust 为主，前端 JavaScript 的主要风险不是体量，而是 Tauri IPC、流式助手、日报结构、时间线增量合并和配置状态缺少静态类型约束。

本设计不追求“全项目单一语言”，而是把前端从松散 JavaScript 收敛为 Svelte + TypeScript，并保持 Rust 是领域规则和持久化数据的唯一事实来源。

## 目标

1. 建立可在 CI 和本地执行的 TypeScript/Svelte 类型检查，类型错误必须阻止前端验证通过。
2. 建立统一的类型化 Tauri IPC 入口，逐步减少页面中直接调用字符串命令。
3. 优先迁移高风险生产 JavaScript，再逐页迁移 Svelte `<script>`。
4. 保持现有 UI、业务行为、Tauri command 名称和持久化格式不变。
5. 保持现有 Node `node:test` 测试语义，并支持测试直接导入 TypeScript 源码。

## 非目标

- 不把 Svelte 重写为 Rust/WASM 或原生 Rust UI。
- 不在本次迁移中重做视觉样式、路由、数据库或 Rust command。
- 不一次性迁移所有测试、本地化字典和构建脚本。
- 不用 `any`、全局 `@ts-ignore` 或关闭检查来伪造“迁移完成”。确实位于外部边界的未知数据先使用 `unknown`，经过类型守卫后再进入应用代码。
- 不在迁移过程中保留同名 `.js` 兼容副本；每个模块只有一个权威实现。

## 方案比较

### 方案 A：只加入 TypeScript 配置

改动最小，但现有生产模块继续是 JavaScript，不能约束 IPC 和业务状态。只能算工具链准备，不能解决当前问题。

### 方案 B：一次性迁移全部前端

最终形态整齐，但会同时改动约 2 万行 Svelte、约 4 千行前端逻辑以及大量源码契约测试。当前工作区还有大量未提交功能修改，批量迁移会显著增加回归和冲突风险。

### 方案 C：渐进迁移，严格检查（采用）

先建立检查链和类型化边界，再迁移高风险纯模块，随后逐页迁移 Svelte。未迁移 JavaScript可继续参与构建，但新建生产模块必须使用 TypeScript。每一批都能独立通过类型检查、测试和生产构建。

## 目标架构

```text
work-review-core / src-tauri (Rust)
  -> serde DTO / Tauri command
  -> src/lib/api/contracts.ts       共享前端 DTO
  -> src/lib/api/tauri.ts           唯一底层 invoke 封装
  -> src/lib/api/*.ts               按领域组织的 typed client
  -> stores / routes / components   Svelte + TypeScript
```

### Rust 边界

Rust 继续负责数据库、隐私过滤、分类、统计口径、日报结构、系统能力和 AI 编排。本设计不把领域规则复制到 TypeScript；发现已有重复时，只为现状建模并记录后续下沉任务，不在 TS 迁移中扩大双实现。

### TypeScript 边界

TypeScript 负责：

- Tauri command 参数、返回值和事件 payload 的前端契约；
- Svelte 页面状态、请求所有权、加载/错误状态和用户交互；
- 展示层纯函数，如本地化格式化、进度条宽度和视口位置；
- 对 `localStorage`、Tauri 返回值和流式事件等不可信边界进行运行时缩窄。

## 工具链设计

新增开发依赖：

- `typescript`：编译与静态类型系统；
- `svelte-check`：检查 `.svelte` 模板和 `<script lang="ts">`；
- `@tsconfig/svelte`：使用与 Svelte 4 兼容的基础配置；
- `@types/node`：为 Node 测试和构建配置提供类型；
- `tsx`：保持 Node 18+ 环境下的 `node:test` 测试能够直接导入 `.ts` 源码。

新增命令：

```json
{
  "check": "svelte-check --tsconfig ./tsconfig.json --threshold error",
  "test:frontend": "tsx --test",
  "verify:frontend": "npm run check && npm run test:frontend && npm run build"
}
```

`tsconfig.json` 使用严格模式、`noEmit`、`allowImportingTsExtensions` 和 `allowJs`。`allowJs` 只为渐进兼容，`checkJs` 保持关闭；所有 `.ts` 与 `<script lang="ts">` 必须通过严格检查。允许显式 `.ts` 导入扩展名，确保 Node 测试和 Vite 解析一致。另建 `src/vite-env.d.ts` 引用 `vite/client`；Node 构建配置不纳入浏览器源码的全局类型范围。

## 类型化 IPC

### 文件职责

- `src/lib/api/contracts.ts`：跨页面共享 DTO、分页结果、请求状态和结构化错误类型。
- `src/lib/api/commandMap.ts`：Tauri command 到 `{ args, result }` 的映射。
- `src/lib/api/tauri.ts`：唯一直接导入 `@tauri-apps/api/core` 的模块，提供 `invokeCommand<K>()`。
- `src/lib/api/overview.ts`、`timeline.ts`、`report.ts`、`assistant.ts`、`config.ts`：领域 client，隐藏命令字符串和参数命名。

核心接口：

```ts
export interface TauriCommandMap {
  get_overview_stats: {
    args: OverviewStatsRequest;
    result: DailyStats;
  };
}

export async function invokeCommand<K extends keyof TauriCommandMap>(
  command: K,
  args: TauriCommandMap[K]['args'],
): Promise<TauriCommandMap[K]['result']>;
```

第一批只覆盖正在迁移页面使用的命令，不为 103 个 command 一次性制造不可靠的占位类型。未建模命令继续使用现有调用，进入对应迁移批次时再加入映射。

错误不通过匹配中文消息进行分支。领域 client 将未知异常归一化为包含 `code`、`message` 和 `cause` 的 `FrontendCommandError`；没有结构化后端错误码的命令只保留原始消息，不猜测错误类别。

## 迁移顺序

### 第一批：工具链与叶子模块试点

先写会失败的工具链/源码约束测试，再加入 TypeScript 配置。第一个试点固定为只有单一职责和独立测试的 `src/lib/utils/dateValidation.js`，用它验证 `tsx`、`svelte-check`、Vite 和源码测试对 `.ts` 导入的完整链路。

试点通过后，按“一个模块及其直接测试”为单位迁移其余叶子模块：

- `src/lib/components/Avatar/bubbleMessage.js`
- `src/routes/ask/requestEventGate.js`
- `src/routes/report/reportPromptFeedback.js`
- `src/routes/report/reportDateNavigation.js`
- `src/lib/utils/errorDisplay.js`
- `src/lib/utils/popoverPosition.js`
- `src/lib/utils/browserUrl.js`
- `src/lib/utils/focusTrap.js`
- `src/routes/ask/starterPromptPresentation.js`
- `src/routes/ask/modelPresentation.js`
- `src/routes/overviewCategoryPresentation.js`
- `src/lib/utils/appDisplay.js`

### 第二批：高风险纯逻辑模块

在工具链稳定后迁移带有数据协议、排序或状态归约的模块：

- `src/routes/ask/historyPayload.js`
- `src/routes/ask/streamEvent.js`
- `src/routes/report/reportSections.js`
- `src/routes/report/reportMeta.js`
- `src/routes/timeline/timelineData.js`
- `src/routes/overviewDomainPresentation.js`
- `src/routes/timeline/summaryPresentation.js`
- `src/lib/utils/appVisuals.js`

这些模块改为 `.ts`，补充明确输入/输出类型和边界守卫。现有测试继续验证行为，不因为迁移而改写业务期望。

### 第三批：共享状态与 IPC client

迁移 `stores`、更新器、图标缓存和共享 API client。优先替换概览、时间线、日报、助手和设置页的直接 `invoke` 调用。每个领域 client 完成后才迁移对应页面，不建立长期存在的半类型封装。

### 第四批：Svelte 页面

按以下顺序添加 `<script lang="ts">`：

1. `StatsCard.svelte`、`Toast.svelte`、`ConfirmDialog.svelte`、`CollapsibleSection.svelte`；
2. `LocalizedDatePicker.svelte` 与两个图表组件；
3. Avatar 和 Node Gateway 小组件；
4. Settings 各功能组件；
5. `About.svelte` 与 `Settings.svelte`；
6. `Ask.svelte` 与 `Report.svelte`；
7. `Overview.svelte`；
8. `Timeline.svelte` 与抽屉；
9. `App.svelte`。

每次只迁移一个领域，必须先通过该领域测试和全量前端验证。页面内临时状态使用局部接口或判别联合；不把所有状态集中为一个巨型全局类型文件。

### 第五批：本地化、测试与构建脚本

生产页面稳定后，再将本地化字典约束为共享 schema，并逐步迁移测试和构建脚本。测试文件可以暂时保留 JavaScript，只要通过 `tsx` 导入 TypeScript 生产模块并参加完整测试。

## 测试策略

遵循测试驱动迁移：

1. 先新增源码约束测试，断言 `tsconfig.json`、检查脚本和 typed IPC 入口存在，并确认在实现前失败。
2. 对每个迁移的纯函数先运行原有测试，确认基线；重命名后先观察导入失败，再修改测试导入并恢复通过。
3. 为 `invokeCommand` 编写真实类型夹具和运行时错误归一化测试。类型级错误使用 `tsc`/`svelte-check` 负例夹具或 `@ts-expect-error` 断言。
4. 每个 Svelte 页面迁移后运行该页面的行为/源码测试、`npm run check` 和生产构建。
5. 最终运行 `npm run verify:frontend`、Rust 中受 IPC DTO 影响的定向测试以及 `git diff --check`。

不把现有“读取源码并正则断言”的测试视为长期目标。迁移触及这些测试时，优先改为纯函数、组件行为或构建契约测试；与本次范围无关的测试不顺手重写。

## 兼容与回滚

- 每批迁移保持 Tauri command、序列化字段和路由行为不变，可以按批次独立回退。
- 不同时保留 `.js` 和 `.ts` 两份实现，避免解析顺序和测试导入产生歧义。
- 未迁移 Svelte 仍可导入已迁移 TypeScript 模块；`tsx` 负责测试环境解析，Vite 负责应用构建解析。
- 如果严格检查暴露既有类型矛盾，优先在边界增加类型守卫或修正真实数据契约，不通过放宽全局编译选项绕过。

## 验收标准

第一阶段完成需同时满足：

1. `npm run check`、`npm run test:frontend`、`npm run build` 均退出 0；
2. 首批纯逻辑模块已改为 `.ts`，不存在同名 `.js` 副本；
3. typed IPC 基础层有运行时测试和类型约束；
4. 新建生产前端逻辑使用 TypeScript；
5. 现有 UI、数据格式、命令名称和用户工作流无行为变化；
6. 工作区中用户原有未提交修改全部保留，不被还原或覆盖。

完整迁移完成需额外满足：

1. `src/` 下除明确豁免的本地化字典外，生产脚本均为 `.ts` 或 `<script lang="ts">`；
2. 页面不再直接导入 Tauri `invoke`，统一经过 `src/lib/api`；
3. command 参数、结果和事件 payload 不使用无约束 `any`；
4. 本地化字典通过共享键结构检查；
5. 前端验证命令成为 CI 的必跑门槛。

## 风险控制

- 当前工作区改动很多，不采用全量机械替换 Svelte 脚本；逐领域修改并在每批后检查差异。
- TypeScript 迁移不与视觉重构、Rust 领域重构或依赖大版本升级混在同一批。
- 对助手流式事件、日报生成所有权、时间线增量合并等并发状态，使用判别联合和请求 ID 类型，不改现有时序。
- 所有手工 DTO 必须引用对应 Rust struct/command 位置，并在后续阶段评估用 Rust 生成 TypeScript 类型，避免长期双维护。
