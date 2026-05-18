# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

DocAgent 是一款 AI 文档处理桌面应用，通过自然语言驱动 Agent 完成 Word、Excel、PDF、PPT、Markdown 等文档的生成、修改、格式转换等操作。技术栈：Tauri 2 + React 19 + TypeScript 5 + Zustand 5 + Tailwind CSS 4。

## 开发阶段（Phase 1 MVP 后期）

当前各模块完成度：

| 模块 | 完成度 | 说明 |
|------|--------|------|
| 前端 UI 组件 | 100% | 组件、Store、事件封装、后端调用全部完成，设置管理按钮已对接后端 |
| Rust 后端 | 100% | Agent 引擎含确认通道和 Todo 事件，LLM 适配、Skill 系统（含禁用/启用持久化）、Sidecar 集成全部完成 |
| Python Sidecar | 95% | 所有文档处理器已实现，部分格式的 convert/modify 受限 |
| 共享类型 | 10% | 仅定义了 NodeType 和 ExecutionStatus |
| 设计文档 | 100% | PRD、技术架构、组件设计、数据库设计等齐全 |

### 各模块详细状态

#### 前端 UI 组件（100% 完成）

| 子模块 | 状态 | 说明 |
|--------|------|------|
| 布局组件 | 完成 | TopBar（状态指示器对接实际 LLM Provider）、MainLayout、Sidebar、MainArea、InputArea |
| 工作流节点 | 完成 | WorkflowTimeline + 7 种节点组件，支持展开/折叠 |
| 侧边栏区块 | 完成 | AgentInfo、FileTree、Todo、Token 四个区块 |
| 设置对话框 | 完成 | SettingsDialog + 5 个标签页（Provider 添加/编辑/删除/测试/设默认、工作区添加/切换/移除均已对接后端） |
| 预览面板 | 完成 | PreviewOverlay 支持文档预览和基础差异对比 |
| 状态管理 | 完成 | 6 个 Zustand Store 全部实现，已连接后端 Tauri 命令 |
| 事件监听 | 完成 | 完整的 Agent 事件监听封装（event.ts），9 种 Agent 事件 + 4 种系统事件 |
| Tauri 命令封装 | 完成 | 全部 29 个 Tauri 命令的 TypeScript 封装（tauri.ts） |
| useAgent Hook | 完成 | 封装 Agent 调用逻辑，自动管理事件监听和状态更新 |

#### Rust 后端（100% 完成）

| 子模块 | 状态 | 说明 |
|--------|------|------|
| 数据库层 | 完成 | SQLite 封装、5 张表（schema_version, sessions, session_messages, version_snapshots, token_usage）、CRUD 全部实现，WAL 模式+外键约束 |
| 配置管理 | 完成 | LLM 配置、应用设置、工作区配置全部实现，JSON 文件持久化，支持向前兼容合并 |
| 模型定义 | 完成 | 全部 7 个模型模块（session, message, workspace, document, skill, llm, 含 ChatMessage/ToolDefinition 等） |
| 事件系统 | 完成 | AgentEmitter 封装全部 10 种事件类型，含发射方法和 payload 类型定义 |
| LLM 服务 | 完成 | OpenAI 兼容 API 适配器完整实现，支持流式(SSE)和非流式调用，指数退避重试，Default/Fallback 路由，list_providers 返回完整元数据 |
| Agent 执行器 | 完成 | 完整实现：包含确认通道（高风险 Skill 前发射 agent:confirm 并通过 oneshot channel 等待用户决策）、Todo 进度更新（agent:todo_update）、Tool Calling 循环，支持停止检查和最大迭代限制 |
| Skill 注册表 | 完成 | Skill trait + 注册表框架 + 9 个内置 Skills（含参数 schema）+ 禁用/启用能力（运行时修改+配置持久化），线程安全（Mutex 保护） |
| Sidecar 集成 | 完成 | SidecarManager（进程管理/超时/自动重启）+ DocumentService（请求/响应路由）与 Python stdin/stdout 通信 |
| Tauri 命令 | 完成 | 全部 29 个核心命令已注册（session、settings、workspace、llm、agent、skill、document），confirm_operation 基于 oneshot channel 实现，toggle_skill 已完成持久化 |

#### Python Sidecar（95% 完成）

| 处理器 | 状态 | 功能 |
|--------|------|------|
| Word 处理器 | 完成 | generate、read、modify、convert、analyze 全部实现 |
| Excel 处理器 | 完成 | generate、read、modify、analyze 实现；convert 暂未实现 |
| PDF 处理器 | 完成 | generate、read、analyze 实现；modify 和 convert 暂不支持 |
| PPT 处理器 | 完成 | generate、read、modify、analyze 实现；convert 暂未实现 |
| Markdown 处理器 | 完成 | generate、read、modify、analyze 实现；convert 暂未实现 |
| main.py | 完成 | stdin/stdout JSON 协议通信，日志输出到 log/sidecar.log |

### 尚未完成的工作（需在后续迭代中完善）

以下功能已预留接口但尚未完全实现：

| 功能 | 状态 | 说明 |
|------|------|------|
| 自定义 Skill 添加/删除 | stub | `add_custom_skill`、`delete_custom_skill` 仅记录日志，无实际配置持久化 |
| 设置对话框扩展 | stub | 自定义 Skill 添加/删除、模板创建等按钮无实际后端调用 |
| PDF modify/convert | 未实现 | Sidecar PDF 处理器中 modify 和 convert 返回错误 |
| 其余格式 convert | 未实现 | Excel/PPT/Markdown 处理器的 convert 均返回未实现错误 |
| LLM 适配器扩展 | 未实现 | 当前仅实现 OpenAI 兼容 API 适配器；Claude 适配器（Anthropic API）和 Gemini 适配器待开发 |
| 共享类型自动化 | 未实现 | 引入 ts-rs 等工具自动生成 TypeScript 类型，确保前后端类型同步 |

### 下一步开发重点

1. **优先级中：端到端测试与联调**
   - 测试完整 Agent 执行流程（思考→工具调用→确认通道→文档生成/读取）
   - 测试事件流（agent:thinking、agent:content、agent:confirm 等增量事件）
   - 验证数据持久化（会话、消息、Token 统计）
   - 测试 Sidecar 通信和文档处理

2. **优先级中：Skill 系统完善**
   - 实现自定义 Skill 的动态加载与持久化
   - 完善设置对话框的 Skill 管理/模板管理标签页

3. **优先级低：LLM 适配器扩展**
   - 实现 Claude 适配器（Anthropic API）
   - 实现 Gemini 适配器（Google AI API）

4. **优先级低：Sidecar convert 功能完善**
   - 实现 Excel/PPT/Markdown 的格式转换
   - 实现 PDF 的 modify 和 convert

5. **优先级低：共享类型自动化**
   - 引入 ts-rs 或类似工具自动生成 TypeScript 类型
   - 确保前后端类型同步

## 常用命令

```bash
# 启动 Vite 开发服务器（端口 1420）
npm run dev

# TypeScript 检查 + Vite 构建
npm run build

# 启动 Tauri 桌面应用开发模式
npm run tauri:dev

# Tauri 生产构建
npm run tauri:build
```

## 目录结构

```
src/                      # React 前端
├── components/
│   ├── layout/           # TopBar, MainLayout, MainArea, Sidebar, InputArea
│   ├── workflow/         # WorkflowTimeline + 7 种节点组件
│   ├── sidebar/          # FileTreeSection, AgentInfoSection, TodoSection, TokenSection
│   ├── preview/          # PreviewOverlay 预览面板
│   ├── settings/         # SettingsDialog + 5 个标签页
│   ├── session/          # HistoryPanel
│   └── common/           # Button, Icon
├── stores/               # 6 个 Zustand Store
├── types/                # 类型定义（workflow、session、workspace、settings、document）
├── utils/                # fileIcons, format, logger
├── services/             # event.ts（事件监听封装，9 种 Agent + 4 种系统事件）、tauri.ts（29 个 Tauri 命令封装）
├── hooks/                # useAgent Hook
└── styles/globals.css    # Tailwind + 自定义设计令牌

src-tauri/                # Tauri Rust 后端
├── src/
│   ├── commands/         # Tauri 命令（29 个）
│   │   ├── agent.rs      # start_agent, stop_agent, confirm_operation
│   │   ├── session.rs    # 会话 CRUD
│   │   ├── settings.rs   # 应用设置
│   │   ├── workspace.rs  # 工作区管理、文件树、搜索
│   │   ├── llm.rs        # LLM Provider 管理
│   │   ├── skill.rs      # Skill 管理（list_skills + toggle_skill 完整实现含持久化）
│   │   └── document.rs   # 文档操作
│   ├── services/         # 服务层
│   │   ├── agent/        # Agent 执行器（executor.rs 含确认通道+Todo 事件、context.rs）
│   │   ├── llm/          # LLM 服务（router.rs 完整 Provider 元数据，list_providers 返回真实信息）
│   │   ├── skill/        # Skill 系统（registry.rs, builtin.rs）
│   │   └── document/     # 文档服务（mod.rs - SidecarManager, DocumentService）
│   ├── db/               # SQLite 数据库层（init.rs, session_repo.rs, message_repo.rs 等）
│   ├── config/           # 配置管理（llm_config.rs, app_settings.rs, workspace_config.rs）
│   ├── models/           # 数据模型定义（session, message, workspace, document, skill, llm）
│   ├── events/           # 事件系统（emitter.rs, types.rs）
│   ├── utils/            # 工具函数（logger.rs）
│   ├── errors.rs         # 统一错误类型，7 个错误域 55+ 命名常量
│   └── lib.rs            # 应用入口、AppState 全局状态管理、29 个命令注册
├── capabilities/         # Tauri 权限配置
├── gen/                  # 自动生成的 schema 文件
└── resources/            # 资源文件

sidecar/                  # Python Sidecar
├── main.py               # 入口，stdin/stdout JSON 协议通信
├── requirements.txt      # Python 依赖
└── handlers/             # 文档处理器
    ├── word_handler.py   # Word 文档处理（完整实现含 convert）
    ├── excel_handler.py  # Excel 文档处理（convert 暂未实现）
    ├── pdf_handler.py    # PDF 文档处理（modify/convert 暂不支持）
    ├── ppt_handler.py    # PPT 文档处理（convert 暂未实现）
    └── markdown_handler.py # Markdown 文档处理（convert 暂未实现）

shared/types.ts           # 前后端共享类型（极少维护）
docs/                     # 设计文档
├── PRD_DocAgent.md       # 产品需求文档
├── tech_architecture.md  # 技术架构文档
├── component_design.md   # 前端组件设计文档
├── database_design.md    # 数据库设计文档
├── tauri_commands.md     # Tauri 命令接口文档
├── skill_development.md  # Skill 系统开发规范
└── task_breakdown.md     # 开发任务分解文档
```

## 核心架构

### 通信方式
- `invoke()` — 请求-响应式调用（同步）
- `emit()/listen()` — 事件推送（流式输出、进度更新）
- **事件命名**：`namespace:action` 格式（如 `agent:thinking`, `session:updated`）

### Agent 执行流程
前后端完整的流式事件处理协议已实现，后端执行器核心流程如下：

1. `agent:thinking` — LLM 思考链增量（executor 中已发射）
2. `agent:content` — LLM 回复内容增量，is_streaming 标识流式状态（executor 中已发射）
3. `agent:tool_call` — Tool 调用开始（executor 中已发射）
4. `agent:tool_result` — Tool 执行结果（executor 中已发射）
5. `agent:confirm` — 高风险操作前发射（delete_document/modify_document/batch_process），通过 oneshot channel 等待用户确认/超时（executor 中 `request_confirmation` 方法已实现）
6. `agent:todo_update` — Todo 列表每步执行和完成时更新（executor 中 `emit_todo_progress` 方法已实现）
7. `agent:done` — 执行完成（executor 中已发射，含 total_steps、total_tokens、duration_ms）
8. `agent:error` / `agent:stopped` — 错误/中断（executor 中已发射）

Agent 生命周期由 `useAgent` Hook 管理，自动注册/清理事件监听，状态更新到 `useWorkflowStore`。

**确认通道机制**：
- `AppState` 维护 `confirm_channels: HashMap<String, oneshot::Sender<ConfirmDecision>>`
- executor 在高风险 Skill 执行前发射 `agent:confirm` 事件并创建 channel
- 前端 ConfirmNode 展示确认对话框 → `confirmOperation` 命令 → channel 发送决策 → executor 继续或跳过
- 超时时间 300 秒，超时后自动拒绝并发射 `agent:error`
- 已实现流式事件驱动架构：9 种 Agent 事件 + 4 种系统事件，前后端类型完全对齐（serde camelCase ↔ TypeScript camelCase）

### 状态管理
6 个 Zustand Store 职责分离：
- `useWorkflowStore` — 工作流节点管理（addNode/updateNode/removeNode/clearNodes）+ confirmHandler（确认回调）
- `useSessionStore` — 会话管理（创建/列表/删除/更新标题）
- `useWorkspaceStore` — 工作区管理（添加/切换/删除/列表）
- `useSettingsStore` — 设置、LLM、Skill、模板管理
- `useFileTreeStore` — 文件树管理（从后端加载/搜索过滤）
- `useTokenStore` — Token 统计（会话/日/月累计和预算管理）

### 错误处理架构
Rust 后端通过统一的 `CommandError` 类型返回错误：
- **7 个错误域**：LLM (1000-1999)、Agent (2000-2999)、Document (3000-3999)、Database (4000-4999)、Config (5000-5999)、FileSystem (6000-6999)、Runtime (7000-7999)
- **55+ 命名常量**（如 `LLM_CONNECTION_FAILED: 1001`、`AGENT_CONFIRMATION_TIMEOUT: 2004`）
- **自动转换**：`rusqlite::Error`、`reqwest::Error`、`serde_json::Error`、`std::io::Error`、`tauri::Error` 均自动映射到对应错误域
- 前端通过 `agent:error` 事件或 `invoke` 抛出的 `CommandError` 接收错误信息

### Rust 后端依赖
关键 crate：`tauri 2`、`tauri-plugin-shell 2`、`serde/serde_json`、`tokio`（full）、`rusqlite 0.31`（bundled）、`reqwest 0.12`（json+stream+rustls-tls）、`uuid`（v4）、`chrono`（serde）、`async-trait`、`futures`、`eventsource-stream 0.2`（SSE 流式解析）、`dirs 5`、`log`。Python Sidecar 依赖：`python-docx`、`openpyxl`、`python-pptx`、`reportlab`、`PyMuPDF`。

### Python Sidecar
文档处理通过独立 Python 进程执行，与 Rust 后端通过 stdin/stdout JSON 协议通信。

**通信协议**：
- 请求格式：`{ "id": "uuid", "action": "generate|read|modify|...", "type": "docx|xlsx|...", "params": {...} }`
- 响应格式：`{ "id": "uuid", "success": true|false, "data": {...}, "error": "..." }`

**依赖库**：
- `python-docx` — Word 文档处理
- `openpyxl` — Excel 文档处理
- `python-pptx` — PPT 文档处理
- `reportlab` — PDF 生成
- `PyMuPDF` (fitz) — PDF 读取

**SidecarManager 特性**：
- 自动启动和重启 Sidecar 进程
- 请求超时处理（默认 120 秒）
- 失败后自动重试
- stderr 日志转发到 Rust 日志系统

### 应用状态管理
`AppState` 是全局共享状态（通过 `tauri::State` 注入）：
- `db: Arc<Database>` — SQLite 数据库连接（Mutex 保护）
- `config: Arc<Mutex<ConfigManager>>` — 配置管理器（LLM、应用、工作区配置，JSON 文件持久化）
- `active_agents: Arc<Mutex<HashMap<String, bool>>>` — 活跃 Agent 追踪（bool 表示是否运行中）
- `confirm_channels: Arc<Mutex<HashMap<String, oneshot::Sender<ConfirmDecision>>>>` — 确认通道表（operation_id → sender）
- `doc_service: Arc<DocumentService>` — 文档服务（Sidecar 路由）
- `llm_router: Arc<RwLock<Arc<LlmRouter>>>` — LLM 路由表（读写锁，支持运行时热切换）
- `skill_registry: Arc<Mutex<SkillRegistry>>` — Skill 注册表（Mutex 保护，支持运行时修改禁用状态）

### 数据库设计
SQLite 数据库包含 5 张表：
- `schema_version` — 数据库版本元数据
- `sessions` — 会话记录
- `session_messages` — 消息历史（含 tool_calls/tool_result 字段）
- `version_snapshots` — 文档版本快照
- `token_usage` — Token 统计

数据库特性：
- WAL 模式提升并发性能
- 外键约束保证数据完整性
- 通过 Mutex 保护连接，支持多线程访问

### Skill 系统
内置 9 个 Skills，通过 Tool Calling 与 LLM 交互：

| Skill | 功能 | 实现方式 |
|-------|------|----------|
| `generate_document` | 生成新文档 | Sidecar |
| `read_document` | 读取文档内容 | Sidecar |
| `modify_document` | 修改已有文档 | Sidecar（不支持 PDF） |
| `delete_document` | 删除文档 | Rust 原生（支持备份） |
| `convert_format` | 格式转换 | Sidecar（部分格式未实现） |
| `search_documents` | 搜索文档 | Rust 原生（文件名/内容搜索） |
| `analyze_document` | 分析文档 | Sidecar |
| `list_workspace` | 列出工作区文件 | Rust 原生（深度控制/扩展名过滤） |
| `batch_process` | 批量处理 | Sidecar |

## 开发注意事项

- **命名规范**：Tauri 命令用 `snake_case`，前端封装用 `camelCase`，事件名用 `namespace:action`
- **状态管理**：避免直接修改 store 中的数组/对象，使用不可变更新
- **组件优化**：工作流节点使用 React.memo，长列表使用虚拟滚动，搜索输入使用防抖
- **提交规范**：遵循 Conventional Commits 格式（feat/fix/docs/refactor/chore 等），使用中文描述
- **错误处理**：Rust 后端使用统一的 `CommandError` 类型（7 个错误域 55+ 命名常量），前端通过事件接收错误信息
- **日志规范**：Rust 后端使用 `log` crate 输出到文件和终端，Python Sidecar 使用 `logging` 模块输出到 `log/sidecar.log`，均支持按级别控制
- **Sidecar 调试**：可通过环境变量 `DOCAGENT_PYTHON` 指定 Python 路径
