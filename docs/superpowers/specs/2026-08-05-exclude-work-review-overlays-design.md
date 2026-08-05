# 排除 Work Review 自身浮动窗口设计

## 背景

macOS 活动监控会通过 `CGWindowListCopyWindowInfo` 枚举 `layer > 0` 的窗口，
用于补充画中画等浮动窗口的使用时长。当其他应用位于前台时，Work Review 的
桌宠和通知气泡仍保持置顶，窗口所有者会被系统报告为 `work-review`，从而错误地
给 Work Review 活动累计时长。

## 目标

- Work Review 的桌宠、通知气泡及其他自身浮动窗口不参与活动时长统计。
- Work Review 主窗口真正成为前台应用时，仍通过正常的前台活动流程计时。
- 其他应用的有效画中画窗口继续沿用现有统计逻辑。

## 方案

在 `monitor.rs` 中读取 CGWindow 提供的 `kCGWindowOwnerPID`，并与当前
Work Review 进程 PID 精确比较。PID 相同时说明窗口属于当前应用，无需依赖
应用名称、窗口标题或尺寸。

`get_overlay_windows()` 读取到窗口所有者后，在尺寸、标题和分类判断之前排除
Work Review 自身窗口。该过滤只作用于浮动窗口补充统计，不改变
`get_active_window()` 的前台窗口识别，所以主窗口前台计时保持不变。

## 测试边界

单元测试覆盖：

- 当前进程 PID 对应的浮动窗口会被识别为自身窗口。
- 其他进程 PID 不会被误排除。
- 缺少 PID 信息时不会误判为自身窗口。

## 非目标

- 不改变其他应用浮动窗口的尺寸、标题和系统进程过滤规则。
- 不尝试通过 CGWindow API 区分同一进程内的 Tauri 窗口标签。
- 不修改桌宠窗口本身的显示或交互行为。
