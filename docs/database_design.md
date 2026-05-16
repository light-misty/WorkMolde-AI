# DocAgent 数据库设计文档

> 版本：1.0.0
> 最后更新：2026-05-14
> 适用项目：DocAgent AI文档处理桌面应用

---

## 目录

1. [概述](#1-概述)
2. [SQLite 表设计](#2-sqlite-表设计)
   - [sessions 会话表](#21-sessions-会话表)
   - [session_messages 消息表](#22-session_messages-消息表)
   - [version_snapshots 版本快照表](#23-version_snapshots-版本快照表)
   - [token_usage Token统计表](#24-token_usage-token统计表)
3. [索引设计](#3-索引设计)
4. [JSON 配置文件 Schema](#4-json-配置文件-schema)
   - [llm_config.json](#41-llm_configjson)
   - [app_settings.json](#42-app_settingsjson)
   - [workspaces.json](#43-workspacesjson)
   - [prompt_templates.json](#44-prompt_templatesjson)
5. [数据迁移策略](#5-数据迁移策略)
6. [附录](#6-附录)

---

## 1. 概述

DocAgent 使用 **SQLite** 作为本地嵌入式数据库，存储会话记录、消息历史、版本快照元数据及 Token 使用统计。应用配置信息采用 **JSON 文件** 存储，便于用户手动编辑与版本管理。

### 设计原则

- **轻量嵌入**：SQLite 无需独立服务进程，随应用启动即可使用
- **单文件存储**：数据库整体存储为单个 `.db` 文件，便于备份与迁移
- **配置分离**：业务数据存 SQLite，应用配置存 JSON，职责清晰
- **向前兼容**：所有表均包含版本号字段，支持增量迁移

### 数据库文件位置

```
{userData}/docagent/docagent.db
```

其中 `{userData}` 为 Electron 的 `app.getPath('userData')` 返回路径。

---

## 2. SQLite 表设计

### 2.1 sessions 会话表

存储用户与 AI 的会话元信息，每个会话关联一个工作区。

#### DDL

```sql
CREATE TABLE IF NOT EXISTS sessions (
    id                TEXT        NOT NULL PRIMARY KEY,
    workspace_id      TEXT        NOT NULL,
    title             TEXT        NOT NULL DEFAULT '新会话',
    created_at        TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at        TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    total_input_tokens  INTEGER   NOT NULL DEFAULT 0,
    total_output_tokens INTEGER   NOT NULL DEFAULT 0,
    llm_provider      TEXT        NOT NULL DEFAULT '',
    llm_model         TEXT        NOT NULL DEFAULT ''
);
```

#### 字段说明

| 字段 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `id` | TEXT | PRIMARY KEY | 会话唯一标识，UUID v4 |
| `workspace_id` | TEXT | NOT NULL | 所属工作区 ID，关联 workspaces.json |
| `title` | TEXT | NOT NULL, DEFAULT '新会话' | 会话标题，可由用户修改或由 AI 自动生成 |
| `created_at` | TEXT | NOT NULL | 创建时间，ISO 8601 格式 |
| `updated_at` | TEXT | NOT NULL | 最后更新时间，ISO 8601 格式 |
| `total_input_tokens` | INTEGER | NOT NULL, DEFAULT 0 | 该会话累计输入 Token 数 |
| `total_output_tokens` | INTEGER | NOT NULL, DEFAULT 0 | 该会话累计输出 Token 数 |
| `llm_provider` | TEXT | NOT NULL, DEFAULT '' | 使用的 LLM 提供商标识（如 openai、anthropic） |
| `llm_model` | TEXT | NOT NULL, DEFAULT '' | 使用的 LLM 模型名称（如 gpt-4o、claude-3-sonnet） |

#### 业务规则

- `id` 由应用层生成 UUID v4，不依赖数据库自增
- `updated_at` 在每次消息写入时由应用层更新
- `total_input_tokens` / `total_output_tokens` 为冗余聚合字段，由应用层在写入消息时同步更新
- `workspace_id` 为逻辑外键，指向 workspaces.json 中的工作区，不建立物理外键约束

---

### 2.2 session_messages 消息表

存储会话中的每一条消息，包括用户输入、AI 回复及工具调用结果。

#### DDL

```sql
CREATE TABLE IF NOT EXISTS session_messages (
    id                TEXT        NOT NULL PRIMARY KEY,
    session_id        TEXT        NOT NULL,
    role              TEXT        NOT NULL CHECK (role IN ('user', 'assistant', 'tool')),
    content           TEXT        NOT NULL DEFAULT '',
    tool_name         TEXT        DEFAULT NULL,
    tool_args         TEXT        DEFAULT NULL,
    tool_result       TEXT        DEFAULT NULL,
    thinking_content  TEXT        DEFAULT NULL,
    input_tokens      INTEGER     NOT NULL DEFAULT 0,
    output_tokens     INTEGER     NOT NULL DEFAULT 0,
    created_at        TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
```

#### 字段说明

| 字段 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `id` | TEXT | PRIMARY KEY | 消息唯一标识，UUID v4 |
| `session_id` | TEXT | NOT NULL | 所属会话 ID，关联 sessions.id |
| `role` | TEXT | NOT NULL, CHECK | 消息角色：user / assistant / tool |
| `content` | TEXT | NOT NULL, DEFAULT '' | 消息正文内容（Markdown 格式） |
| `tool_name` | TEXT | DEFAULT NULL | 工具调用名称，仅 role='tool' 时有值 |
| `tool_args` | TEXT | DEFAULT NULL | 工具调用参数（JSON 字符串），仅 role='tool' 时有值 |
| `tool_result` | TEXT | DEFAULT NULL | 工具调用返回结果（JSON 字符串），仅 role='tool' 时有值 |
| `thinking_content` | TEXT | DEFAULT NULL | AI 思考过程内容（支持扩展思考的模型） |
| `input_tokens` | INTEGER | NOT NULL, DEFAULT 0 | 本条消息消耗的输入 Token 数 |
| `output_tokens` | INTEGER | NOT NULL, DEFAULT 0 | 本条消息消耗的输出 Token 数 |
| `created_at` | TEXT | NOT NULL | 消息创建时间，ISO 8601 格式 |

#### 业务规则

- `role` 字段通过 CHECK 约束限制为 `user`、`assistant`、`tool` 三种值
- 当 `role='tool'` 时，`tool_name`、`tool_args`、`tool_result` 应有值；其他 role 下为 NULL
- `tool_args` 和 `tool_result` 存储为 JSON 字符串，应用层负责序列化/反序列化
- `thinking_content` 用于存储支持"扩展思考"的模型（如 Claude）返回的思考过程
- `session_id` 为逻辑外键，不建立物理外键约束，删除会话时由应用层级联删除消息

---

### 2.3 version_snapshots 版本快照表

存储文档版本快照的元数据，快照文件本身存储在文件系统中。

#### DDL

```sql
CREATE TABLE IF NOT EXISTS version_snapshots (
    id                TEXT        NOT NULL PRIMARY KEY,
    workspace_id      TEXT        NOT NULL,
    session_id        TEXT        NOT NULL,
    file_path         TEXT        NOT NULL,
    snapshot_path     TEXT        NOT NULL,
    operation         TEXT        NOT NULL DEFAULT '',
    created_at        TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
```

#### 字段说明

| 字段 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `id` | TEXT | PRIMARY KEY | 快照唯一标识，UUID v4 |
| `workspace_id` | TEXT | NOT NULL | 所属工作区 ID |
| `session_id` | TEXT | NOT NULL | 触发快照的会话 ID |
| `file_path` | TEXT | NOT NULL | 原始文档的相对路径（相对于工作区根目录） |
| `snapshot_path` | TEXT | NOT NULL | 快照文件的存储路径（相对于快照根目录） |
| `operation` | TEXT | NOT NULL, DEFAULT '' | 触发快照的操作描述（如"AI编辑"、"用户确认"） |
| `created_at` | TEXT | NOT NULL | 快照创建时间，ISO 8601 格式 |

#### 业务规则

- `file_path` 存储相对于工作区根目录的路径，使用正斜杠 `/` 作为分隔符
- `snapshot_path` 存储相对于应用快照根目录 `{userData}/docagent/snapshots/` 的路径
- 快照文件命名规则：`{file_hash}_{timestamp}.md`
- 清理策略由 `app_settings.json` 中的 `version_snapshot` 配置控制
- 删除快照记录时，应用层需同步删除对应的快照文件

---

### 2.4 token_usage Token统计表

存储每次 LLM 调用的 Token 消耗明细，用于预算控制与使用分析。

#### DDL

```sql
CREATE TABLE IF NOT EXISTS token_usage (
    id                TEXT        NOT NULL PRIMARY KEY,
    session_id        TEXT        NOT NULL,
    workspace_id      TEXT        NOT NULL,
    llm_provider      TEXT        NOT NULL DEFAULT '',
    llm_model         TEXT        NOT NULL DEFAULT '',
    input_tokens      INTEGER     NOT NULL DEFAULT 0,
    output_tokens     INTEGER     NOT NULL DEFAULT 0,
    created_at        TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
```

#### 字段说明

| 字段 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `id` | TEXT | PRIMARY KEY | 记录唯一标识，UUID v4 |
| `session_id` | TEXT | NOT NULL | 关联的会话 ID |
| `workspace_id` | TEXT | NOT NULL | 关联的工作区 ID |
| `llm_provider` | TEXT | NOT NULL, DEFAULT '' | LLM 提供商标识 |
| `llm_model` | TEXT | NOT NULL, DEFAULT '' | LLM 模型名称 |
| `input_tokens` | INTEGER | NOT NULL, DEFAULT 0 | 本次调用输入 Token 数 |
| `output_tokens` | INTEGER | NOT NULL, DEFAULT 0 | 本次调用输出 Token 数 |
| `created_at` | TEXT | NOT NULL | 记录创建时间，ISO 8601 格式 |

#### 业务规则

- 每次成功的 LLM API 调用均写入一条记录
- 日/月预算检查通过聚合查询实现，参见索引设计
- `workspace_id` 冗余存储，便于按工作区维度统计

---

## 3. 索引设计

### 3.1 索引 DDL

```sql
-- sessions 表索引
CREATE INDEX IF NOT EXISTS idx_sessions_workspace_id
    ON sessions (workspace_id);

CREATE INDEX IF NOT EXISTS idx_sessions_updated_at
    ON sessions (updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_sessions_created_at
    ON sessions (created_at DESC);

-- session_messages 表索引
CREATE INDEX IF NOT EXISTS idx_session_messages_session_id
    ON session_messages (session_id);

CREATE INDEX IF NOT EXISTS idx_session_messages_session_id_created_at
    ON session_messages (session_id, created_at ASC);

CREATE INDEX IF NOT EXISTS idx_session_messages_role
    ON session_messages (role);

-- version_snapshots 表索引
CREATE INDEX IF NOT EXISTS idx_version_snapshots_workspace_id
    ON version_snapshots (workspace_id);

CREATE INDEX IF NOT EXISTS idx_version_snapshots_session_id
    ON version_snapshots (session_id);

CREATE INDEX IF NOT EXISTS idx_version_snapshots_file_path
    ON version_snapshots (file_path);

CREATE INDEX IF NOT EXISTS idx_version_snapshots_created_at
    ON version_snapshots (created_at DESC);

-- token_usage 表索引
CREATE INDEX IF NOT EXISTS idx_token_usage_session_id
    ON token_usage (session_id);

CREATE INDEX IF NOT EXISTS idx_token_usage_workspace_id
    ON token_usage (workspace_id);

CREATE INDEX IF NOT EXISTS idx_token_usage_created_at
    ON token_usage (created_at DESC);

CREATE INDEX IF NOT EXISTS idx_token_usage_workspace_created
    ON token_usage (workspace_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_token_usage_provider_model
    ON token_usage (llm_provider, llm_model);
```

### 3.2 索引说明

| 索引名 | 所在表 | 字段 | 用途 |
|--------|--------|------|------|
| `idx_sessions_workspace_id` | sessions | workspace_id | 按工作区查询会话列表 |
| `idx_sessions_updated_at` | sessions | updated_at DESC | 按最近更新排序会话 |
| `idx_sessions_created_at` | sessions | created_at DESC | 按创建时间排序会话 |
| `idx_session_messages_session_id` | session_messages | session_id | 按会话查询消息 |
| `idx_session_messages_session_id_created_at` | session_messages | session_id, created_at ASC | 按时间顺序加载会话消息（核心查询） |
| `idx_session_messages_role` | session_messages | role | 按角色筛选消息 |
| `idx_version_snapshots_workspace_id` | version_snapshots | workspace_id | 按工作区查询快照 |
| `idx_version_snapshots_session_id` | version_snapshots | session_id | 按会话查询快照 |
| `idx_version_snapshots_file_path` | version_snapshots | file_path | 按文件路径查询快照历史 |
| `idx_version_snapshots_created_at` | version_snapshots | created_at DESC | 按时间排序快照/清理过期快照 |
| `idx_token_usage_session_id` | token_usage | session_id | 按会话查询 Token 用量 |
| `idx_token_usage_workspace_id` | token_usage | workspace_id | 按工作区查询 Token 用量 |
| `idx_token_usage_created_at` | token_usage | created_at DESC | 按时间排序/清理过期记录 |
| `idx_token_usage_workspace_created` | token_usage | workspace_id, created_at DESC | 按工作区+时间范围聚合（日/月预算检查） |
| `idx_token_usage_provider_model` | token_usage | llm_provider, llm_model | 按提供商和模型统计用量 |

---

## 4. JSON 配置文件 Schema

### 4.1 llm_config.json

LLM 提供商与模型配置文件。

#### 文件位置

```
{userData}/docagent/config/llm_config.json
```

#### Schema 定义

```jsonc
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "LLMConfig",
  "type": "object",
  "required": ["providers", "fallback_order"],
  "additionalProperties": false,
  "properties": {
    "providers": {
      "type": "array",
      "description": "LLM 提供商列表",
      "items": {
        "type": "object",
        "required": ["id", "type", "name", "api_base_url", "api_key_encrypted", "model", "is_default"],
        "additionalProperties": false,
        "properties": {
          "id": {
            "type": "string",
            "description": "提供商唯一标识，UUID v4",
            "pattern": "^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$"
          },
          "type": {
            "type": "string",
            "description": "提供商类型",
            "enum": ["openai", "anthropic", "azure_openai", "ollama", "custom"]
          },
          "name": {
            "type": "string",
            "description": "提供商显示名称",
            "minLength": 1,
            "maxLength": 100
          },
          "api_base_url": {
            "type": "string",
            "description": "API 基础地址",
            "format": "uri",
            "examples": ["https://api.openai.com/v1", "https://api.anthropic.com"]
          },
          "api_key_encrypted": {
            "type": "string",
            "description": "加密后的 API Key（AES-256-GCM 加密，Base64 编码存储）"
          },
          "model": {
            "type": "string",
            "description": "默认模型名称",
            "examples": ["gpt-4o", "claude-3-5-sonnet-20241022", "deepseek-chat"]
          },
          "is_default": {
            "type": "boolean",
            "description": "是否为默认提供商（全局仅一个）",
            "default": false
          },
          "advanced": {
            "type": "object",
            "description": "高级参数配置",
            "additionalProperties": false,
            "properties": {
              "temperature": {
                "type": "number",
                "description": "生成温度，0.0-2.0",
                "minimum": 0,
                "maximum": 2,
                "default": 1.0
              },
              "top_p": {
                "type": "number",
                "description": "Top-P 采样参数",
                "minimum": 0,
                "maximum": 1,
                "default": 1.0
              },
              "max_tokens": {
                "type": "integer",
                "description": "最大输出 Token 数",
                "minimum": 1,
                "maximum": 128000,
                "default": 4096
              },
              "timeout_seconds": {
                "type": "integer",
                "description": "请求超时时间（秒）",
                "minimum": 5,
                "maximum": 300,
                "default": 60
              },
              "max_retries": {
                "type": "integer",
                "description": "最大重试次数",
                "minimum": 0,
                "maximum": 5,
                "default": 2
              },
              "extra_headers": {
                "type": "object",
                "description": "额外请求头",
                "additionalProperties": { "type": "string" }
              }
            }
          }
        }
      }
    },
    "fallback_order": {
      "type": "array",
      "description": "故障转移顺序，按优先级排列的提供商 ID 列表",
      "items": {
        "type": "string"
      }
    }
  }
}
```

#### 示例

```json
{
  "providers": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440001",
      "type": "openai",
      "name": "OpenAI",
      "api_base_url": "https://api.openai.com/v1",
      "api_key_encrypted": "U2FsdGVkX1+...",
      "model": "gpt-4o",
      "is_default": true,
      "advanced": {
        "temperature": 0.7,
        "max_tokens": 4096,
        "timeout_seconds": 60,
        "max_retries": 2
      }
    },
    {
      "id": "550e8400-e29b-41d4-a716-446655440002",
      "type": "anthropic",
      "name": "Anthropic",
      "api_base_url": "https://api.anthropic.com",
      "api_key_encrypted": "U2FsdGVkX2+...",
      "model": "claude-3-5-sonnet-20241022",
      "is_default": false,
      "advanced": {
        "temperature": 1.0,
        "max_tokens": 8192
      }
    }
  ],
  "fallback_order": [
    "550e8400-e29b-41d4-a716-446655440001",
    "550e8400-e29b-41d4-a716-446655440002"
  ]
}
```

---

### 4.2 app_settings.json

应用全局设置配置文件。

#### 文件位置

```
{userData}/docagent/config/app_settings.json
```

#### Schema 定义

```jsonc
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "AppSettings",
  "type": "object",
  "required": ["general", "token_budget", "version_snapshot", "workspace", "shortcuts"],
  "additionalProperties": false,
  "properties": {
    "general": {
      "type": "object",
      "description": "通用设置",
      "required": ["author_name", "confirmation_level", "language"],
      "additionalProperties": false,
      "properties": {
        "author_name": {
          "type": "string",
          "description": "默认作者名称，用于文档元数据",
          "default": "",
          "maxLength": 100
        },
        "confirmation_level": {
          "type": "string",
          "description": "AI 操作确认级别",
          "enum": ["always", "edit_only", "never"],
          "default": "edit_only",
          "enumDescriptions": {
            "always": "所有操作均需确认",
            "edit_only": "仅文件编辑操作需确认",
            "never": "无需确认，自动执行"
          }
        },
        "language": {
          "type": "string",
          "description": "界面语言",
          "enum": ["zh-CN", "en-US", "ja-JP"],
          "default": "zh-CN"
        }
      }
    },
    "token_budget": {
      "type": "object",
      "description": "Token 预算控制",
      "required": ["daily_limit", "monthly_limit", "exceed_action"],
      "additionalProperties": false,
      "properties": {
        "daily_limit": {
          "type": "integer",
          "description": "每日 Token 上限（0 表示不限制）",
          "minimum": 0,
          "default": 0
        },
        "monthly_limit": {
          "type": "integer",
          "description": "每月 Token 上限（0 表示不限制）",
          "minimum": 0,
          "default": 0
        },
        "exceed_action": {
          "type": "string",
          "description": "超出预算时的行为",
          "enum": ["warn", "block", "fallback"],
          "default": "warn",
          "enumDescriptions": {
            "warn": "仅警告，允许继续使用",
            "block": "阻止请求，直到预算重置",
            "fallback": "自动切换到更便宜的模型"
          }
        }
      }
    },
    "version_snapshot": {
      "type": "object",
      "description": "版本快照设置",
      "required": ["retention_policy", "max_count", "max_days"],
      "additionalProperties": false,
      "properties": {
        "retention_policy": {
          "type": "string",
          "description": "快照保留策略",
          "enum": ["by_count", "by_days", "both"],
          "default": "both",
          "enumDescriptions": {
            "by_count": "按数量保留，超出最旧的自动删除",
            "by_days": "按天数保留，超出天数的自动删除",
            "both": "同时满足数量和天数限制"
          }
        },
        "max_count": {
          "type": "integer",
          "description": "每个文件最大快照数量",
          "minimum": 1,
          "maximum": 1000,
          "default": 50
        },
        "max_days": {
          "type": "integer",
          "description": "快照最大保留天数",
          "minimum": 1,
          "maximum": 365,
          "default": 30
        }
      }
    },
    "workspace": {
      "type": "object",
      "description": "工作区默认设置",
      "required": ["default_workspace_id"],
      "additionalProperties": false,
      "properties": {
        "default_workspace_id": {
          "type": "string",
          "description": "默认工作区 ID，启动时自动打开（空字符串表示无默认）",
          "default": ""
        }
      }
    },
    "shortcuts": {
      "type": "object",
      "description": "键盘快捷键配置",
      "additionalProperties": {
        "type": "string",
        "description": "快捷键组合，Electron Accelerator 格式"
      },
      "properties": {
        "new_session": {
          "type": "string",
          "default": "CmdOrCtrl+N"
        },
        "close_session": {
          "type": "string",
          "default": "CmdOrCtrl+W"
        },
        "send_message": {
          "type": "string",
          "default": "CmdOrCtrl+Enter"
        },
        "toggle_sidebar": {
          "type": "string",
          "default": "CmdOrCtrl+B"
        },
        "quick_prompt": {
          "type": "string",
          "default": "CmdOrCtrl+Shift+P"
        }
      }
    }
  }
}
```

#### 示例

```json
{
  "general": {
    "author_name": "张三",
    "confirmation_level": "edit_only",
    "language": "zh-CN"
  },
  "token_budget": {
    "daily_limit": 100000,
    "monthly_limit": 2000000,
    "exceed_action": "warn"
  },
  "version_snapshot": {
    "retention_policy": "both",
    "max_count": 50,
    "max_days": 30
  },
  "workspace": {
    "default_workspace_id": "550e8400-e29b-41d4-a716-446655440010"
  },
  "shortcuts": {
    "new_session": "CmdOrCtrl+N",
    "close_session": "CmdOrCtrl+W",
    "send_message": "CmdOrCtrl+Enter",
    "toggle_sidebar": "CmdOrCtrl+B",
    "quick_prompt": "CmdOrCtrl+Shift+P"
  }
}
```

---

### 4.3 workspaces.json

工作区配置文件，管理用户的工作区列表。

#### 文件位置

```
{userData}/docagent/config/workspaces.json
```

#### Schema 定义

```jsonc
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "Workspaces",
  "type": "object",
  "required": ["workspaces"],
  "additionalProperties": false,
  "properties": {
    "workspaces": {
      "type": "array",
      "description": "工作区列表",
      "items": {
        "type": "object",
        "required": ["id", "name", "path", "created_at"],
        "additionalProperties": false,
        "properties": {
          "id": {
            "type": "string",
            "description": "工作区唯一标识，UUID v4",
            "pattern": "^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$"
          },
          "name": {
            "type": "string",
            "description": "工作区显示名称",
            "minLength": 1,
            "maxLength": 200
          },
          "path": {
            "type": "string",
            "description": "工作区根目录的绝对路径"
          },
          "author_name_override": {
            "type": "string",
            "description": "工作区级别的作者名称覆盖（为空时使用全局设置）",
            "default": ""
          },
          "created_at": {
            "type": "string",
            "description": "创建时间，ISO 8601 格式",
            "format": "date-time"
          }
        }
      }
    }
  }
}
```

#### 示例

```json
{
  "workspaces": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440010",
      "name": "技术文档项目",
      "path": "D:/Projects/tech-docs",
      "author_name_override": "李四",
      "created_at": "2026-01-15T08:30:00.000Z"
    },
    {
      "id": "660e8400-e29b-41d4-a716-446655440020",
      "name": "个人笔记",
      "path": "D:/Notes",
      "author_name_override": "",
      "created_at": "2026-03-20T14:00:00.000Z"
    }
  ]
}
```

---

### 4.4 prompt_templates.json

提示词模板配置文件，管理内置和自定义的提示词模板。

#### 文件位置

```
{userData}/docagent/config/prompt_templates.json
```

#### Schema 定义

```jsonc
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "PromptTemplates",
  "type": "object",
  "required": ["templates"],
  "additionalProperties": false,
  "properties": {
    "templates": {
      "type": "array",
      "description": "提示词模板列表",
      "items": {
        "type": "object",
        "required": ["id", "name", "description", "content", "variables", "is_builtin", "created_at"],
        "additionalProperties": false,
        "properties": {
          "id": {
            "type": "string",
            "description": "模板唯一标识，UUID v4（内置模板使用固定 ID）",
            "pattern": "^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$"
          },
          "name": {
            "type": "string",
            "description": "模板名称",
            "minLength": 1,
            "maxLength": 100
          },
          "description": {
            "type": "string",
            "description": "模板功能描述",
            "maxLength": 500
          },
          "content": {
            "type": "string",
            "description": "模板内容，支持 {{variable}} 占位符语法",
            "minLength": 1
          },
          "variables": {
            "type": "array",
            "description": "模板变量定义列表",
            "items": {
              "type": "object",
              "required": ["name", "label", "type", "default_value"],
              "additionalProperties": false,
              "properties": {
                "name": {
                  "type": "string",
                  "description": "变量名，与 content 中的 {{name}} 对应",
                  "minLength": 1,
                  "maxLength": 50,
                  "pattern": "^[a-zA-Z_][a-zA-Z0-9_]*$"
                },
                "label": {
                  "type": "string",
                  "description": "变量在界面上的显示标签",
                  "minLength": 1
                },
                "type": {
                  "type": "string",
                  "description": "变量类型",
                  "enum": ["string", "number", "select", "boolean"],
                  "default": "string"
                },
                "default_value": {
                  "description": "变量默认值",
                  "type": "string"
                },
                "options": {
                  "type": "array",
                  "description": "当 type='select' 时的可选项列表",
                  "items": {
                    "type": "object",
                    "required": ["value", "label"],
                    "additionalProperties": false,
                    "properties": {
                      "value": { "type": "string" },
                      "label": { "type": "string" }
                    }
                  }
                }
              }
            }
          },
          "is_builtin": {
            "type": "boolean",
            "description": "是否为内置模板（内置模板不可删除，可被更新覆盖）",
            "default": false
          },
          "created_at": {
            "type": "string",
            "description": "创建时间，ISO 8601 格式",
            "format": "date-time"
          }
        }
      }
    }
  }
}
```

#### 示例

```json
{
  "templates": [
    {
      "id": "00000000-0000-4000-a000-000000000001",
      "name": "文档润色",
      "description": "对选中的文档内容进行润色优化，提升可读性和专业性",
      "content": "请对以下文档内容进行润色优化，保持原意不变，提升可读性和专业性。\n\n作者风格参考：{{author_style}}\n\n待润色内容：\n{{content}}",
      "variables": [
        {
          "name": "author_style",
          "label": "作者风格",
          "type": "select",
          "default_value": "professional",
          "options": [
            { "value": "professional", "label": "专业严谨" },
            { "value": "casual", "label": "轻松活泼" },
            { "value": "academic", "label": "学术规范" }
          ]
        },
        {
          "name": "content",
          "label": "待润色内容",
          "type": "string",
          "default_value": ""
        }
      ],
      "is_builtin": true,
      "created_at": "2026-01-01T00:00:00.000Z"
    },
    {
      "id": "550e8400-e29b-41d4-a716-446655440099",
      "name": "翻译助手",
      "description": "将文档内容翻译为指定语言",
      "content": "请将以下内容翻译为{{target_language}}，保持原文的格式和结构：\n\n{{content}}",
      "variables": [
        {
          "name": "target_language",
          "label": "目标语言",
          "type": "string",
          "default_value": "英文"
        },
        {
          "name": "content",
          "label": "待翻译内容",
          "type": "string",
          "default_value": ""
        }
      ],
      "is_builtin": false,
      "created_at": "2026-04-10T10:00:00.000Z"
    }
  ]
}
```

---

## 5. 数据迁移策略

### 5.1 迁移框架设计

采用**版本号追踪**机制，在数据库中维护一个 `schema_version` 元数据表记录当前数据库版本。

```sql
CREATE TABLE IF NOT EXISTS schema_version (
    version     INTEGER NOT NULL PRIMARY KEY,
    description TEXT    NOT NULL,
    applied_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
```

#### 初始版本记录

```sql
INSERT INTO schema_version (version, description)
VALUES (1, '初始建表：sessions, session_messages, version_snapshots, token_usage');
```

### 5.2 迁移脚本规范

每个版本对应一个独立的迁移脚本文件，存放于应用资源目录中：

```
resources/migrations/
  ├── 001_initial.sql          -- 版本1：初始建表
  ├── 002_add_xxx_column.sql   -- 版本2：增量变更
  └── 003_xxx.sql              -- 版本3：增量变更
```

#### 迁移脚本模板

```sql
-- 迁移版本：{version}
-- 描述：{description}
-- 前置版本：{prev_version}
-- 创建日期：{date}

-- 在事务中执行所有变更，确保原子性
BEGIN TRANSACTION;

-- ========== 变更内容开始 ==========

-- 示例：添加新列
-- ALTER TABLE sessions ADD COLUMN new_column TEXT DEFAULT '';

-- ========== 变更内容结束 ==========

-- 记录迁移版本
INSERT INTO schema_version (version, description)
VALUES ({version}, '{description}');

COMMIT;
```

### 5.3 迁移执行流程

```
应用启动
  │
  ├─ 打开数据库连接
  │
  ├─ 检查 schema_version 表是否存在
  │   ├─ 不存在 → 执行 001_initial.sql（初始建表）
  │   └─ 存在 → 读取当前版本号 current_version
  │
  ├─ 扫描 resources/migrations/ 目录
  │   └─ 筛选版本号 > current_version 的迁移脚本
  │
  ├─ 按版本号升序排列，逐个执行
  │   ├─ 每个脚本在独立事务中执行
  │   ├─ 执行成功 → 写入 schema_version 记录
  │   └─ 执行失败 → 回滚事务，终止启动并提示用户
  │
  └─ 全部完成 → 数据库就绪
```

### 5.4 JSON 配置文件迁移

JSON 配置文件采用**合并策略**进行版本升级：

1. **读取默认配置**：从应用资源目录读取当前版本的默认配置模板
2. **读取用户配置**：从用户数据目录读取现有配置
3. **深度合并**：以默认配置为基准，用用户配置覆盖已有字段
4. **新增字段**：默认配置中的新字段自动写入用户配置
5. **废弃字段**：保留但不使用，不主动删除（避免数据丢失）
6. **写回文件**：合并后的配置写回用户数据目录

#### 合并策略伪代码

```typescript
function mergeConfig(defaultConfig: object, userConfig: object): object {
  const result = { ...defaultConfig };
  for (const key of Object.keys(userConfig)) {
    if (key in result && typeof result[key] === 'object' && typeof userConfig[key] === 'object'
        && !Array.isArray(result[key]) && !Array.isArray(userConfig[key])) {
      result[key] = mergeConfig(result[key], userConfig[key]);
    } else {
      result[key] = userConfig[key];
    }
  }
  return result;
}
```

### 5.5 回滚策略

| 场景 | 回滚方式 |
|------|----------|
| 迁移脚本执行失败 | 事务自动回滚，数据库保持在迁移前版本 |
| 迁移后发现问题 | 从自动备份恢复（见 5.6） |
| JSON 配置合并异常 | 保留 `.bak` 备份文件，手动恢复 |

### 5.6 备份策略

- **迁移前自动备份**：每次执行迁移前，将 `docagent.db` 复制为 `docagent.db.bak.{version}`
- **配置文件备份**：每次合并前，将原配置文件复制为 `{filename}.bak`
- **备份清理**：保留最近 5 个数据库备份，超出自动删除最旧的
- **用户手动备份**：应用设置中提供"导出数据"功能，打包所有数据文件

### 5.7 迁移示例

假设 v1.1.0 需要为 `sessions` 表添加 `tags` 字段：

**文件**：`resources/migrations/002_add_session_tags.sql`

```sql
-- 迁移版本：2
-- 描述：为 sessions 表添加 tags 字段
-- 前置版本：1
-- 创建日期：2026-06-01

BEGIN TRANSACTION;

ALTER TABLE sessions ADD COLUMN tags TEXT DEFAULT '[]';

INSERT INTO schema_version (version, description)
VALUES (2, '为 sessions 表添加 tags 字段');

COMMIT;
```

---

## 6. 附录

### 6.1 完整建表脚本

以下为应用首次启动时执行的完整建表脚本（即 `001_initial.sql`）：

```sql
-- DocAgent 数据库初始化脚本
-- 版本：1
-- 描述：初始建表

-- schema_version 元数据表
CREATE TABLE IF NOT EXISTS schema_version (
    version     INTEGER NOT NULL PRIMARY KEY,
    description TEXT    NOT NULL,
    applied_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- sessions 会话表
CREATE TABLE IF NOT EXISTS sessions (
    id                TEXT        NOT NULL PRIMARY KEY,
    workspace_id      TEXT        NOT NULL,
    title             TEXT        NOT NULL DEFAULT '新会话',
    created_at        TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at        TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    total_input_tokens  INTEGER   NOT NULL DEFAULT 0,
    total_output_tokens INTEGER   NOT NULL DEFAULT 0,
    llm_provider      TEXT        NOT NULL DEFAULT '',
    llm_model         TEXT        NOT NULL DEFAULT ''
);

-- session_messages 消息表
CREATE TABLE IF NOT EXISTS session_messages (
    id                TEXT        NOT NULL PRIMARY KEY,
    session_id        TEXT        NOT NULL,
    role              TEXT        NOT NULL CHECK (role IN ('user', 'assistant', 'tool')),
    content           TEXT        NOT NULL DEFAULT '',
    tool_name         TEXT        DEFAULT NULL,
    tool_args         TEXT        DEFAULT NULL,
    tool_result       TEXT        DEFAULT NULL,
    thinking_content  TEXT        DEFAULT NULL,
    input_tokens      INTEGER     NOT NULL DEFAULT 0,
    output_tokens     INTEGER     NOT NULL DEFAULT 0,
    created_at        TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- version_snapshots 版本快照表
CREATE TABLE IF NOT EXISTS version_snapshots (
    id                TEXT        NOT NULL PRIMARY KEY,
    workspace_id      TEXT        NOT NULL,
    session_id        TEXT        NOT NULL,
    file_path         TEXT        NOT NULL,
    snapshot_path     TEXT        NOT NULL,
    operation         TEXT        NOT NULL DEFAULT '',
    created_at        TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- token_usage Token统计表
CREATE TABLE IF NOT EXISTS token_usage (
    id                TEXT        NOT NULL PRIMARY KEY,
    session_id        TEXT        NOT NULL,
    workspace_id      TEXT        NOT NULL,
    llm_provider      TEXT        NOT NULL DEFAULT '',
    llm_model         TEXT        NOT NULL DEFAULT '',
    input_tokens      INTEGER     NOT NULL DEFAULT 0,
    output_tokens     INTEGER     NOT NULL DEFAULT 0,
    created_at        TEXT        NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- 索引创建
CREATE INDEX IF NOT EXISTS idx_sessions_workspace_id
    ON sessions (workspace_id);
CREATE INDEX IF NOT EXISTS idx_sessions_updated_at
    ON sessions (updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_sessions_created_at
    ON sessions (created_at DESC);

CREATE INDEX IF NOT EXISTS idx_session_messages_session_id
    ON session_messages (session_id);
CREATE INDEX IF NOT EXISTS idx_session_messages_session_id_created_at
    ON session_messages (session_id, created_at ASC);
CREATE INDEX IF NOT EXISTS idx_session_messages_role
    ON session_messages (role);

CREATE INDEX IF NOT EXISTS idx_version_snapshots_workspace_id
    ON version_snapshots (workspace_id);
CREATE INDEX IF NOT EXISTS idx_version_snapshots_session_id
    ON version_snapshots (session_id);
CREATE INDEX IF NOT EXISTS idx_version_snapshots_file_path
    ON version_snapshots (file_path);
CREATE INDEX IF NOT EXISTS idx_version_snapshots_created_at
    ON version_snapshots (created_at DESC);

CREATE INDEX IF NOT EXISTS idx_token_usage_session_id
    ON token_usage (session_id);
CREATE INDEX IF NOT EXISTS idx_token_usage_workspace_id
    ON token_usage (workspace_id);
CREATE INDEX IF NOT EXISTS idx_token_usage_created_at
    ON token_usage (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_token_usage_workspace_created
    ON token_usage (workspace_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_token_usage_provider_model
    ON token_usage (llm_provider, llm_model);

-- 记录初始版本
INSERT INTO schema_version (version, description)
VALUES (1, '初始建表：sessions, session_messages, version_snapshots, token_usage');
```

### 6.2 数据类型映射

| 逻辑类型 | SQLite 类型 | 说明 |
|----------|-------------|------|
| UUID | TEXT | UUID v4 字符串，由应用层生成 |
| 时间戳 | TEXT | ISO 8601 格式（`YYYY-MM-DDTHH:MM:SS.sssZ`） |
| 整数 | INTEGER | SQLite 原生整数类型 |
| 文本 | TEXT | SQLite 原生文本类型 |
| JSON | TEXT | JSON 序列化后存储为 TEXT，应用层解析 |
| 布尔 | INTEGER | 0 = false，1 = true |

### 6.3 命名规范

| 类别 | 规范 | 示例 |
|------|------|------|
| 表名 | 小写蛇形命名，复数形式 | `sessions`, `session_messages` |
| 字段名 | 小写蛇形命名 | `workspace_id`, `created_at` |
| 索引名 | `idx_{表名}_{字段名}` | `idx_sessions_workspace_id` |
| 主键 | `id` | 所有表统一使用 `id` 作为主键名 |
| 外键字段 | `{关联表单数}_id` | `session_id`, `workspace_id` |
| 时间字段 | `{动作}_at` | `created_at`, `updated_at` |
