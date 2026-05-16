# DocAgent Tauri 命令接口文档

> 项目：DocAgent AI文档处理桌面应用
> 技术栈：Tauri 2 + Rust后端 + React前端
> 版本：1.0.0

---

## 目录

- [1. 概述](#1-概述)
- [2. 通用约定](#2-通用约定)
- [3. LLM相关命令](#3-llm相关命令)
- [4. Agent相关命令](#4-agent相关命令)
- [5. 会话相关命令](#5-会话相关命令)
- [6. 工作区相关命令](#6-工作区相关命令)
- [7. 文档相关命令](#7-文档相关命令)
- [8. Skill相关命令](#8-skill相关命令)
- [9. 设置相关命令](#9-设置相关命令)
- [10. 模板相关命令](#10-模板相关命令)
- [11. Token相关命令](#11-token相关命令)
- [12. Tauri事件定义](#12-tauri事件定义)
- [13. Python Sidecar通信协议](#13-python-sidecar通信协议)
- [14. 错误码定义](#14-错误码定义)

---

## 1. 概述

本文档定义了 DocAgent 桌面应用中所有 Tauri 命令（Command）、事件（Event）、Python Sidecar 通信协议及错误码的完整接口规范。

- **Rust后端**：通过 `#[tauri::command]` 宏暴露命令，供前端通过 `invoke` 调用。
- **React前端**：通过 `@tauri-apps/api/core` 的 `invoke` 函数调用后端命令，通过 `@tauri-apps/api/event` 的 `listen` 函数监听后端事件。
- **Python Sidecar**：通过 stdin/stdout JSON 协议与 Rust 后端通信，负责文档处理的核心逻辑。

---

## 2. 通用约定

### 2.1 前端调用方式

```typescript
import { invoke } from "@tauri-apps/api/core";

// 通用调用模板
const result = await invoke<ReturnType>("command_name", {
  param1: value1,
  param2: value2,
});
```

### 2.2 事件监听方式

```typescript
import { listen } from "@tauri-apps/api/event";

const unlisten = await listen<EventType>("event:name", (event) => {
  console.log(event.payload);
});

// 取消监听
unlisten();
```

### 2.3 统一错误响应

所有返回 `Result<T>` 的命令，在错误时前端会收到 `Error` 对象：

```typescript
interface CommandError {
  code: number;      // 错误码，参见第14节
  message: string;   // 人类可读的错误描述
}
```

### 2.4 命名约定

- Rust命令函数：`snake_case`
- 前端调用名称：与Rust函数名一致，使用 `snake_case`
- 事件名称：`模块:动作`，如 `agent:thinking`
- 类型名称：`PascalCase`

---

## 3. LLM相关命令

> 源文件：`commands/llm.rs`

### 3.1 test_connection

测试指定LLM Provider的连接是否可用。

**Rust签名：**

```rust
#[tauri::command]
async fn test_connection(provider_id: String) -> Result<ConnectionResult, CommandError>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| provider_id | String | 是 | Provider的唯一标识 |

**返回类型：**

```rust
struct ConnectionResult {
    success: bool,
    latency_ms: u64,
    model_info: Option<ModelInfo>,
    error_message: Option<String>,
}

struct ModelInfo {
    model_name: String,
    max_tokens: u32,
    supports_streaming: bool,
    supports_tool_call: bool,
}
```

**前端调用：**

```typescript
const result = await invoke<ConnectionResult>("test_connection", {
  providerId: "openai-main",
});
```

---

### 3.2 list_providers

列出所有已配置的LLM Provider。

**Rust签名：**

```rust
#[tauri::command]
async fn list_providers() -> Vec<ProviderInfo>
```

**参数：** 无

**返回类型：**

```rust
struct ProviderInfo {
    id: String,
    name: String,
    provider_type: String,       // "openai" | "anthropic" | "ollama" | "custom"
    api_base: String,
    model: String,
    is_default: bool,
    is_available: bool,
    created_at: String,          // ISO 8601
}
```

**前端调用：**

```typescript
const providers = await invoke<ProviderInfo[]>("list_providers");
```

---

### 3.3 add_provider

添加一个新的LLM Provider配置。

**Rust签名：**

```rust
#[tauri::command]
async fn add_provider(config: ProviderConfig) -> Result<(), CommandError>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| config | ProviderConfig | 是 | Provider配置信息 |

```rust
struct ProviderConfig {
    name: String,
    provider_type: String,       // "openai" | "anthropic" | "ollama" | "custom"
    api_base: String,
    api_key: String,             // 加密存储
    model: String,
    extra_params: Option<HashMap<String, serde_json::Value>>,
}
```

**返回类型：** `Result<(), CommandError>`

**前端调用：**

```typescript
await invoke("add_provider", {
  config: {
    name: "My OpenAI",
    providerType: "openai",
    apiBase: "https://api.openai.com/v1",
    apiKey: "sk-xxx",
    model: "gpt-4o",
    extraParams: null,
  },
});
```

---

### 3.4 update_provider

更新已有的Provider配置。

**Rust签名：**

```rust
#[tauri::command]
async fn update_provider(provider_id: String, config: ProviderConfig) -> Result<(), CommandError>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| provider_id | String | 是 | 要更新的Provider标识 |
| config | ProviderConfig | 是 | 新的Provider配置 |

**返回类型：** `Result<(), CommandError>`

**前端调用：**

```typescript
await invoke("update_provider", {
  providerId: "openai-main",
  config: { /* ... */ },
});
```

---

### 3.5 delete_provider

删除指定的Provider配置。

**Rust签名：**

```rust
#[tauri::command]
async fn delete_provider(provider_id: String) -> Result<(), CommandError>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| provider_id | String | 是 | 要删除的Provider标识 |

**返回类型：** `Result<(), CommandError>`

**前端调用：**

```typescript
await invoke("delete_provider", { providerId: "openai-main" });
```

---

### 3.6 set_default_provider

设置默认的LLM Provider。

**Rust签名：**

```rust
#[tauri::command]
async fn set_default_provider(provider_id: String) -> Result<(), CommandError>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| provider_id | String | 是 | 设为默认的Provider标识 |

**返回类型：** `Result<(), CommandError>`

**前端调用：**

```typescript
await invoke("set_default_provider", { providerId: "openai-main" });
```

---

## 4. Agent相关命令

> 源文件：`commands/agent.rs`

### 4.1 start_agent

启动Agent执行任务。Agent通过事件（Event）异步推送执行过程和结果。

**Rust签名：**

```rust
#[tauri::command]
async fn start_agent(session_id: String, prompt: String, options: Option<AgentOptions>) -> Result<(), CommandError>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| session_id | String | 是 | 关联的会话ID |
| prompt | String | 是 | 用户输入的任务描述 |
| options | Option\<AgentOptions\> | 否 | Agent执行选项 |

```rust
struct AgentOptions {
    provider_id: Option<String>,       // 指定使用的Provider，不填则使用默认
    max_iterations: Option<u32>,       // 最大迭代次数，默认20
    auto_confirm: Option<bool>,        // 是否自动确认操作，默认false
    skills: Option<Vec<String>>,       // 启用的Skill列表
    working_directory: Option<String>, // 工作目录
}
```

**返回类型：** `Result<(), CommandError>`

执行结果通过以下事件推送：
- `agent:thinking` - Agent思考中
- `agent:content` - Agent输出内容
- `agent:tool_call` - Agent调用工具
- `agent:tool_result` - 工具执行结果
- `agent:confirm` - 需要用户确认
- `agent:todo_update` - 任务进度更新
- `agent:done` - 执行完成
- `agent:error` - 执行出错
- `agent:stopped` - 被用户中断

**前端调用：**

```typescript
await invoke("start_agent", {
  sessionId: "sess-xxx",
  prompt: "请将这份Word文档转换为PDF格式",
  options: {
    providerId: "openai-main",
    maxIterations: 30,
    autoConfirm: false,
    skills: ["document-converter"],
    workingDirectory: "C:\\Users\\docs",
  },
});
```

---

### 4.2 stop_agent

中断正在执行的Agent任务。

**Rust签名：**

```rust
#[tauri::command]
async fn stop_agent(session_id: String) -> Result<(), CommandError>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| session_id | String | 是 | 要中断的会话ID |

**返回类型：** `Result<(), CommandError>`

中断成功后，前端将收到 `agent:stopped` 事件。

**前端调用：**

```typescript
await invoke("stop_agent", { sessionId: "sess-xxx" });
```

---

### 4.3 confirm_operation

确认Agent请求的操作。当Agent执行需要用户确认的操作时（`agent:confirm`事件），前端调用此命令进行确认或拒绝。

**Rust签名：**

```rust
#[tauri::command]
async fn confirm_operation(session_id: String, operation_id: String, approved: bool, feedback: Option<String>) -> Result<(), CommandError>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| session_id | String | 是 | 会话ID |
| operation_id | String | 是 | 操作ID，来自`agent:confirm`事件 |
| approved | bool | 是 | true=批准，false=拒绝 |
| feedback | Option\<String\> | 否 | 用户的附加反馈信息 |

**返回类型：** `Result<(), CommandError>`

**前端调用：**

```typescript
await invoke("confirm_operation", {
  sessionId: "sess-xxx",
  operationId: "op-123",
  approved: true,
  feedback: "请确保保留原始格式",
});
```

---

## 5. 会话相关命令

> 源文件：`commands/session.rs`

### 5.1 create_session

创建新的对话会话。

**Rust签名：**

```rust
#[tauri::command]
async fn create_session(params: CreateSessionParams) -> Result<Session, CommandError>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| params | CreateSessionParams | 是 | 创建会话的参数 |

```rust
struct CreateSessionParams {
    title: Option<String>,              // 会话标题，不填则自动生成
    workspace_id: Option<String>,       // 关联的工作区ID
    provider_id: Option<String>,        // 使用的Provider
    template_id: Option<String>,        // 关联的模板ID
}

struct Session {
    id: String,
    title: String,
    workspace_id: Option<String>,
    provider_id: String,
    template_id: Option<String>,
    created_at: String,                 // ISO 8601
    updated_at: String,                 // ISO 8601
    status: String,                     // "active" | "archived"
}
```

**返回类型：** `Result<Session, CommandError>`

**前端调用：**

```typescript
const session = await invoke<Session>("create_session", {
  params: {
    title: "文档处理任务",
    workspaceId: "ws-001",
    providerId: "openai-main",
    templateId: null,
  },
});
```

---

### 5.2 list_sessions

列出所有会话的摘要信息。

**Rust签名：**

```rust
#[tauri::command]
async fn list_sessions(filter: Option<SessionFilter>) -> Vec<SessionSummary>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| filter | Option\<SessionFilter\> | 否 | 筛选条件 |

```rust
struct SessionFilter {
    workspace_id: Option<String>,
    status: Option<String>,             // "active" | "archived"
    search: Option<String>,             // 按标题搜索
    limit: Option<u32>,                 // 返回数量限制，默认50
    offset: Option<u32>,                // 偏移量，默认0
}

struct SessionSummary {
    id: String,
    title: String,
    status: String,
    message_count: u32,
    last_message_preview: Option<String>,
    created_at: String,
    updated_at: String,
}
```

**返回类型：** `Vec<SessionSummary>`

**前端调用：**

```typescript
const sessions = await invoke<SessionSummary[]>("list_sessions", {
  filter: { status: "active", limit: 20 },
});
```

---

### 5.3 get_session

获取会话的完整详情，包括消息历史。

**Rust签名：**

```rust
#[tauri::command]
async fn get_session(session_id: String) -> Result<SessionDetail, CommandError>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| session_id | String | 是 | 会话ID |

```rust
struct SessionDetail {
    session: Session,
    messages: Vec<Message>,
    token_usage: TokenUsage,
}

struct Message {
    id: String,
    role: String,                       // "user" | "assistant" | "system"
    content: String,
    tool_calls: Option<Vec<ToolCall>>,
    created_at: String,
}

struct ToolCall {
    id: String,
    name: String,
    arguments: serde_json::Value,
    result: Option<serde_json::Value>,
}

struct TokenUsage {
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
}
```

**返回类型：** `Result<SessionDetail, CommandError>`

**前端调用：**

```typescript
const detail = await invoke<SessionDetail>("get_session", {
  sessionId: "sess-xxx",
});
```

---

### 5.4 delete_session

删除指定会话及其所有消息。

**Rust签名：**

```rust
#[tauri::command]
async fn delete_session(session_id: String) -> Result<(), CommandError>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| session_id | String | 是 | 要删除的会话ID |

**返回类型：** `Result<(), CommandError>`

**前端调用：**

```typescript
await invoke("delete_session", { sessionId: "sess-xxx" });
```

---

### 5.5 update_session_title

更新会话标题。

**Rust签名：**

```rust
#[tauri::command]
async fn update_session_title(session_id: String, title: String) -> Result<(), CommandError>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| session_id | String | 是 | 会话ID |
| title | String | 是 | 新标题 |

**返回类型：** `Result<(), CommandError>`

**前端调用：**

```typescript
await invoke("update_session_title", {
  sessionId: "sess-xxx",
  title: "新的会话标题",
});
```

---

## 6. 工作区相关命令

> 源文件：`commands/workspace.rs`

### 6.1 list_workspaces

列出所有已配置的工作区。

**Rust签名：**

```rust
#[tauri::command]
async fn list_workspaces() -> Vec<WorkspaceInfo>
```

**参数：** 无

**返回类型：**

```rust
struct WorkspaceInfo {
    id: String,
    name: String,
    path: String,                       // 工作区根路径
    is_active: bool,                    // 是否为当前活动工作区
    file_count: u32,                    // 文件数量
    created_at: String,
    last_accessed: String,
}
```

**前端调用：**

```typescript
const workspaces = await invoke<WorkspaceInfo[]>("list_workspaces");
```

---

### 6.2 add_workspace

添加一个新的工作区目录。

**Rust签名：**

```rust
#[tauri::command]
async fn add_workspace(path: String, name: Option<String>) -> Result<WorkspaceInfo, CommandError>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| path | String | 是 | 工作区目录的绝对路径 |
| name | Option\<String\> | 否 | 工作区名称，不填则使用目录名 |

**返回类型：** `Result<WorkspaceInfo, CommandError>`

**前端调用：**

```typescript
const workspace = await invoke<WorkspaceInfo>("add_workspace", {
  path: "D:\\Projects\\MyDocs",
  name: "我的文档项目",
});
```

---

### 6.3 remove_workspace

移除工作区（仅从应用中移除，不删除实际文件）。

**Rust签名：**

```rust
#[tauri::command]
async fn remove_workspace(workspace_id: String) -> Result<(), CommandError>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| workspace_id | String | 是 | 要移除的工作区ID |

**返回类型：** `Result<(), CommandError>`

**前端调用：**

```typescript
await invoke("remove_workspace", { workspaceId: "ws-001" });
```

---

### 6.4 set_active_workspace

设置当前活动的工作区。

**Rust签名：**

```rust
#[tauri::command]
async fn set_active_workspace(workspace_id: String) -> Result<(), CommandError>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| workspace_id | String | 是 | 设为活动的工作区ID |

**返回类型：** `Result<(), CommandError>`

切换成功后，前端将收到 `workspace:changed` 事件。

**前端调用：**

```typescript
await invoke("set_active_workspace", { workspaceId: "ws-001" });
```

---

### 6.5 get_file_tree

获取指定工作区的文件树结构。

**Rust签名：**

```rust
#[tauri::command]
async fn get_file_tree(workspace_id: String, path: Option<String>, depth: Option<u32>) -> Vec<FileNode>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| workspace_id | String | 是 | 工作区ID |
| path | Option\<String\> | 否 | 相对路径，不填则从根目录开始 |
| depth | Option\<u32\> | 否 | 目录深度限制，默认3 |

```rust
struct FileNode {
    name: String,
    path: String,                       // 相对路径
    is_dir: bool,
    size: Option<u64>,                  // 文件大小（字节）
    modified: Option<String>,           // 最后修改时间
    extension: Option<String>,          // 文件扩展名
    children: Option<Vec<FileNode>>,    // 子节点（仅目录有）
}
```

**返回类型：** `Vec<FileNode>`

**前端调用：**

```typescript
const tree = await invoke<FileNode[]>("get_file_tree", {
  workspaceId: "ws-001",
  path: "docs/chapters",
  depth: 2,
});
```

---

### 6.6 search_files

在工作区中搜索文件。

**Rust签名：**

```rust
#[tauri::command]
async fn search_files(workspace_id: String, query: String, options: Option<SearchOptions>) -> Vec<SearchResult>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| workspace_id | String | 是 | 工作区ID |
| query | String | 是 | 搜索关键词 |
| options | Option\<SearchOptions\> | 否 | 搜索选项 |

```rust
struct SearchOptions {
    extensions: Option<Vec<String>>,    // 限定文件扩展名，如 ["docx", "pdf"]
    max_results: Option<u32>,           // 最大结果数，默认50
    include_content: Option<bool>,      // 是否搜索文件内容，默认false
}

struct SearchResult {
    path: String,                       // 文件相对路径
    name: String,
    extension: String,
    size: u64,
    modified: String,
    match_type: String,                 // "name" | "content"
    match_preview: Option<String>,      // 匹配内容预览
    line_number: Option<u32>,           // 内容匹配时的行号
}
```

**返回类型：** `Vec<SearchResult>`

**前端调用：**

```typescript
const results = await invoke<SearchResult[]>("search_files", {
  workspaceId: "ws-001",
  query: "合同模板",
  options: { extensions: ["docx"], maxResults: 20, includeContent: true },
});
```

---

## 7. 文档相关命令

> 源文件：`commands/document.rs`

### 7.1 preview_document

获取文档的预览内容（纯文本/Markdown格式）。

**Rust签名：**

```rust
#[tauri::command]
async fn preview_document(workspace_id: String, path: String) -> Result<PreviewContent, CommandError>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| workspace_id | String | 是 | 工作区ID |
| path | String | 是 | 文档相对路径 |

```rust
struct PreviewContent {
    path: String,
    file_type: String,                  // "docx" | "xlsx" | "pptx" | "pdf" | "md"
    content: String,                    // 预览文本内容
    page_count: Option<u32>,            // 页数（适用于PDF/DOCX）
    sheet_names: Option<Vec<String>>,   // 工作表名称（适用于XLSX）
    metadata: Option<DocumentMetadata>,
}

struct DocumentMetadata {
    title: Option<String>,
    author: Option<String>,
    created: Option<String>,
    modified: Option<String>,
    word_count: Option<u32>,
}
```

**返回类型：** `Result<PreviewContent, CommandError>`

**前端调用：**

```typescript
const preview = await invoke<PreviewContent>("preview_document", {
  workspaceId: "ws-001",
  path: "reports/年度报告.docx",
});
```

---

### 7.2 get_document_versions

获取文档的版本历史记录。

**Rust签名：**

```rust
#[tauri::command]
async fn get_document_versions(workspace_id: String, path: String) -> Vec<VersionInfo>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| workspace_id | String | 是 | 工作区ID |
| path | String | 是 | 文档相对路径 |

```rust
struct VersionInfo {
    version_id: String,
    path: String,
    timestamp: String,                  // ISO 8601
    operation: String,                  // "create" | "modify" | "convert" | "rollback"
    description: String,                // 操作描述
    size: u64,                          // 文件大小
    session_id: Option<String>,         // 关联的会话ID
}
```

**返回类型：** `Vec<VersionInfo>`

**前端调用：**

```typescript
const versions = await invoke<VersionInfo[]>("get_document_versions", {
  workspaceId: "ws-001",
  path: "reports/年度报告.docx",
});
```

---

### 7.3 rollback_version

将文档回滚到指定的历史版本。

**Rust签名：**

```rust
#[tauri::command]
async fn rollback_version(workspace_id: String, path: String, version_id: String) -> Result<(), CommandError>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| workspace_id | String | 是 | 工作区ID |
| path | String | 是 | 文档相对路径 |
| version_id | String | 是 | 目标版本ID |

**返回类型：** `Result<(), CommandError>`

回滚成功后，前端将收到 `file:changed` 事件。

**前端调用：**

```typescript
await invoke("rollback_version", {
  workspaceId: "ws-001",
  path: "reports/年度报告.docx",
  versionId: "v-003",
});
```

---

## 8. Skill相关命令

> 源文件：`commands/skill.rs`

### 8.1 list_skills

列出所有可用的Skill（内置 + 自定义）。

**Rust签名：**

```rust
#[tauri::command]
async fn list_skills() -> Vec<SkillInfo>
```

**参数：** 无

**返回类型：**

```rust
struct SkillInfo {
    id: String,
    name: String,
    description: String,
    category: String,                   // "document" | "data" | "format" | "custom"
    is_builtin: bool,                   // 是否为内置Skill
    is_enabled: bool,                   // 是否已启用
    version: String,
    params_schema: Option<serde_json::Value>,  // 参数JSON Schema
    supported_types: Vec<String>,       // 支持的文档类型
}
```

**前端调用：**

```typescript
const skills = await invoke<SkillInfo[]>("list_skills");
```

---

### 8.2 toggle_skill

启用或禁用指定Skill。

**Rust签名：**

```rust
#[tauri::command]
async fn toggle_skill(skill_id: String, enabled: bool) -> Result<(), CommandError>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| skill_id | String | 是 | Skill标识 |
| enabled | bool | 是 | true=启用，false=禁用 |

**返回类型：** `Result<(), CommandError>`

**前端调用：**

```typescript
await invoke("toggle_skill", { skillId: "document-converter", enabled: true });
```

---

### 8.3 add_custom_skill

添加自定义Skill。

**Rust签名：**

```rust
#[tauri::command]
async fn add_custom_skill(config: CustomSkillConfig) -> Result<(), CommandError>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| config | CustomSkillConfig | 是 | 自定义Skill配置 |

```rust
struct CustomSkillConfig {
    name: String,
    description: String,
    category: String,
    prompt_template: String,            // Skill的提示词模板
    supported_types: Vec<String>,       // 支持的文档类型
    params_schema: Option<serde_json::Value>,
}
```

**返回类型：** `Result<(), CommandError>`

**前端调用：**

```typescript
await invoke("add_custom_skill", {
  config: {
    name: "合同审查",
    description: "自动审查合同文档中的关键条款",
    category: "document",
    promptTemplate: "请审查以下合同文档...",
    supportedTypes: ["docx", "pdf"],
    paramsSchema: null,
  },
});
```

---

### 8.4 update_custom_skill

更新自定义Skill配置。

**Rust签名：**

```rust
#[tauri::command]
async fn update_custom_skill(skill_id: String, config: CustomSkillConfig) -> Result<(), CommandError>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| skill_id | String | 是 | 要更新的Skill标识 |
| config | CustomSkillConfig | 是 | 新的Skill配置 |

**返回类型：** `Result<(), CommandError>`

**前端调用：**

```typescript
await invoke("update_custom_skill", {
  skillId: "custom-contract-review",
  config: { /* ... */ },
});
```

---

### 8.5 delete_custom_skill

删除自定义Skill（内置Skill不可删除）。

**Rust签名：**

```rust
#[tauri::command]
async fn delete_custom_skill(skill_id: String) -> Result<(), CommandError>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| skill_id | String | 是 | 要删除的Skill标识 |

**返回类型：** `Result<(), CommandError>`

**前端调用：**

```typescript
await invoke("delete_custom_skill", { skillId: "custom-contract-review" });
```

---

## 9. 设置相关命令

> 源文件：`commands/settings.rs`

### 9.1 get_settings

获取应用的全局设置。

**Rust签名：**

```rust
#[tauri::command]
async fn get_settings() -> AppSettings
```

**参数：** 无

**返回类型：**

```rust
struct AppSettings {
    general: GeneralSettings,
    agent: AgentSettings,
    appearance: AppearanceSettings,
    advanced: AdvancedSettings,
}

struct GeneralSettings {
    language: String,                   // "zh-CN" | "en-US"
    auto_update: bool,                  // 是否自动检查更新
    startup_on_boot: bool,              // 是否开机自启
}

struct AgentSettings {
    default_provider_id: Option<String>,
    max_iterations: u32,                // 默认最大迭代次数
    auto_confirm: bool,                 // 默认自动确认
    confirm_dangerous_ops: bool,        // 危险操作始终需确认
    timeout_seconds: u32,               // 单次执行超时时间
}

struct AppearanceSettings {
    theme: String,                      // "light" | "dark" | "system"
    font_size: u32,                     // 编辑器字号
    sidebar_width: u32,                 // 侧边栏宽度
}

struct AdvancedSettings {
    max_concurrent_agents: u32,         // 最大并发Agent数
    log_level: String,                  // "trace" | "debug" | "info" | "warn" | "error"
    data_dir: String,                   // 数据存储目录
    cache_size_mb: u32,                 // 缓存大小上限
}
```

**返回类型：** `AppSettings`

**前端调用：**

```typescript
const settings = await invoke<AppSettings>("get_settings");
```

---

### 9.2 update_settings

更新应用设置（部分更新，仅更新传入的字段）。

**Rust签名：**

```rust
#[tauri::command]
async fn update_settings(settings: PartialAppSettings) -> Result<(), CommandError>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| settings | PartialAppSettings | 是 | 要更新的设置字段（部分更新） |

`PartialAppSettings` 与 `AppSettings` 结构相同，但所有字段均为 `Option` 类型，仅传入需要更新的字段。

**返回类型：** `Result<(), CommandError>`

**前端调用：**

```typescript
await invoke("update_settings", {
  settings: {
    general: { language: "en-US" },
    appearance: { theme: "dark" },
  },
});
```

---

### 9.3 export_config

导出应用配置为JSON字符串。

**Rust签名：**

```rust
#[tauri::command]
async fn export_config(include_secrets: Option<bool>) -> Result<String, CommandError>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| include_secrets | Option\<bool\> | 否 | 是否包含API密钥等敏感信息，默认false |

**返回类型：** `Result<String, CommandError>`，返回JSON格式的配置字符串。

**前端调用：**

```typescript
const configJson = await invoke<string>("export_config", {
  includeSecrets: false,
});
```

---

### 9.4 import_config

从JSON字符串导入应用配置。

**Rust签名：**

```rust
#[tauri::command]
async fn import_config(config_json: String, overwrite: Option<bool>) -> Result<(), CommandError>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| config_json | String | 是 | JSON格式的配置字符串 |
| overwrite | Option\<bool\> | 否 | 是否覆盖已有配置，默认false（合并） |

**返回类型：** `Result<(), CommandError>`

**前端调用：**

```typescript
await invoke("import_config", {
  configJson: '{"general":{"language":"zh-CN"},...}',
  overwrite: false,
});
```

---

## 10. 模板相关命令

> 源文件：`commands/template.rs`

### 10.1 list_templates

列出所有可用的文档模板。

**Rust签名：**

```rust
#[tauri::command]
async fn list_templates(category: Option<String>) -> Vec<TemplateInfo>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| category | Option\<String\> | 否 | 按分类筛选，如 "report"、"contract" |

```rust
struct TemplateInfo {
    id: String,
    name: String,
    description: String,
    category: String,
    file_type: String,                  // "docx" | "xlsx" | "pptx" | "md"
    is_builtin: bool,
    is_custom: bool,
    preview_path: Option<String>,       // 预览图路径
    created_at: String,
    updated_at: String,
}
```

**返回类型：** `Vec<TemplateInfo>`

**前端调用：**

```typescript
const templates = await invoke<TemplateInfo[]>("list_templates", {
  category: "report",
});
```

---

### 10.2 add_template

添加自定义模板。

**Rust签名：**

```rust
#[tauri::command]
async fn add_template(config: TemplateConfig) -> Result<(), CommandError>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| config | TemplateConfig | 是 | 模板配置 |

```rust
struct TemplateConfig {
    name: String,
    description: String,
    category: String,
    file_type: String,
    source_path: String,                // 模板源文件路径
}
```

**返回类型：** `Result<(), CommandError>`

**前端调用：**

```typescript
await invoke("add_template", {
  config: {
    name: "季度报告模板",
    description: "适用于季度业务报告",
    category: "report",
    fileType: "docx",
    sourcePath: "D:\\Templates\\quarterly.docx",
  },
});
```

---

### 10.3 update_template

更新自定义模板信息。

**Rust签名：**

```rust
#[tauri::command]
async fn update_template(template_id: String, config: TemplateConfig) -> Result<(), CommandError>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| template_id | String | 是 | 模板ID |
| config | TemplateConfig | 是 | 新的模板配置 |

**返回类型：** `Result<(), CommandError>`

**前端调用：**

```typescript
await invoke("update_template", {
  templateId: "tpl-001",
  config: { /* ... */ },
});
```

---

### 10.4 delete_template

删除自定义模板（内置模板不可删除）。

**Rust签名：**

```rust
#[tauri::command]
async fn delete_template(template_id: String) -> Result<(), CommandError>
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| template_id | String | 是 | 要删除的模板ID |

**返回类型：** `Result<(), CommandError>`

**前端调用：**

```typescript
await invoke("delete_template", { templateId: "tpl-001" });
```

---

## 11. Token相关命令

> 源文件：`commands/token.rs`

### 11.1 get_token_stats

获取Token使用统计信息。

**Rust签名：**

```rust
#[tauri::command]
async fn get_token_stats(range: Option<TimeRange>) -> TokenStats
```

**参数：**

| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
| range | Option\<TimeRange\> | 否 | 时间范围，不填则返回全部统计 |

```rust
struct TimeRange {
    start: String,                      // ISO 8601 起始时间
    end: String,                        // ISO 8601 结束时间
}

struct TokenStats {
    total_prompt_tokens: u64,
    total_completion_tokens: u64,
    total_tokens: u64,
    total_cost: f64,                    // 估算费用（美元）
    by_provider: HashMap<String, ProviderTokenUsage>,
    by_day: Vec<DailyTokenUsage>,
}

struct ProviderTokenUsage {
    provider_id: String,
    provider_name: String,
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
    cost: f64,
}

struct DailyTokenUsage {
    date: String,                       // "YYYY-MM-DD"
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
}
```

**返回类型：** `TokenStats`

**前端调用：**

```typescript
const stats = await invoke<TokenStats>("get_token_stats", {
  range: { start: "2025-01-01T00:00:00Z", end: "2025-12-31T23:59:59Z" },
});
```

---

### 11.2 check_token_budget

检查Token预算使用情况。

**Rust签名：**

```rust
#[tauri::command]
async fn check_token_budget() -> BudgetStatus
```

**参数：** 无

**返回类型：**

```rust
struct BudgetStatus {
    budget_limit: Option<u64>,          // 预算上限（Token数），None表示无限制
    budget_used: u64,                   // 已使用Token数
    budget_remaining: Option<u64>,      // 剩余Token数，None表示无限制
    cost_limit: Option<f64>,            // 费用上限（美元），None表示无限制
    cost_used: f64,                     // 已使用费用
    cost_remaining: Option<f64>,        // 剩余费用
    period: String,                     // "daily" | "monthly" | "total"
    period_start: String,               // 当前周期起始时间
    period_end: Option<String>,         // 当前周期结束时间
    is_exceeded: bool,                  // 是否已超出预算
    warning_threshold: Option<f64>,     // 预警阈值（百分比）
    is_warning: bool,                   // 是否已触发预警
}
```

**返回类型：** `BudgetStatus`

**前端调用：**

```typescript
const budget = await invoke<BudgetStatus>("check_token_budget");
```

---

## 12. Tauri事件定义

所有事件均为 Rust 后端向前端推送，前端通过 `listen` 监听。

### 12.1 Agent事件

| 事件名 | 说明 | Payload类型 | 触发时机 |
|--------|------|-------------|----------|
| `agent:thinking` | Agent思考中 | `ThinkingPayload` | Agent开始处理新步骤 |
| `agent:content` | Agent输出文本内容 | `ContentPayload` | Agent生成文本回复 |
| `agent:tool_call` | Agent调用工具 | `ToolCallPayload` | Agent发起工具调用 |
| `agent:tool_result` | 工具执行结果 | `ToolResultPayload` | 工具执行完成 |
| `agent:confirm` | 需要用户确认 | `ConfirmPayload` | Agent操作需确认 |
| `agent:todo_update` | 任务进度更新 | `TodoUpdatePayload` | Agent更新任务列表 |
| `agent:done` | 执行完成 | `DonePayload` | Agent任务完成 |
| `agent:error` | 执行出错 | `ErrorPayload` | Agent执行遇到错误 |
| `agent:stopped` | 被用户中断 | `StoppedPayload` | Agent被stop_agent中断 |

**Payload定义：**

```rust
struct ThinkingPayload {
    session_id: String,
    step: u32,
    thought: String,
}

struct ContentPayload {
    session_id: String,
    message_id: String,
    content: String,
    is_streaming: bool,                 // 是否为流式输出（后续还有内容）
}

struct ToolCallPayload {
    session_id: String,
    call_id: String,
    tool_name: String,
    arguments: serde_json::Value,
}

struct ToolResultPayload {
    session_id: String,
    call_id: String,
    success: bool,
    result: serde_json::Value,
    error: Option<String>,
    duration_ms: u64,
}

struct ConfirmPayload {
    session_id: String,
    operation_id: String,
    operation_type: String,             // "file_write" | "file_delete" | "command" | "api_call"
    description: String,
    details: serde_json::Value,         // 操作详细信息
    risk_level: String,                 // "low" | "medium" | "high"
}

struct TodoUpdatePayload {
    session_id: String,
    todos: Vec<TodoItem>,
}

struct TodoItem {
    id: String,
    content: String,
    status: String,                     // "pending" | "in_progress" | "completed" | "failed"
}

struct DonePayload {
    session_id: String,
    summary: String,
    total_steps: u32,
    total_tokens: u64,
    duration_ms: u64,
}

struct ErrorPayload {
    session_id: String,
    code: u32,
    message: String,
    recoverable: bool,
}

struct StoppedPayload {
    session_id: String,
    completed_steps: u32,
    reason: String,                     // "user_requested" | "timeout" | "budget_exceeded"
}
```

**前端监听示例：**

```typescript
import { listen } from "@tauri-apps/api/event";

// 监听Agent内容输出
const unlisten = await listen<ContentPayload>("agent:content", (event) => {
  const { session_id, content, is_streaming } = event.payload;
  appendToChat(session_id, content, is_streaming);
});

// 监听Agent确认请求
await listen<ConfirmPayload>("agent:confirm", (event) => {
  const { operation_id, description, risk_level } = event.payload;
  showConfirmDialog(operation_id, description, risk_level);
});
```

---

### 12.2 系统事件

| 事件名 | 说明 | Payload类型 | 触发时机 |
|--------|------|-------------|----------|
| `session:updated` | 会话信息更新 | `SessionUpdatePayload` | 会话属性变更 |
| `workspace:changed` | 工作区切换 | `WorkspaceChangePayload` | 活动工作区变更 |
| `file:changed` | 文件变更 | `FileChangePayload` | 工作区文件增删改 |
| `token:updated` | Token用量更新 | `TokenUpdatePayload` | Token消耗变化 |

**Payload定义：**

```rust
struct SessionUpdatePayload {
    session_id: String,
    change_type: String,                // "created" | "updated" | "deleted" | "title_changed"
    data: Option<serde_json::Value>,
}

struct WorkspaceChangePayload {
    workspace_id: String,
    workspace_name: String,
    workspace_path: String,
}

struct FileChangePayload {
    workspace_id: String,
    change_type: String,                // "created" | "modified" | "deleted" | "renamed"
    path: String,
    old_path: Option<String>,           // 重命名时的原路径
}

struct TokenUpdatePayload {
    session_id: String,
    provider_id: String,
    prompt_tokens: u64,
    completion_tokens: u64,
    total_cost: f64,
}
```

---

## 13. Python Sidecar通信协议

Python Sidecar 通过 stdin/stdout 与 Rust 后端通信，使用 JSON 格式进行数据交换。

### 13.1 通信架构

```
Rust后端 ──JSON──> stdin  ┌──────────────┐  stdout ──JSON──> Rust后端
                          │ Python       │
                          │ Sidecar      │
                          │ (文档处理)    │
                          └──────────────┘
```

- 通信方式：JSON over stdin/stdout
- 编码：UTF-8
- 消息分隔：每条消息以换行符 `\n` 结尾
- 并发处理：每条请求携带唯一 `id`，响应通过 `id` 匹配

### 13.2 请求格式

```json
{
  "id": "uuid-string",
  "action": "generate|modify|convert|read",
  "type": "docx|xlsx|pptx|pdf|md",
  "params": {
    // 动作相关参数
  }
}
```

**字段说明：**

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| id | String | 是 | 请求唯一标识，用于匹配响应 |
| action | String | 是 | 操作类型 |
| type | String | 是 | 文档类型 |
| params | Object | 是 | 操作参数，具体结构取决于action和type |

### 13.3 响应格式

```json
{
  "id": "uuid-string",
  "success": true,
  "data": {
    // 结果数据
  },
  "error": null
}
```

**字段说明：**

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| id | String | 是 | 对应请求的唯一标识 |
| success | bool | 是 | 操作是否成功 |
| data | Object | 否 | 成功时的结果数据 |
| error | String | 否 | 失败时的错误信息 |

### 13.4 Action详细定义

#### generate - 生成文档

根据模板和参数生成新文档。

```json
// 请求
{
  "id": "req-001",
  "action": "generate",
  "type": "docx",
  "params": {
    "template_path": "/templates/report.docx",
    "output_path": "/output/生成报告.docx",
    "variables": {
      "title": "2025年度报告",
      "author": "DocAgent",
      "date": "2025-05-14"
    },
    "content": "可选的正文内容（Markdown格式）"
  }
}

// 响应
{
  "id": "req-001",
  "success": true,
  "data": {
    "output_path": "/output/生成报告.docx",
    "file_size": 102400,
    "page_count": 5
  },
  "error": null
}
```

#### modify - 修改文档

对现有文档进行修改。

```json
// 请求
{
  "id": "req-002",
  "action": "modify",
  "type": "docx",
  "params": {
    "input_path": "/docs/合同.docx",
    "output_path": "/docs/合同_修改版.docx",
    "operations": [
      {
        "type": "replace_text",
        "target": "旧公司名称",
        "replacement": "新公司名称"
      },
      {
        "type": "insert_paragraph",
        "position": 3,
        "text": "新增条款内容",
        "style": "normal"
      },
      {
        "type": "delete_section",
        "section_index": 2
      }
    ]
  }
}

// 响应
{
  "id": "req-002",
  "success": true,
  "data": {
    "output_path": "/docs/合同_修改版.docx",
    "operations_applied": 3,
    "file_size": 98304
  },
  "error": null
}
```

#### convert - 格式转换

在不同文档格式之间转换。

```json
// 请求
{
  "id": "req-003",
  "action": "convert",
  "type": "pdf",
  "params": {
    "input_path": "/docs/报告.docx",
    "output_path": "/docs/报告.pdf",
    "source_type": "docx",
    "options": {
      "quality": "high",
      "preserve_layout": true,
      "embed_fonts": true
    }
  }
}

// 响应
{
  "id": "req-003",
  "success": true,
  "data": {
    "output_path": "/docs/报告.pdf",
    "source_type": "docx",
    "target_type": "pdf",
    "page_count": 5,
    "file_size": 204800
  },
  "error": null
}
```

#### read - 读取文档

提取文档内容。

```json
// 请求
{
  "id": "req-004",
  "action": "read",
  "type": "xlsx",
  "params": {
    "input_path": "/data/销售数据.xlsx",
    "options": {
      "sheet_name": "Sheet1",
      "range": "A1:Z100",
      "include_formatting": false,
      "include_formulas": true
    }
  }
}

// 响应
{
  "id": "req-004",
  "success": true,
  "data": {
    "sheet_name": "Sheet1",
    "rows": 100,
    "columns": 26,
    "content": [
      ["姓名", "部门", "销售额"],
      ["张三", "技术部", "50000"]
    ],
    "metadata": {
      "author": "admin",
      "created": "2025-01-15T10:30:00Z"
    }
  },
  "error": null
}
```

### 13.5 错误响应示例

```json
{
  "id": "req-005",
  "success": false,
  "data": null,
  "error": "文件不存在: /docs/不存在的文件.docx"
}
```

### 13.6 支持的文档类型与操作矩阵

| 类型 | generate | modify | convert | read |
|------|----------|--------|---------|------|
| docx | 支持 | 支持 | 支持 | 支持 |
| xlsx | 支持 | 支持 | 支持 | 支持 |
| pptx | 支持 | 支持 | 支持 | 支持 |
| pdf | 不支持 | 不支持 | 支持（仅输出） | 支持 |
| md | 支持 | 支持 | 支持 | 支持 |

---

## 14. 错误码定义

### 14.1 错误码结构

错误码为4位数字，格式为 `Exxxx`，其中 `E` 为固定前缀，`xxxx` 为4位数字编号。

### 14.2 错误码范围

| 范围 | 模块 | 说明 |
|------|------|------|
| 1000-1999 | LLM | LLM相关错误 |
| 2000-2999 | Agent | Agent相关错误 |
| 3000-3999 | Document | 文档处理错误 |
| 4000-4999 | Database | 数据库错误 |
| 5000-5999 | Config | 配置错误 |
| 6000-6999 | FileSystem | 文件系统错误 |

### 14.3 LLM相关错误 (1xxx)

| 错误码 | 常量名 | 说明 | 处理建议 |
|--------|--------|------|----------|
| 1001 | LLM_CONNECTION_FAILED | 连接LLM服务失败 | 检查网络连接和API地址 |
| 1002 | LLM_AUTH_FAILED | API密钥认证失败 | 检查API密钥是否正确 |
| 1003 | LLM_RATE_LIMITED | 请求频率超限 | 等待后重试或升级套餐 |
| 1004 | LLM_QUOTA_EXCEEDED | 配额已用尽 | 检查账户余额或更换Provider |
| 1005 | LLM_MODEL_NOT_FOUND | 指定的模型不存在 | 检查模型名称是否正确 |
| 1006 | LLM_TIMEOUT | 请求超时 | 增加超时时间或重试 |
| 1007 | LLM_INVALID_REQUEST | 请求参数无效 | 检查请求参数格式 |
| 1008 | LLM_STREAM_ERROR | 流式响应中断 | 重试请求 |
| 1009 | LLM_PROVIDER_UNAVAILABLE | Provider服务不可用 | 切换到其他Provider |
| 1010 | LLM_RESPONSE_PARSE_ERROR | 响应解析失败 | 检查模型兼容性 |

### 14.4 Agent相关错误 (2xxx)

| 错误码 | 常量名 | 说明 | 处理建议 |
|--------|--------|------|----------|
| 2001 | AGENT_ALREADY_RUNNING | Agent已在运行中 | 等待当前任务完成或中断 |
| 2002 | AGENT_NOT_RUNNING | Agent未在运行 | 无需处理 |
| 2003 | AGENT_MAX_ITERATIONS | 达到最大迭代次数 | 增加迭代上限或简化任务 |
| 2004 | AGENT_CONFIRMATION_TIMEOUT | 确认操作超时 | 及时响应确认请求 |
| 2005 | AGENT_OPERATION_REJECTED | 操作被用户拒绝 | 修改任务描述后重试 |
| 2006 | AGENT_SKILL_NOT_FOUND | 指定的Skill不存在 | 检查Skill ID |
| 2007 | AGENT_SKILL_DISABLED | Skill已被禁用 | 启用对应Skill |
| 2008 | AGENT_EXECUTION_ERROR | Agent执行内部错误 | 查看日志获取详细信息 |
| 2009 | AGENT_BUDGET_EXCEEDED | Token预算已超限 | 增加预算或优化提示词 |
| 2010 | AGENT_SESSION_NOT_FOUND | 关联的会话不存在 | 检查会话ID |

### 14.5 文档处理错误 (3xxx)

| 错误码 | 常量名 | 说明 | 处理建议 |
|--------|--------|------|----------|
| 3001 | DOC_FILE_NOT_FOUND | 文档文件不存在 | 检查文件路径 |
| 3002 | DOC_FORMAT_UNSUPPORTED | 不支持的文档格式 | 检查文件扩展名 |
| 3003 | DOC_PARSE_ERROR | 文档解析失败 | 文件可能已损坏 |
| 3004 | DOC_WRITE_ERROR | 文档写入失败 | 检查输出路径权限 |
| 3005 | DOC_CONVERT_ERROR | 格式转换失败 | 检查源文件和目标格式兼容性 |
| 3006 | DOC_TEMPLATE_NOT_FOUND | 模板文件不存在 | 检查模板路径 |
| 3007 | DOC_TEMPLATE_ERROR | 模板渲染失败 | 检查模板变量是否完整 |
| 3008 | DOC_VERSION_NOT_FOUND | 版本记录不存在 | 检查版本ID |
| 3009 | DOC_ROLLBACK_FAILED | 版本回滚失败 | 检查版本文件完整性 |
| 3010 | DOC_SIDECAR_ERROR | Python Sidecar通信错误 | 检查Sidecar进程状态 |
| 3011 | DOC_PERMISSION_DENIED | 文档访问权限不足 | 检查文件读写权限 |
| 3012 | DOC_FILE_TOO_LARGE | 文件过大 | 压缩文件或拆分处理 |

### 14.6 数据库错误 (4xxx)

| 错误码 | 常量名 | 说明 | 处理建议 |
|--------|--------|------|----------|
| 4001 | DB_CONNECTION_FAILED | 数据库连接失败 | 检查数据库文件权限 |
| 4002 | DB_QUERY_FAILED | 查询执行失败 | 查看日志获取SQL详情 |
| 4003 | DB_RECORD_NOT_FOUND | 记录不存在 | 检查查询条件 |
| 4004 | DB_RECORD_EXISTS | 记录已存在 | 使用更新操作替代 |
| 4005 | DB_CONSTRAINT_VIOLATION | 约束冲突 | 检查数据完整性 |
| 4006 | DB_MIGRATION_FAILED | 数据库迁移失败 | 检查迁移脚本 |
| 4007 | DB_CORRUPTED | 数据库损坏 | 从备份恢复 |

### 14.7 配置错误 (5xxx)

| 错误码 | 常量名 | 说明 | 处理建议 |
|--------|--------|------|----------|
| 5001 | CONFIG_INVALID_FORMAT | 配置格式无效 | 检查JSON格式 |
| 5002 | CONFIG_MISSING_FIELD | 缺少必填配置项 | 补充缺失配置 |
| 5003 | CONFIG_INVALID_VALUE | 配置值无效 | 检查配置值范围 |
| 5004 | CONFIG_IMPORT_FAILED | 配置导入失败 | 检查导入文件格式 |
| 5005 | CONFIG_EXPORT_FAILED | 配置导出失败 | 检查磁盘空间 |
| 5006 | CONFIG_PROVIDER_NOT_FOUND | Provider配置不存在 | 先添加Provider |
| 5007 | CONFIG_DEFAULT_PROVIDER_REQUIRED | 需要设置默认Provider | 设置默认Provider |

### 14.8 文件系统错误 (6xxx)

| 错误码 | 常量名 | 说明 | 处理建议 |
|--------|--------|------|----------|
| 6001 | FS_PATH_NOT_FOUND | 路径不存在 | 检查路径是否正确 |
| 6002 | FS_PERMISSION_DENIED | 权限不足 | 以管理员身份运行或修改权限 |
| 6003 | FS_ALREADY_EXISTS | 文件/目录已存在 | 更换名称或删除已有项 |
| 6004 | FS_NOT_A_DIRECTORY | 路径不是目录 | 检查路径类型 |
| 6005 | FS_DISK_FULL | 磁盘空间不足 | 清理磁盘空间 |
| 6006 | FS_IO_ERROR | IO读写错误 | 检查磁盘健康状态 |
| 6007 | FS_WATCH_ERROR | 文件监听失败 | 检查系统inotify/FSEvents限制 |
| 6008 | FS_ENCODING_ERROR | 文件编码错误 | 检查文件编码格式 |

---

## 附录A：类型索引

以下为本文档中所有自定义类型的快速索引：

| 类型名 | 所属模块 | 首次出现位置 |
|--------|----------|-------------|
| ConnectionResult | LLM | 3.1 |
| ModelInfo | LLM | 3.1 |
| ProviderInfo | LLM | 3.2 |
| ProviderConfig | LLM | 3.3 |
| AgentOptions | Agent | 4.1 |
| Session | Session | 5.1 |
| CreateSessionParams | Session | 5.1 |
| SessionFilter | Session | 5.2 |
| SessionSummary | Session | 5.2 |
| SessionDetail | Session | 5.3 |
| Message | Session | 5.3 |
| ToolCall | Session | 5.3 |
| TokenUsage | Session | 5.3 |
| WorkspaceInfo | Workspace | 6.1 |
| FileNode | Workspace | 6.5 |
| SearchOptions | Workspace | 6.6 |
| SearchResult | Workspace | 6.6 |
| PreviewContent | Document | 7.1 |
| DocumentMetadata | Document | 7.1 |
| VersionInfo | Document | 7.2 |
| SkillInfo | Skill | 8.1 |
| CustomSkillConfig | Skill | 8.3 |
| AppSettings | Settings | 9.1 |
| GeneralSettings | Settings | 9.1 |
| AgentSettings | Settings | 9.1 |
| AppearanceSettings | Settings | 9.1 |
| AdvancedSettings | Settings | 9.1 |
| TemplateInfo | Template | 10.1 |
| TemplateConfig | Template | 10.2 |
| TimeRange | Token | 11.1 |
| TokenStats | Token | 11.1 |
| ProviderTokenUsage | Token | 11.1 |
| DailyTokenUsage | Token | 11.1 |
| BudgetStatus | Token | 11.2 |
| CommandError | 通用 | 2.3 |

---

## 附录B：事件Payload类型索引

| 类型名 | 所属事件 | 首次出现位置 |
|--------|----------|-------------|
| ThinkingPayload | agent:thinking | 12.1 |
| ContentPayload | agent:content | 12.1 |
| ToolCallPayload | agent:tool_call | 12.1 |
| ToolResultPayload | agent:tool_result | 12.1 |
| ConfirmPayload | agent:confirm | 12.1 |
| TodoUpdatePayload | agent:todo_update | 12.1 |
| TodoItem | agent:todo_update | 12.1 |
| DonePayload | agent:done | 12.1 |
| ErrorPayload | agent:error | 12.1 |
| StoppedPayload | agent:stopped | 12.1 |
| SessionUpdatePayload | session:updated | 12.2 |
| WorkspaceChangePayload | workspace:changed | 12.2 |
| FileChangePayload | file:changed | 12.2 |
| TokenUpdatePayload | token:updated | 12.2 |
