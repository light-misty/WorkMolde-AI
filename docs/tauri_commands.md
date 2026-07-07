# DocAgent Tauri 命令接口文档

> 项目：DocAgent AI文档处理桌面应用
> 技术栈：Tauri 2 + Rust后端 + React前端
> 版本：0.1.6
> 最后更新：2026-06-14

---

## 目录

- [1. 概述](#1-概述)
- [2. 通用约定](#2-通用约定)
- [3. LLM相关命令](#3-llm相关命令)
- [4. Agent相关命令](#4-agent相关命令)
- [5. 会话相关命令](#5-会话相关命令)
- [6. 工作区相关命令](#6-工作区相关命令)
- [7. 文档相关命令](#7-文档相关命令)
- [8. Handler/Tool相关命令](#8-handlertool相关命令)
- [9. 设置相关命令](#9-设置相关命令)
- [10. 模板相关命令](#10-模板相关命令)
- [11. 日志相关命令](#11-日志相关命令)
- [12. 更新相关命令](#12-更新相关命令)
- [13. Tauri事件定义](#13-tauri事件定义)
- [14. Python Sidecar通信协议](#14-python-sidecar通信协议)
- [15. 错误码定义](#15-错误码定义)

---

## 1. 概述

本文档定义了 DocAgent 桌面应用中所有 Tauri 命令、事件、Python Sidecar 通信协议及错误码的完整接口规范。

- **Rust后端**：通过 `#[tauri::command]` 暴露 **48 个命令**（跨 10 个模块），供前端通过 `invoke` 调用
- **React前端**：通过 `@tauri-apps/api/core` 的 `invoke` 调用后端命令，通过 `@tauri-apps/api/event` 的 `listen` 监听后端事件
- **Python Sidecar**：通过 stdin/stdout JSON 通信，负责文档处理

---

## 2. 通用约定

### 2.1 前端调用方式

```typescript
import { invoke } from "@tauri-apps/api/core";

const result = await invoke<ReturnType>("command_name", {
  param1: value1,
  param2: value2,
});
```

### 2.2 事件监听方式

```typescript
import { listen } from "@tauri-apps/api/event";

const unlisten = await listen<PayloadType>("event:name", (event) => {
  console.log(event.payload);
});
unlisten();
```

### 2.3 统一错误响应

```typescript
interface CommandError {
  code: number;      // 错误码，参见第15节
  message: string;   // 人类可读的错误描述
}
```

### 2.4 命名约定

- Rust命令函数：`snake_case`
- 前端调用封装：`camelCase`（见 `src/services/tauri.ts`）
- 事件名称：`模块:动作`，如 `agent:thinking`
- Rust Payload：`#[serde(rename_all = "camelCase")]`

---

## 3. LLM相关命令

> 源文件：`commands/llm.rs`（10个命令）

### 3.1 test_connection

测试指定LLM Provider的连接可用性。

```rust
#[tauri::command]
async fn test_connection(provider_id: String, state: State<'_, AppState>) -> Result<ConnectionResult, CommandError>
```

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| provider_id | String | 是 | Provider唯一标识 |

```typescript
const result = await invoke<ConnectionResult>("test_connection", { providerId: "openai-main" });
// { success: bool, latencyMs: u64, modelInfo?: ModelInfo, errorMessage?: string }
```

### 3.2 test_connection_with_config

使用提供的配置测试连接（不保存）。

```rust
#[tauri::command]
async fn test_connection_with_config(config: ProviderConfig, provider_id: Option<String>, state: State<'_, AppState>) -> Result<ConnectionResult, CommandError>
```

### 3.3 list_providers

列出所有已配置的LLM Provider。

```rust
#[tauri::command]
async fn list_providers(state: State<'_, AppState>) -> Result<Vec<ProviderInfo>, CommandError>
```

```typescript
const providers = await invoke<ProviderInfo[]>("list_providers");
```

### 3.4 add_provider

添加新的LLM Provider配置。

```rust
#[tauri::command]
async fn add_provider(config: ProviderConfig, state: State<'_, AppState>) -> Result<(), CommandError>
```

### 3.5 update_provider

更新已有的Provider配置。

```rust
#[tauri::command]
async fn update_provider(provider_id: String, config: ProviderConfig, state: State<'_, AppState>) -> Result<(), CommandError>
```

### 3.6 delete_provider

删除指定Provider配置。

```rust
#[tauri::command]
async fn delete_provider(provider_id: String, state: State<'_, AppState>) -> Result<(), CommandError>
```

### 3.7 set_default_provider

设置默认LLM Provider。

```rust
#[tauri::command]
async fn set_default_provider(provider_id: String, state: State<'_, AppState>) -> Result<(), CommandError>
```

### 3.8 health_check_providers

对所有Provider执行健康检查。

```rust
#[tauri::command]
async fn health_check_providers(state: State<'_, AppState>) -> Result<HashMap<String, ConnectionResult>, CommandError>
```

### 3.9 force_recover_providers

强制恢复所有被标记为不可用的Provider。

```rust
#[tauri::command]
async fn force_recover_providers(state: State<'_, AppState>) -> Result<(), CommandError>
```

### 3.10 get_network_status

获取当前网络状态。

```rust
#[tauri::command]
async fn get_network_status(state: State<'_, AppState>) -> Result<String, CommandError>
```

### ProviderConfig 类型

```rust
struct ProviderConfig {
    id: Option<String>,
    name: String,
    provider_type: String,                // "openai" | "anthropic" | "ollama" | "gemini" | "custom"
    api_base: String,
    api_key: String,
    model: String,
    is_default: bool,
    context_window: Option<u32>,          // 上下文窗口大小
    supports_vision: Option<bool>,        // 是否支持图片多模态
    extra_params: Option<HashMap<String, Value>>,
    temperature: Option<f64>,
    max_tokens: Option<u32>,
    top_p: Option<f64>,
}
```

---

## 4. Agent相关命令

> 源文件：`commands/agent.rs`（5个命令）

### 4.1 start_agent

启动Agent执行任务，通过事件流式推送结果。

```rust
#[tauri::command]
async fn start_agent(session_id: String, prompt: String, options: Option<Value>, app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), CommandError>
```

| 参数 | 类型 | 说明 |
|------|------|------|
| session_id | String | 关联的会话ID |
| prompt | String | 用户输入的任务描述 |
| options | Option\<Value\> | Agent执行选项（JSON对象） |

options 支持字段：`provider_id`, `max_iterations`(默认20), `auto_confirm`

```typescript
await invoke("start_agent", {
  sessionId: "sess-xxx",
  prompt: "请生成一份文档",
  options: { providerId: "openai-main", maxIterations: 30 },
});
```

### 4.2 stop_agent

中断正在执行的Agent任务。

```rust
#[tauri::command]
async fn stop_agent(session_id: String, state: State<'_, AppState>) -> Result<(), CommandError>
```

### 4.3 confirm_operation

确认或拒绝Agent请求的操作。

```rust
#[tauri::command]
async fn confirm_operation(session_id: String, operation_id: String, approved: bool, feedback: Option<String>, state: State<'_, AppState>) -> Result<(), CommandError>
```

### 4.4 get_context_usage

获取当前会话的Token用量信息。

```rust
#[tauri::command]
async fn get_context_usage(session_id: String, state: State<'_, AppState>) -> Result<ContextUsageInfo, CommandError>
```

### 4.5 is_agent_running

检查指定会话的Agent是否正在运行。

```rust
#[tauri::command]
async fn is_agent_running(session_id: String, state: State<'_, AppState>) -> Result<bool, CommandError>
```

---

## 5. 会话相关命令

> 源文件：`commands/session.rs`（6个命令）

### 5.1 create_session

```rust
#[tauri::command]
async fn create_session(params: CreateSessionParams, app_handle: AppHandle, state: State<'_, AppState>) -> Result<Session, CommandError>
```

### 5.2 list_sessions

```rust
#[tauri::command]
async fn list_sessions(filter: Option<SessionFilter>, state: State<'_, AppState>) -> Result<Vec<SessionSummary>, CommandError>
```

### 5.3 get_session

```rust
#[tauri::command]
async fn get_session(session_id: String, state: State<'_, AppState>) -> Result<SessionDetail, CommandError>
```

### 5.4 delete_session

```rust
#[tauri::command]
async fn delete_session(session_id: String, app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), CommandError>
```

### 5.5 update_session_title

```rust
#[tauri::command]
async fn update_session_title(session_id: String, title: String, app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), CommandError>
```

### 5.6 clear_all_sessions

清除所有会话（返回删除的会话数）。

```rust
#[tauri::command]
async fn clear_all_sessions(app_handle: AppHandle, state: State<'_, AppState>) -> Result<u64, CommandError>
```

### 数据类型

```rust
struct CreateSessionParams {
    title: Option<String>,
    workspace_id: Option<String>,
    provider_id: Option<String>,
    template_id: Option<String>,
}

struct Session {
    id: String,
    title: String,
    workspace_id: Option<String>,
    provider_id: String,
    template_id: Option<String>,
    created_at: String,
    updated_at: String,
    status: String,                   // "active" | "archived"
}

struct SessionFilter {
    status: Option<String>,
    search: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

struct SessionSummary {
    id: String, title: String, status: String, message_count: u32,
    last_message_preview: Option<String>, created_at: String, updated_at: String,
}

struct SessionDetail {
    session: Session,
    messages: Vec<Message>,
}
```

---

## 6. 工作区相关命令

> 源文件：`commands/workspace.rs`（6个命令）

### 6.1 list_workspaces

```rust
#[tauri::command]
async fn list_workspaces(state: State<'_, AppState>) -> Result<Vec<WorkspaceInfo>, CommandError>
```

### 6.2 add_workspace

```rust
#[tauri::command]
async fn add_workspace(path: String, name: Option<String>, app_handle: AppHandle, state: State<'_, AppState>) -> Result<WorkspaceInfo, CommandError>
```

### 6.3 remove_workspace

```rust
#[tauri::command]
async fn remove_workspace(workspace_id: String, app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), CommandError>
```

### 6.4 set_active_workspace

发射 `workspace:change` 事件。

```rust
#[tauri::command]
async fn set_active_workspace(workspace_id: String, state: State<'_, AppState>) -> Result<(), CommandError>
```

### 6.5 get_file_tree

```rust
#[tauri::command]
async fn get_file_tree(workspace_id: String, path: Option<String>, depth: Option<u32>, state: State<'_, AppState>) -> Result<Vec<FileNode>, CommandError>
```

### 6.6 search_files

```rust
#[tauri::command]
async fn search_files(workspace_id: String, query: String, options: Option<SearchOptions>, state: State<'_, AppState>) -> Result<Vec<SearchResult>, CommandError>
```

---

## 7. 文档相关命令

> 源文件：`commands/document.rs`（10个命令）

### 7.1 preview_document

```rust
#[tauri::command]
async fn preview_document(workspace_id: String, path: String, state: State<'_, AppState>) -> Result<PreviewContent, CommandError>
```

### 7.2 get_document_versions

```rust
#[tauri::command]
async fn get_document_versions(workspace_id: String, path: String, state: State<'_, AppState>) -> Result<Vec<VersionInfo>, CommandError>
```

### 7.3 rollback_version

```rust
#[tauri::command]
async fn rollback_version(workspace_id: String, path: String, version_id: String, state: State<'_, AppState>) -> Result<(), CommandError>
```

### 7.4 get_version_content

获取历史版本的内容预览。

```rust
#[tauri::command]
async fn get_version_content(workspace_id: String, path: String, version_id: String, state: State<'_, AppState>) -> Result<PreviewContent, CommandError>
```

### 7.5 create_file

```rust
#[tauri::command]
async fn create_file(workspace_id: String, path: String, state: State<'_, AppState>) -> Result<(), CommandError>
```

### 7.6 create_directory

```rust
#[tauri::command]
async fn create_directory(workspace_id: String, path: String, state: State<'_, AppState>) -> Result<(), CommandError>
```

### 7.7 rename_file

```rust
#[tauri::command]
async fn rename_file(workspace_id: String, old_path: String, new_path: String, state: State<'_, AppState>) -> Result<(), CommandError>
```

### 7.8 delete_file

```rust
#[tauri::command]
async fn delete_file(workspace_id: String, path: String, state: State<'_, AppState>) -> Result<(), CommandError>
```

### 7.9 show_in_file_manager

在系统文件管理器中打开文件所在目录。

```rust
#[tauri::command]
async fn show_in_file_manager(workspace_id: String, path: String, state: State<'_, AppState>) -> Result<(), CommandError>
```

### 7.10 get_pdf_data

获取PDF文件的base64编码数据（用于前端渲染）。

```rust
#[tauri::command]
async fn get_pdf_data(workspace_id: String, path: String, state: State<'_, AppState>) -> Result<String, CommandError>
```

---

## 8. Handler/Tool相关命令

> 源文件：`commands/handler.rs`（2个命令）

### 8.1 list_handlers

列出所有已注册的Handler（5个文档类型内置Handler）。

```rust
#[tauri::command]
async fn list_handlers(state: State<'_, AppState>) -> Result<Vec<HandlerInfo>, CommandError>
```

### 8.2 list_tools

列出所有已注册的Tool（8个文件系统内置Tool）。

```rust
#[tauri::command]
async fn list_tools(state: State<'_, AppState>) -> Result<Vec<ToolInfo>, CommandError>
```

---

## 9. 设置相关命令

> 源文件：`commands/settings.rs`（2个命令）

### 9.1 get_settings

```rust
#[tauri::command]
async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, CommandError>
```

返回的 AppSettings 结构：

```rust
struct AppSettings {
    general: GeneralSettings,           // author_name, author_email, author_company, confirmation_level
    appearance: AppearanceSettings,     // theme_mode, language, language_follow_system
    version_snapshot: VersionSnapshot,   // retention_policy, max_count, max_days
    workspace: WorkspaceDefaults,       // default_workspace_id
    shortcuts: Shortcuts,               // new_session, close_session, send_message, toggle_sidebar, quick_prompt
    update: UpdateSettings,             // auto_check
}
```

### 9.2 update_settings

```rust
#[tauri::command]
async fn update_settings(settings: Value, state: State<'_, AppState>) -> Result<(), CommandError>
```

settings 为 Partial JSON，仅传入需要更新的字段。

```typescript
await invoke("update_settings", {
  settings: { general: { authorName: "新名字" } },
});
```

---

## 10. 模板相关命令

> 源文件：`commands/template.rs`（5个命令）

### 10.1 list_templates

```rust
#[tauri::command]
async fn list_templates(state: State<'_, AppState>) -> Result<Vec<PromptTemplate>, CommandError>
```

### 10.2 get_template

```rust
#[tauri::command]
async fn get_template(template_id: String, state: State<'_, AppState>) -> Result<PromptTemplate, CommandError>
```

### 10.3 create_template

```rust
#[tauri::command]
async fn create_template(params: CreateTemplateParams, state: State<'_, AppState>) -> Result<PromptTemplate, CommandError>
```

### 10.4 update_template

```rust
#[tauri::command]
async fn update_template(template_id: String, params: UpdateTemplateParams, state: State<'_, AppState>) -> Result<PromptTemplate, CommandError>
```

### 10.5 delete_template

```rust
#[tauri::command]
async fn delete_template(template_id: String, state: State<'_, AppState>) -> Result<(), CommandError>
```

---

## 11. 日志相关命令

> 源文件：`commands/log.rs`（2个命令）

### 11.1 get_log_path

```rust
#[tauri::command]
async fn get_log_path(app_handle: AppHandle) -> Result<LogPathInfo, CommandError>
```

### 11.2 get_error_log

```rust
#[tauri::command]
async fn get_error_log(app_handle: AppHandle) -> Result<String, CommandError>
```

---

## 12. 更新相关命令

> 源文件：`commands/update.rs`（3个命令，仅desktop平台）

### 12.1 check_update

```rust
#[tauri::command]
async fn check_update(app: AppHandle) -> Result<Option<UpdateInfo>, CommandError>
```

---

## 13. Tauri事件定义

### 13.1 Agent事件

| 事件名 | 说明 | Payload | 触发时机 |
|--------|------|---------|----------|
| `agent:thinking` | 普通思考链 | `ThinkingPayload` | Agent思考过程 |
| `agent:deep_thinking` | 深度思考链 | `DeepThinkingPayload` | Extended Thinking输出 |
| `agent:content` | 回复内容 | `ContentPayload` | 流式输出文本 |
| `agent:tool_call` | Tool调用 | `ToolCallPayload` | 发起工具调用 |
| `agent:tool_result` | Tool结果 | `ToolResultPayload` | 工具执行完成 |
| `agent:confirm` | 需确认 | `ConfirmPayload` | 高风险操作需确认 |
| `agent:context_update` | Token用量 | `ContextUsagePayload` | Token用量变化 |
| `agent:network_retry` | 网络重试 | `NetworkRetryPayload` | LLM请求重试 |
| `agent:done` | 完成 | `DonePayload` | Agent任务完成 |
| `agent:error` | 错误 | `ErrorPayload` | 执行出错 |
| `agent:stopped` | 中断 | `StoppedPayload` | 被用户中断 |

### 13.2 系统事件

| 事件名 | 说明 | Payload |
|--------|------|---------|
| `session:updated` | 会话变更 | `SessionUpdatePayload` |
| `workspace:change` | 工作区切换 | `WorkspaceChangePayload` |
| `workspace:directory_deleted` | 工作区目录被外部删除 | `WorkspaceDirectoryDeletedPayload` |
| `file:change` | 文件变更 | `FileChangePayload` |
| `llm:provider_switch` | Provider自动切换 | `ProviderSwitchPayload` |
| `system:network_change` | 网络状态变化 | `NetworkChangePayload` |

### 13.3 核心Payload结构

```rust
struct DeepThinkingPayload { session_id, step, thought, is_streaming, iteration? }
struct ContentPayload { session_id, message_id, content, is_streaming, iteration? }
struct ToolCallPayload { session_id, call_id, tool_name, arguments, iteration? }
struct ToolResultPayload { session_id, call_id, success, result, error?, duration_ms }
struct ConfirmPayload { session_id, operation_id, operation_type, description, details, risk_level }
struct NetworkRetryPayload { session_id, attempt, max_attempts, reason }
struct ContextUsagePayload { session_id, context_usage: ContextUsageInfo }
struct ErrorPayload { session_id, code, message, recoverable }
struct StoppedPayload { session_id, completed_steps, reason }
struct FileChangePayload { workspace_id, change_type, path, old_path? }
struct ProviderSwitchPayload { from_provider_id, to_provider_id, reason, is_automatic }
struct NetworkChangePayload { status, previous_status }
```

---

## 14. Python Sidecar通信协议

### 14.1 通信架构

```
Rust后端 ──JSON──> stdin  ┌──────────────┐  stdout ──JSON──> Rust后端
                         │ Python        │
                         │ Sidecar       │
                         └──────────────┘
```

- 消息分隔：每条 JSON 以换行符 `\n` 结尾
- 超时：60秒（文档操作）

### 14.2 请求格式

```json
{
  "id": "uuid-string",
  "action": "read|convert|analyze|ping|validate",
  "type": "docx|xlsx|pptx|pdf|md|txt",
  "params": { "input_path": "...", ... }
}
```

### 14.3 响应格式

```json
{
  "id": "uuid-string",
  "success": true,
  "data": { ... },
  "error": null
}
```

### 14.4 Action详细定义

| Action | 说明 | 适用类型 |
|--------|------|----------|
| `read` | 读取文档内容（文本+元数据） | docx/xlsx/pptx/pdf/md |
| `convert` | 格式转换 | docx/pdf/md（如 docx→pdf） |
| `analyze` | 文档分析（统计+结构） | docx/xlsx/pptx/pdf/md |
| `ping` | 健康检查 | 通用 |
| `validate` | 文档校验 | docx/xlsx/pptx/pdf |

### 14.5 支持的文档类型与操作矩阵

| 类型 | read | convert | analyze |
|------|------|---------|---------|
| docx | 支持 | 支持 | 支持 |
| xlsx | 支持 | 支持 | 支持 |
| pptx | 支持 | 支持 | 支持 |
| pdf | 支持 | 支持 | 支持 |
| md/txt | 支持 | 支持 | 支持 |

---

## 15. 错误码定义

### 15.1 错误码结构

4位数字，按模块分段：

| 范围 | 模块 | 数量 |
|------|------|------|
| 1000-1999 | LLM | 14个 |
| 2000-2999 | Agent | 8个 |
| 3000-3999 | 文档处理 | 12个 |
| 4000-4999 | 数据库 | 7个 |
| 5000-5999 | 配置 | 8个 |
| 6000-6999 | 文件系统 | 8个 |
| 7000-7999 | 运行时 | 1个 |
| 8000-8999 | 更新 | 5个 |
| 9000-9999 | Tool | 4个 |

### 15.2 LLM错误 (1xxx)

| 码 | 常量名 | 说明 |
|----|--------|------|
| 1001 | LLM_CONNECTION_FAILED | 连接LLM服务失败 |
| 1002 | LLM_AUTH_FAILED | API密钥认证失败 |
| 1003 | LLM_RATE_LIMITED | 请求频率超限 |
| 1004 | LLM_QUOTA_EXCEEDED | 配额已用尽 |
| 1005 | LLM_MODEL_NOT_FOUND | 指定模型不存在 |
| 1006 | LLM_TIMEOUT | 请求超时 |
| 1007 | LLM_INVALID_REQUEST | 请求参数无效 |
| 1008 | LLM_STREAM_ERROR | 流式响应中断 |
| 1009 | LLM_PROVIDER_UNAVAILABLE | Provider不可用 |
| 1010 | LLM_RESPONSE_PARSE_ERROR | 响应解析失败 |
| 1011 | LLM_DNS_RESOLVE_FAILED | DNS解析失败 |
| 1012 | LLM_CONNECTION_REFUSED | 连接被拒绝 |
| 1013 | LLM_SSL_ERROR | SSL/TLS握手失败 |
| 1014 | LLM_NETWORK_UNREACHABLE | 网络不可达 |

### 15.3 Agent错误 (2xxx)

| 码 | 常量名 | 说明 |
|----|--------|------|
| 2001 | AGENT_ALREADY_RUNNING | Agent已在运行 |
| 2002 | AGENT_NOT_RUNNING | Agent未在运行 |
| 2003 | AGENT_MAX_ITERATIONS | 达到最大迭代次数 |
| 2004 | AGENT_CONFIRMATION_TIMEOUT | 确认操作超时 |
| 2005 | AGENT_OPERATION_REJECTED | 操作被用户拒绝 |
| 2006 | AGENT_HANDLER_NOT_FOUND | Handler不存在 |
| 2008 | AGENT_EXECUTION_ERROR | Agent执行内部错误 |
| 2010 | AGENT_SESSION_NOT_FOUND | 会话不存在 |

### 15.4 文档处理错误 (3xxx)

| 码 | 常量名 | 说明 |
|----|--------|------|
| 3001 | DOC_FILE_NOT_FOUND | 文件不存在 |
| 3002 | DOC_FORMAT_UNSUPPORTED | 不支持的格式 |
| 3003 | DOC_PARSE_ERROR | 解析失败 |
| 3004 | DOC_WRITE_ERROR | 写入失败 |
| 3005 | DOC_CONVERT_ERROR | 转换失败 |
| 3006 | DOC_TEMPLATE_NOT_FOUND | 模板不存在 |
| 3007 | DOC_TEMPLATE_ERROR | 模板渲染失败 |
| 3008 | DOC_VERSION_NOT_FOUND | 版本记录不存在 |
| 3009 | DOC_ROLLBACK_FAILED | 回滚失败 |
| 3010 | DOC_SIDECAR_ERROR | Sidecar通信错误 |
| 3011 | DOC_PERMISSION_DENIED | 权限不足 |
| 3012 | DOC_FILE_TOO_LARGE | 文件过大 |

### 15.5 数据库错误 (4xxx)

| 码 | 常量名 | 说明 |
|----|--------|------|
| 4001 | DB_CONNECTION_FAILED | 连接失败 |
| 4002 | DB_QUERY_FAILED | 查询失败 |
| 4003 | DB_RECORD_NOT_FOUND | 记录不存在 |
| 4004 | DB_RECORD_EXISTS | 记录已存在 |
| 4005 | DB_CONSTRAINT_VIOLATION | 约束冲突 |
| 4006 | DB_MIGRATION_FAILED | 迁移失败 |
| 4007 | DB_CORRUPTED | 数据库损坏 |

### 15.6 配置错误 (5xxx)

| 码 | 常量名 | 说明 |
|----|--------|------|
| 5001 | CONFIG_INVALID_FORMAT | 配置格式无效 |
| 5002 | CONFIG_MISSING_FIELD | 缺少必填配置项 |
| 5003 | CONFIG_INVALID_VALUE | 配置值无效 |
| 5004 | CONFIG_IMPORT_FAILED | 导入失败 |
| 5005 | CONFIG_EXPORT_FAILED | 导出失败 |
| 5006 | CONFIG_PROVIDER_NOT_FOUND | Provider不存在 |
| 5007 | CONFIG_DEFAULT_PROVIDER_REQUIRED | 需要设置默认Provider |
| 5008 | CONFIG_WORKSPACE_PATH_EXISTS | 工作区路径已存在 |

### 15.7 文件系统错误 (6xxx)

| 码 | 常量名 | 说明 |
|----|--------|------|
| 6001 | FS_PATH_NOT_FOUND | 路径不存在 |
| 6002 | FS_PERMISSION_DENIED | 权限不足 |
| 6003 | FS_ALREADY_EXISTS | 文件/目录已存在 |
| 6004 | FS_NOT_A_DIRECTORY | 路径不是目录 |
| 6005 | FS_DISK_FULL | 磁盘空间不足 |
| 6006 | FS_IO_ERROR | IO读写错误 |
| 6007 | FS_WATCH_ERROR | 文件监听失败 |
| 6008 | FS_ENCODING_ERROR | 文件编码错误 |

### 15.8 运行时错误 (7xxx)

| 码 | 常量名 | 说明 |
|----|--------|------|
| 7001 | RUNTIME_EVENT_EMIT_ERROR | 事件发射错误 |

### 15.9 更新错误 (8xxx)

| 码 | 常量名 | 说明 |
|----|--------|------|
| 8001 | UPDATE_CHECK_FAILED | 更新检查失败 |
| 8002 | UPDATE_DOWNLOAD_FAILED | 下载失败 |
| 8003 | UPDATE_INSTALL_FAILED | 安装失败 |
| 8004 | UPDATE_NO_UPDATE_AVAILABLE | 没有可用更新 |

### 15.10 Tool错误 (9xxx)

| 码 | 常量名 | 说明 |
|----|--------|------|
| 9001 | TOOL_NOT_FOUND | 工具不存在 |
| 9002 | TOOL_INVALID_PARAMS | 参数无效 |
| 9003 | TOOL_EXECUTION_ERROR | 执行失败 |
| 9004 | TOOL_PATH_OUT_OF_BOUNDS | 路径越界 |

---

## 附录：命令数量汇总

| 模块 | 命令数 | 说明 |
|------|--------|------|
| llm.rs | 10 | Provider管理+测试+健康检查 |
| agent.rs | 5 | 启动/停止/确认/状态查询 |
| session.rs | 6 | 会话CRUD+清空 |
| document.rs | 10 | 文档预览/版本/文件操作 |
| workspace.rs | 6 | 工作区CRUD+文件树+搜索 |
| handler.rs | 2 | Handler/Tool列表查询 |
| settings.rs | 2 | 设置读写 |
| template.rs | 5 | 模板CRUD |
| log.rs | 2 | 日志路径/错误日志 |
| update.rs | 2 | 更新检查/安装(桌面端) |
| **合计** | **48+2** | **10个模块，46+2个(桌面)命令** |
