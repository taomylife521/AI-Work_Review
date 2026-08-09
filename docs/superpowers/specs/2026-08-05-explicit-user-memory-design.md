# 用户显式长期记忆设计

## 背景

现有助手能保存会话、检索工作活动和读取规则洞察，但不能稳定保存用户明确声明的回答
偏好、工作流程、长期目标和助手约束。显式长期记忆必须与 OCR/屏幕语义记忆隔离，并由
用户拥有和控制。

## 目标

1. 用户明确说“记住、修改、忘掉”时，模型可以提出结构化操作候选。
2. 新增、修改和删除必须经过用户确认；普通聊天不得静默写入。
3. 用户可在设置页查看、手动新增、编辑、硬删除和清空长期记忆。
4. 召回有条数和字符预算，不能覆盖系统安全、工具权限或确认要求。
5. 密码、令牌、私钥等认证信息禁止保存。

## 采用方案

采用“模型提取候选 + 用户确认后写入”，同时提供设置页手动管理。

后端根据原始用户消息识别显式记忆意图。普通问题只注册查询工具；只有明确记忆意图时
才注册对应写工具，避免网页、OCR 或工具结果通过提示注入获得记忆写权限。

## 数据模型

新增 `assistant_user_memories`：

```sql
CREATE TABLE assistant_user_memories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    memory_type TEXT NOT NULL,
    memory_key TEXT NOT NULL,
    value_text TEXT NOT NULL,
    recall_policy TEXT NOT NULL DEFAULT 'relevant',
    sensitivity TEXT NOT NULL DEFAULT 'normal',
    source_kind TEXT NOT NULL DEFAULT 'explicit_chat',
    source_conversation_id INTEGER,
    source_request_id TEXT,
    revision INTEGER NOT NULL DEFAULT 1,
    expires_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(memory_type, memory_key)
);
```

第一版类型：`preference`、`workflow`、`profile`、`goal`、`project`、`constraint`。
召回策略：`always`、`relevant`、`manual`。

## 工具和确认

- `search_user_memories`：只读，无需确认。
- `remember_user_memory`：新增，必须确认。
- `update_user_memory`：按 ID 和 revision 修改，必须确认。
- `forget_user_memory`：按 ID 和 revision 硬删除，必须确认。

同一 `type + key` 不允许静默覆盖；必须转为修改操作。删除只删除结构化长期记忆，不删除
原始聊天、OCR 或活动数据，确认卡片必须明确这一点。

第一版沿用现有批准/拒绝确认协议，不加入确认卡片内编辑；用户要修改候选时先取消，再用
更明确的自然语言重新提出，避免扩大确认协议和临时敏感参数持久化范围。

## 召回

- `always`：每次请求最多 8 条。
- `relevant`：按 key、类型和值的本地文本匹配，最多 6 条。
- `manual`：只在用户查询“你记得什么”或调用搜索工具时返回。
- 总计最多 12 条、2400 字符。
- 注入系统提示词时标记为“用户确认的长期记忆”，并声明不能覆盖系统和权限规则。

第一版不使用 Embedding，避免重复引入索引、模型版本和删除一致性问题。

## 隐私与删除

- 禁止保存密码、API Key、Access Token、私钥、恢复码和认证 Cookie。
- 电话、邮箱、地址、健康和财务信息标为 `caution`，默认不得 `always` 召回。
- “忘掉”执行硬删除，不保留旧值。
- 设置页明确提示：本地保存的记忆被召回后，可能发送给用户配置的云模型。
- 工具摘要和日志不得包含完整 `value_text`。

## UI

设置页 AI/记忆区域新增独立的“助手长期记忆”管理组件，与“屏幕语义记忆”分区：

- 功能开关；
- 记忆列表、类型筛选和搜索；
- 手动新增、编辑、删除；
- 清空全部二次确认；
- 展示召回策略、来源和更新时间。

第一版不实现导入、自动学习、候选记忆箱或跨设备同步。
