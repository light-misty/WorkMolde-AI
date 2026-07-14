# WorkMolde AI 数据库设计文档

> 版本：0.1.6
> 最后更新：2026-06-14
> 适用项目：WorkMolde AI文档处理桌面应用

---

## 目录

1. [概述](#1-概述)
2. [SQLite 表设计](#2-sqlite-表设计)
   - [sessions 会话表](#21-sessions-会话表)
   - [session_messages 消息表](#22-session_messages-消息表)
   - [version_snapshots 版本快照表](#23-version_snapshots-版本快照表)
   - [session_summaries 会话摘要表](#24-session_summaries-会话摘要表)
   - [templates 模板表](#25-templates-模板表)
   - [user_preferences 用户偏好表](#26-user_preferences-用户偏好表)
3. [索引设计](#3-索引设计)
4. [JSON 配置文件 Schema](#4-json-配置文件-schema)
   - [llm_config.json](#41-llm_configjson)
   - [app_settings.json](#42-app_settingsjson)
   - [workspaces.json](#43-workspacesjson)
5. [数据迁移策略](#5-数据迁移策略)
6. [附录](#6-附录)

---

## 1. 概述

WorkMolde AI 使用 **SQLite** 作为本地嵌入式数据库，存储会话记录、消息历史、版本快照元数据、模板及用户偏好。应用配置信息采用 **JSON 文件** 存储。

### 设计原则

- **轻量嵌入**：SQLite 无需独立服务进程，随应用启动即可使用
- **单文件存储**：数据库整体存储为单个 `.db` 文件，便于备份与迁移
- **配置分离**：业务数据存 SQLite，应用配置存 JSON，职责清晰
- **向前兼容**：所有表均包含版本号字段，支持增量迁移

### 数据库文件位置

```
{app_data_dir}/workmolde.db
```

其中 `{app_data_dir}` 为 Tauri 的 `app.path().app_data_dir()` 返回路径（Windows 上为 `%APPDATA%/workmolde`）。

---

## 2. SQLite 表设计

### 2.1 sessions 会话表

存储用户与 AI 的会话元信息。

#### DDL

```sql
CREATE TABLE IF NOT EXISTS sessions (
    id                TEXT NOT NULL PRIMARY KEY,
    title             TEXT NOT NULL DEFAULT '新会话',
    workspace_id      TEXT DEFAULT NULL,
    provider_id       TEXT NOT NULL DEFAULT '',
    template_id       TEXT DEFAULT NULL,
    status            TEXT NOT NULL DEFAULT 'active',
    created_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
```

#### 字段说明

| 字段 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `id` | TEXT | PRIMARY KEY | 会话唯一标识，UUID v4 |
| `title` | TEXT | NOT NULL, DEFAULT '新会话' | 会话标题 |
| `workspace_id` | TEXT | DEFAULT NULL | 所属工作区 ID |
| `provider_id` | TEXT | NOT NULL, DEFAULT '' | 使用的 LLM Provider ID |
| `template_id` | TEXT | DEFAULT NULL | 关联的 Prompt 模板 ID |
| `status` | TEXT | NOT NULL, DEFAULT 'active' | 状态：active / archived |
| `created_at` | TEXT | NOT NULL | 创建时间，ISO 8601 |
| `updated_at` | TEXT | NOT NULL | 最后更新时间，ISO 8601 |

### 2.2 session_messages 消息表

存储会话中的每一条消息。

#### DDL

```sql
CREATE TABLE IF NOT EXISTS session_messages (
    id                TEXT NOT NULL PRIMARY KEY,
    session_id        TEXT NOT NULL,
    role              TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'tool')),
    content           TEXT NOT NULL DEFAULT '',
    tool_name         TEXT DEFAULT NULL,
    tool_args         TEXT DEFAULT NULL,
    tool_result       TEXT DEFAULT NULL,
    thinking_content  TEXT DEFAULT NULL,
    iteration_group   INTEGER DEFAULT 0,
    tool_call_id      TEXT DEFAULT NULL,
    is_streaming      INTEGER DEFAULT 0,
    is_accumulated    INTEGER DEFAULT 0,
    created_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
```

#### 字段说明

| 字段 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `id` | TEXT | PRIMARY KEY | 消息唯一标识，UUID v4 |
| `session_id` | TEXT | NOT NULL | 所属会话 ID |
| `role` | TEXT | NOT NULL, CHECK | user / assistant / tool |
| `content` | TEXT | NOT NULL, DEFAULT '' | 消息正文 |
| `tool_name` | TEXT | DEFAULT NULL | 工具名称，role='tool' 时有值 |
| `tool_args` | TEXT | DEFAULT NULL | 工具参数 JSON |
| `tool_result` | TEXT | DEFAULT NULL | 工具执行结果 JSON |
| `thinking_content` | TEXT | DEFAULT NULL | AI 思考过程内容 |
| `iteration_group` | INTEGER | DEFAULT 0 | Agent 执行轮次编号 |
| `tool_call_id` | TEXT | DEFAULT NULL | LLM 返回的 tool_call ID |
| `is_streaming` | INTEGER | DEFAULT 0 | 是否为流式中间消息 |
| `is_accumulated` | INTEGER | DEFAULT 0 | 是否为累积拼接消息 |
| `created_at` | TEXT | NOT NULL | 创建时间 |

### 2.3 version_snapshots 版本快照表

#### DDL

```sql
CREATE TABLE IF NOT EXISTS version_snapshots (
    id                TEXT NOT NULL PRIMARY KEY,
    workspace_id      TEXT NOT NULL,
    session_id        TEXT NOT NULL,
    file_path         TEXT NOT NULL,
    snapshot_path     TEXT NOT NULL,
    operation         TEXT NOT NULL DEFAULT '',
    created_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
```

### 2.4 session_summaries 会话摘要表

存储会话的摘要信息（用于上下文窗口管理）。

#### DDL

```sql
CREATE TABLE IF NOT EXISTS session_summaries (
    id                TEXT NOT NULL PRIMARY KEY,
    session_id        TEXT NOT NULL,
    summary           TEXT NOT NULL,
    token_count       INTEGER NOT NULL DEFAULT 0,
    created_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
```

### 2.5 templates 模板表

存储 Prompt 模板。

#### DDL

```sql
CREATE TABLE IF NOT EXISTS templates (
    id                TEXT NOT NULL PRIMARY KEY,
    name              TEXT NOT NULL,
    description       TEXT NOT NULL DEFAULT '',
    content           TEXT NOT NULL,
    variables         TEXT DEFAULT '[]',
    category          TEXT DEFAULT '',
    is_builtin        INTEGER DEFAULT 0,
    created_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
```

### 2.6 user_preferences 用户偏好表

```sql
CREATE TABLE IF NOT EXISTS user_preferences (
    key               TEXT NOT NULL PRIMARY KEY,
    value             TEXT NOT NULL
);
```

---

## 3. 索引设计

### 3.1 索引 DDL

```sql
-- sessions 表
CREATE INDEX IF NOT EXISTS idx_sessions_workspace_id ON sessions (workspace_id);
CREATE INDEX IF NOT EXISTS idx_sessions_updated_at ON sessions (updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_sessions_created_at ON sessions (created_at DESC);

-- session_messages 表
CREATE INDEX IF NOT EXISTS idx_session_messages_session_id ON session_messages (session_id);
CREATE INDEX IF NOT EXISTS idx_session_messages_session_id_created ON session_messages (session_id, created_at ASC);
CREATE INDEX IF NOT EXISTS idx_session_messages_role ON session_messages (role);

-- version_snapshots 表
CREATE INDEX IF NOT EXISTS idx_version_snapshots_workspace_id ON version_snapshots (workspace_id);
CREATE INDEX IF NOT EXISTS idx_version_snapshots_session_id ON version_snapshots (session_id);
CREATE INDEX IF NOT EXISTS idx_version_snapshots_file_path ON version_snapshots (file_path);
CREATE INDEX IF NOT EXISTS idx_version_snapshots_created_at ON version_snapshots (created_at DESC);

-- session_summaries 表
CREATE INDEX IF NOT EXISTS idx_session_summaries_session_id ON session_summaries (session_id);

-- templates 表
CREATE INDEX IF NOT EXISTS idx_templates_category ON templates (category);
```

---

## 4. JSON 配置文件 Schema

### 4.1 llm_config.json

#### 文件位置

```
{app_data_dir}/config/llm_config.json
```

#### Schema

```json
{
  "providers": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440001",
      "name": "OpenAI",
      "provider_type": "openai",
      "api_base": "https://api.openai.com/v1",
      "api_key": "sk-...",
      "model": "gpt-4o",
      "is_default": true,
      "context_window": 128000,
      "supports_vision": true,
      "temperature": 0.7,
      "max_tokens": 4096,
      "top_p": 1.0,
      "extra_params": {}
    }
  ]
}
```

Provider 类型：`openai | anthropic | ollama | gemini | custom`

注意：API Key 以明文存储（用户自行保护配置文件安全）。

### 4.2 app_settings.json

#### 文件位置

```
{app_data_dir}/config/app_settings.json
```

#### Schema

```json
{
  "general": {
    "author_name": "张三",
    "author_email": "",
    "author_company": "",
    "confirmation_level": "Always"
  },
  "appearance": {
    "theme_mode": "system",
    "language": "zh-CN",
    "language_follow_system": true
  },
  "version_snapshot": {
    "retention_policy": "Both",
    "max_count": 50,
    "max_days": 30
  },
  "workspace": {
    "default_workspace_id": ""
  },
  "shortcuts": {
    "new_session": "CmdOrCtrl+N",
    "close_session": "CmdOrCtrl+W",
    "send_message": "CmdOrCtrl+Enter",
    "toggle_sidebar": "CmdOrCtrl+B",
    "quick_prompt": "CmdOrCtrl+Shift+P"
  },
  "update": {
    "auto_check": true
  }
}
```

### 4.3 workspaces.json

#### 文件位置

```
{app_data_dir}/config/workspaces.json
```

#### Schema

```json
{
  "workspaces": [
    {
      "id": "ws-uuid-1",
      "name": "项目文档",
      "path": "D:/Documents/ProjectDocs",
      "created_at": "2026-05-14T10:00:00Z"
    }
  ]
}
```

---

## 5. 数据迁移策略

### 5.1 迁移框架

采用版本号追踪机制，在数据库启动时自动执行迁移。

```sql
CREATE TABLE IF NOT EXISTS schema_version (
    version     INTEGER NOT NULL PRIMARY KEY,
    description TEXT    NOT NULL,
    applied_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
```

当前版本为 1（初始建表），包含 6 张表。

### 5.2 迁移执行流程

1. 打开数据库连接
2. 检查 `schema_version` 表是否存在 → 不存在则执行完整建表脚本
3. 读取当前版本号 → 应用版本号 > 当前版本的迁移脚本
4. 每个脚本在独立事务中执行

---

## 6. 附录

### 完整建表脚本

见 `src-tauri/src/db/init.rs` 中的 `run_migrations` 函数。

### 数据类型映射

| 逻辑类型 | SQLite 类型 | 说明 |
|----------|-------------|------|
| UUID | TEXT | UUID v4 字符串 |
| 时间戳 | TEXT | ISO 8601 格式 |
| 整数 | INTEGER | SQLite 原生整数 |
| 文本 | TEXT | SQLite 原生文本 |
| JSON | TEXT | JSON 序列化后存储 |
| 布尔 | INTEGER | 0=false, 1=true |

### 命名规范

| 类别 | 规范 | 示例 |
|------|------|------|
| 表名 | 小写蛇形，复数 | `sessions`, `session_messages` |
| 字段名 | 小写蛇形 | `workspace_id`, `created_at` |
| 索引名 | `idx_{表名}_{字段名}` | `idx_sessions_workspace_id` |
| 主键 | `id` | 所有表统一使用 `id` |
| 外键 | `{表名}_id` | `session_id`, `workspace_id` |
| 时间字段 | `{动作}_at` | `created_at`, `updated_at` |
