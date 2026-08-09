# 语义记忆隐私一致性与索引重建设计

## 背景

当前 `memory_chunks` 与源 `activities` 没有来源映射；活动删除、隐私规则变化、Embedding
模型变化和旧活动后补 OCR 都可能留下过期向量。语义检索阶段也没有完整应用当前隐私
规则。当前语义记忆默认关闭，但功能一旦启用就必须保证删除和隐私收紧立即生效。

## 目标

1. 活动删除或隐私规则变化后，旧语义块立即不可检索。
2. Embedding Provider、Endpoint、模型或维度变化后，旧向量不得与新查询向量混用。
3. 提供可恢复的全量重建入口和真实状态；重建期间自动降级到 FTS。
4. 索引和检索都应用完整隐私规则；`Anonymize` 与 `Skip` 内容不进入语义正文。
5. 不引入 Generation、事件溯源、ANN 或跨设备同步。

## 采用方案

采用“全量失效 + 指纹校验 + 显式重建 + 检索 fail-closed”。

- 新增单例 `semantic_memory_state`，记录 Embedding 指纹、隐私指纹、索引状态、是否需要
  重建、进度和最后错误。
- 隐私规则变化或活动删除时，在数据库事务边界使旧索引失效并清空 `memory_chunks`。
- Embedding 配置变化时标记待重建；重建完成前只使用 FTS，不执行向量点积。
- 设置页按钮调用后台命令推进全量重建；页面只展示状态，不再承担索引一致性。
- 现有 `index_semantic_memory` 保持分批执行，但重建从游标 0 开始，完成后写入指纹并标记
  `ready`。

## 数据模型

新增单例表：

```sql
CREATE TABLE semantic_memory_state (
    singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
    embedding_fingerprint TEXT NOT NULL DEFAULT '',
    privacy_fingerprint TEXT NOT NULL DEFAULT '',
    rebuild_required INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'idle',
    indexed_activities INTEGER NOT NULL DEFAULT 0,
    total_activities INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    updated_at INTEGER NOT NULL
);
```

状态只允许：`idle`、`building`、`ready`、`failed`。

Embedding 指纹包含规范化后的 Provider、Endpoint、模型、真实向量维度、分块规则版本和
向量归一化版本；API Key 不参与。隐私指纹包含应用规则、排除标题关键词和排除域名的
规范化表示。

## 隐私语义

- `Record`：可以进入索引。
- `Anonymize`：不进入语义索引，避免历史标题、URL、OCR 泄漏。
- `Skip`：不进入语义索引。
- 检索结果在返回 Agent 前再次使用当前完整隐私规则过滤。
- 规则收紧后先使旧索引失效，再保存新配置；任何失败都不能继续查询旧向量。

## 删除与重建

所有活动删除入口最终通过数据库方法执行。删除事务成功时：

1. 删除活动；
2. 失效相关摘要缓存；
3. 清空 `memory_chunks`；
4. 将状态设为 `rebuild_required=1,status='idle'`。

重建流程：

1. 测试 Embedding 模型并获得维度；
2. 计算当前指纹；
3. 清空旧索引并进入 `building`；
4. 分批读取活动、完整应用隐私规则、构建块并生成向量；
5. 失败写入 `failed` 和 `last_error`；
6. 全部完成后写入当前指纹并进入 `ready`；
7. 查询仅在状态为 `ready` 且当前指纹匹配时使用向量，否则降级 FTS。

## UI

设置页显示：

- `未建立`、`需要重建`、`正在建立`、`可用`、`建立失败`；
- 已处理活动数、总活动数；
- 最后错误；
- “建立索引”“重新建立”“重试”按钮。

隐私或模型配置变化后立即显示“需要重建”。

## 非目标

- 不做影子 Generation 和蓝绿切换。
- 不实现 Activity Outbox 或持续后台 Worker。
- 不引入 SQLite 向量扩展或 ANN。
- 不自动开启语义记忆。
