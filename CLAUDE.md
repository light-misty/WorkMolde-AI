# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概览

DocAgent 是一个基于 Tauri 2.x 的 AI 文档处理桌面应用。用户通过对话式 AI Agent 完成 Word/Excel/PPT/PDF/Markdown 文档的生成、读取、修改、格式转换等操作。

## 当前开发阶段

项目处于 **Phase 2 - 格式扩展**，已完成多格式文档 Sidecar handler、多 LLM Provider 适配（OpenAI/Anthropic/Gemini/Ollama/Custom），正在完善文档预览浮层、文件树、格式转换功能。

## 技术栈

- **桌面框架**: Tauri 2.x (Rust + React/TypeScript)
- **前端**: React 19 + TypeScript 5 + Vite 6 + Tailwind CSS 4
- **状态管理**: Zustand 5
- **后端语言**: Rust 1.80+ (edition 2021)
- **数据库**: SQLite (rusqlite, bundled)
- **配置存储**: JSON 文件 (serde)
- **文档处理**: Python 3.12+ Sidecar (python-docx / openpyxl / python-pptx / PyMuPDF / reportlab)

## 构建与运行命令

```bash
# 开发模式（前端热更新 + Tauri 桌面窗口）
npm run tauri:dev

# 仅启动前端开发服务器（浏览器访问 localhost:1420）
npm run dev

# 生产构建
npm run tauri:build

# TypeScript 类型检查 + Vite 构建
npm run build    # tsc -b && vite build

# 预览生产构建
npm run preview

# Python Sidecar 依赖安装
pip install -r sidecar/requirements.txt
```

环境变量 `DOCAGENT_PYTHON` 可指定 Python 解释器路径。

## 项目架构

```
┌─ src/                  React 前端 (TypeScript)
│  ├─ components/
│  │  ├─ layout/         布局组件: TopBar, MainArea, Sidebar, InputArea, WindowControls
│  │  ├─ workflow/       工作流时间线: WorkflowTimeline, WorkflowNode (User/Thinking/Tool/Result/Reply/Confirm)
│  │  ├─ sidebar/        右侧栏: FileTreeSection, AgentInfoSection, TodoSection, TokenSection
│  │  ├─ preview/        文档预览浮层: PreviewOverlay, MarkdownPreview
│  │  ├─ settings/       设置弹窗: LLMConfig, WorkspaceTab, SkillsTab, TemplatesTab, GeneralTab
│  │  ├─ session/        历史会话面板: HistoryPanel
│  │  └─ common/         通用组件: Button, Icon, ContextMenu, DeleteConfirmDialog
│  ├─ stores/            Zustand stores: workflow, session, settings, workspace, fileTree, token
│  ├─ services/          前端服务层: tauri.ts (invoke封装), event.ts (事件监听/类型定义)
│  ├─ hooks/             useAgent.ts (Agent交互核心hook)
│  └─ types/             TypeScript类型定义 (与Rust后端对齐)
│
├─ src-tauri/            Rust 后端
│  ├─ src/
│  │  ├─ lib.rs          入口, AppState定义, 命令注册, 初始化流程
│  │  ├─ commands/       Tauri命令层 (8个模块): llm, session, workspace, document, skill, settings, agent, mod
│  │  ├─ services/       业务逻辑层
│  │  │  ├─ agent/       Agent调度引擎: executor (Tool Calling循环), context (对话上下文管理)
│  │  │  ├─ llm/         LLM多Provider适配: router (路由+健康检查+fallback), provider (trait),
│  │  │  │                  openai_adapter, anthropic_adapter, gemini_adapter
│  │  │  ├─ skill/       Skill引擎: registry (注册表+禁用管理), builtin (9个内置技能)
│  │  │  ├─ document/    Python Sidecar进程管理 (自动重启、超时、重试)
│  │  │  └─ fs_watcher/  文件系统监听 (notify crate, 递归监听+事件发射)
│  │  ├─ db/             数据库层: init (迁移), session/message/snapshot/token repo
│  │  ├─ config/         配置管理: app_settings, llm_config, workspace_config, ConfigManager
│  │  ├─ models/         数据模型: message, session, document, llm, skill, workspace
│  │  ├─ events/         事件系统: types (事件名常量+payload), emitter (事件发射封装)
│  │  ├─ utils/          工具: logger (双输出日志: 文件+stderr)
│  │  └─ errors.rs       统一错误码 (按模块分段: LLM/Agent/Doc/DB/Config/FS/Runtime)
│  └─ Cargo.toml
│
├─ sidecar/              Python 文档处理引擎
│  ├─ main.py            stdin/stdout JSON 行协议入口
│  └─ handlers/          文档处理器: word_handler, excel_handler, ppt_handler, pdf_handler, markdown_handler
│
├─ shared/               前后端共享类型 (TypeScript, 需与Rust端手动同步)
└─ docs/                 开发文档
```

## 核心架构要点

### 前后端通信
- **`invoke()`**: 请求-响应式调用（查询数据、操作触发），命令名 `snake_case`
- **`emit()/listen()`**: 事件推送（Agent流式输出、进度更新、需确认操作等），事件名 `namespace:action`
- Agent 事件: `agent:thinking`, `agent:content`, `agent:tool_call`, `agent:tool_result`, `agent:confirm`, `agent:todo_update`, `agent:done`, `agent:error`, `agent:stopped`
- 系统事件: `session:updated`, `workspace:change`, `file:change`, `token:update`, `llm:provider_switch`

### Agent 执行流程
1. 前端 `useAgent` hook 调用 `start_agent` 命令
2. Rust `AgentExecutor` 构建上下文（System Prompt + Skill Tool Definitions + 历史消息）
3. 循环: 调用 LLM (流式) → 解析响应 → 执行 Tool Calling (Skill) → 返回结果给 LLM
4. 高风险操作（删除/修改/批量）需用户确认，通过 `confirm_channels` oneshot channel 同步等待（5分钟超时）
5. 每轮迭代后增量持久化消息到 SQLite，防止崩溃丢失
6. Skill 执行时短暂持锁获取 Arc 引用后立即释放，避免阻塞注册表
7. 支持用户手动停止（`stop_agent`），通过 `should_stop` 闭包检查

### LLM Provider 系统
- **OpenAI 适配器**: 兼容 OpenAI API 格式的 Provider（含 Ollama、兼容第三方）
- **Anthropic 适配器**: 原生 Anthropic Messages API
- **Gemini 适配器**: 原生 Gemini API
- **LlmRouter**: 管理多个 Provider，支持默认选择、顺序 Fallback、健康检查（5分钟自动恢复）、延迟 EMA 追踪
- Provider 类型: `openai`, `anthropic`, `ollama`, `custom`, `gemini`
- 每 5 分钟后台自动执行健康检查，自动标记不可用 Provider

### Skill 系统
- 每个 Skill 实现 `Skill` trait: `skill_name()`, `description()`, `parameters()` (JSON Schema), `execute()`
- 内置 9 个 Skill:
  - `generate_document`: 生成 docx/xlsx/pptx/pdf/md
  - `read_document`: 读取文档内容
  - `modify_document`: 修改文档（替换/添加段落/添加表格等）
  - `delete_document`: 删除文件（Rust 原生，含路径安全校验+可选备份）
  - `convert_format`: 格式转换（Rust 原生+Sidecar）
  - `search_documents`: 文件搜索（Rust 原生，支持文件名/内容/扩展名过滤）
  - `analyze_document`: 文档分析（Rust 原生）
  - `list_workspace`: 列出目录结构（Rust 原生，含路径安全校验）
  - `batch_process`: 批量处理（支持批量转换/修改/分析）
- 路径安全：所有文件操作 Skill 通过 executor 注入 `workspace_root`，拒绝路径遍历攻击
- 自定义 Skill 可通过配置添加，执行时转发到 Python Sidecar

### AppState 全局状态
```
AppState {
    db: Arc<Database>,
    config: Arc<Mutex<ConfigManager>>,
    active_agents: Arc<Mutex<HashMap<String, bool>>>,
    confirm_channels: Arc<Mutex<HashMap<String, oneshot::Sender<ConfirmDecision>>>>,
    doc_service: Arc<DocumentService>,
    llm_router: Arc<RwLock<Arc<LlmRouter>>>,
    skill_registry: Arc<Mutex<SkillRegistry>>,
    fs_watcher: Arc<FsWatcherService>,
}
```

### Python Sidecar 通信协议
- stdin/stdout JSON 行协议，SidecarManager 自动管理进程生命周期（启动、停止、崩溃时自动重启）
- 请求: `{"id": "...", "action": "generate|read|modify|delete|convert|analyze", "type": "docx|xlsx|pptx|pdf|md", "params": {...}}`
- 响应: `{"id": "...", "success": true|false, "data": {...}, "error": "..."}`
- 默认请求超时 120 秒，超时后自动重启 Sidecar 进程并重试一次

### 错误码体系
错误码按模块分段，定义在 `errors.rs`：
- 1000-1999: LLM (连接失败/认证/限流/超时/Provider不可用等)
- 2000-2999: Agent (已运行/最大迭代/确认超时/Skill不存在/禁用/预算超限等)
- 3000-3999: 文档处理 (文件不存在/格式不支持/解析错误/Sidecar错误等)
- 4000-4999: 数据库 (连接/查询/记录不存在/约束冲突/迁移失败等)
- 5000-5999: 配置 (格式无效/字段缺失/Provider不存在等)
- 6000-6999: 文件系统 (路径不存在/权限/已存在/IO错误等)
- 7000-7999: 运行时 (事件发射错误)

### 应用设置
`AppSettings` 含以下子配置（JSON 文件存储）：
- `GeneralSettings`: 作者名、确认级别(Always/EditOnly/Never)、语言
- `TokenBudget`: 日限额/月限额/超额动作(Warn/Block/Fallback)
- `VersionSnapshot`: 保留策略(ByCount/ByDays/Both)、最大数量/天数
- `WorkspaceDefaults`: 默认工作区 ID
- `Shortcuts`: 快捷键配置（Ctrl+N/Ctrl+W/Ctrl+Enter/Ctrl+B/Ctrl+/）
- `disabled_skills`: 已禁用 Skill 列表

### 文件监听服务
- 基于 `notify` crate 的 `RecommendedWatcher`，递归监听工作区目录
- 文件变更时发射 `file:change` 事件到前端，用于实时刷新文件树
- 支持监听器切换（切换到新工作区时自动停止旧监听器）

### 状态管理 (前端)
- `useWorkflowStore`: 工作流节点列表、执行状态、确认回调
- `useSessionStore`: 会话 CRUD 和当前会话
- `useWorkspaceStore`: 工作区列表、当前工作区、切换逻辑
- `useSettingsStore`: 应用设置（弹窗开关等）
- `useFileTreeStore`: 文件树数据与加载状态
- `useTokenStore`: Token 用量统计与事件监听

### 数据存储
- SQLite: 会话、消息、版本快照、Token 统计
- JSON 文件: LLM Provider 配置、应用设置、工作区配置
- 文件系统: 工作区文档和 Sidecar 日志 (`log/docagent.log`)
- 应用数据目录: `<app_data_dir>/docagent.db` + `config/` 目录

### 日志系统
- 双输出: 控制台(stderr) + 日志文件(`log/docagent.log`)
- 开发模式(debug): DEBUG 级别；发布模式(release): INFO 级别
- 每次启动覆盖日志文件 (Create + Truncate)
- 日志文件创建失败时降级为仅控制台输出

## 提交规范

使用 Conventional Commits 格式:
```
<类型>(可选范围): <中文标题>

<可选的详细描述>

<可选的脚注>
```

类型: feat(新功能) / fix(修复) / docs(文档) / style(格式) / refactor(重构) / perf(性能) / test(测试) / chore(构建/工具/依赖)

## 关键约束

- 前端与 Rust 后端类型需手动同步（`shared/types.ts` 中的类型定义与 Rust models/events 保持一致）
- Tauri 命令名 `snake_case`，前端封装函数名 `camelCase`
- Rust 事件 payload 使用 `#[serde(rename_all = "camelCase")]`，前端直接接收 camelCase 字段
- Python Sidecar 的 `input_path` 要映射为 handler 期望的 `path` 参数
- Skill 的 `workspace_root` 由 executor 注入，不信任 LLM 提供的值，防止路径遍历攻击
