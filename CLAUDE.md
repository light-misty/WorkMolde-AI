# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概览

Samoyed Work 是一个基于 Tauri 2.x 的 AI 文档处理桌面应用。用户通过对话式 AI Agent 完成 Word/Excel/PPT/PDF/Markdown 文档的生成、读取、修改、格式转换等操作。

## 技术栈

- **桌面框架**: Tauri 2.x (Rust + React/TypeScript)，无边框窗口（decorations: false）
- **前端**: React 19 + TypeScript 5 + Vite 6 + Tailwind CSS 4
- **UI 组件**: Shadcn/ui + Radix 原语组件
- **状态管理**: Zustand 5
- **后端语言**: Rust 1.80+ (edition 2021)，Tokio 异步运行时；reqwest 使用 rustls-tls（非 native-tls）
- **数据库**: SQLite (rusqlite, bundled)
- **配置存储**: JSON 文件 (serde)
- **文档处理**: Python 3.12+ Sidecar (python-docx / openpyxl / python-pptx / PyMuPDF / reportlab / pdfminer.six / fpdf2 / pypdf / pdfplumber / Pillow)
- **Markdown 渲染**: react-markdown + remark-gfm + rehype-highlight
- **PDF 预览**: pdfjs-dist
- **差异对比**: diff 库
- **图表绘制**: recharts
- **国际化**: react-i18next + i18next（zh-CN / en-US）

## 构建与运行命令

```bash
# 开发模式（前端热更新 + Tauri 桌面窗口）
npm run tauri:dev
# （自动执行 pretauri:dev: sync_sidecar_dev.ps1）

# 仅启动前端开发服务器（浏览器访问 localhost:9527）
npm run dev

# 生产构建（自动执行 pretauri:build: build_sidecar.ps1 + sidecar:build）
npm run tauri:build

# TypeScript 类型检查 + Vite 构建
npm run build    # tsc -b && vite build

# 仅 TypeScript 类型检查（不生成产物）
npx tsc -b

# 预览生产构建
npm run preview

# 单独构建 Python Sidecar
npm run sidecar:build

# Python Sidecar 依赖安装
pip install -r sidecar/requirements.txt
```

环境变量 `SAMOYED_WORK_PYTHON` 可指定 Python 解释器路径。

### Rust 后端命令

```bash
# 编译（不运行）
cargo build -p samoyed_work_lib

# 运行所有 Rust 测试（现有 13 个 #[cfg(test)] 模块 + Python 测试）
cargo test

# 运行特定测试
cargo test <test_name>

# Lint 检查
cargo clippy

# 代码格式化检查
cargo fmt --check
```

### 其他有用命令

```bash
# 查看 Rust 依赖树
cargo tree

# 清理构建缓存
cargo clean
```

## 项目架构

```
src/                     React 前端 (TypeScript)
  components/
    layout/              布局组件: TopBar, MainArea, LeftSidebar, Sidebar,
                            InputArea, MainLayout, NetworkStatusBanner,
                            WorkspaceSelector, WindowControls
    workflow/            工作流时间线: WorkflowTimeline, WorkflowNode
                            (User/Thinking/Content/Tool/Confirm/Error)
    sidebar/             侧边栏: FileTreeSection, AgentInfoSection,
                            SessionListSection
    preview/             文档预览浮层: PreviewOverlay, MarkdownPreview,
                            PdfCanvasViewer, VersionHistoryPanel
    settings/            设置弹窗: SettingsDialog + 8 标签页
                            (LLMConfig, WorkspaceTab, HandlersTab, TemplatesTab,
                             AppearanceTab, ShortcutsTab, GeneralTab, HelpTab)
                            + 子弹窗 (ProviderFormDialog, AddWorkspaceDialog,
                              TemplateEditDialog)
    common/              通用组件: Button, Icon, ContextMenu, DeleteConfirmDialog,
                            ErrorBoundary, ProviderSelector, Toast(ToastContainer),
                            UpdateNotification
  stores/                Zustand stores: workflow, session, settings, workspace,
                            fileTree, toast, attachment, network, update
  i18n/                  国际化: index.ts, locales/zh-CN.json, locales/en-US.json
  services/              前端服务层: tauri.ts (invoke封装), event.ts (事件监听),
                            errorHandler.ts
  hooks/                 useAgent.ts (Agent交互核心hook)
  types/                 TypeScript类型定义 (与Rust后端对齐)
  utils/                 工具: fileIcons.tsx, format.ts, logger.ts
  styles/                globals.css

src-tauri/               Rust 后端
  tauri.conf.json        Tauri 配置 (无边框窗口, CSP, 构建命令)
  capabilities/          Tauri 权限配置 (shell, dialog 等插件权限)
  src/
    lib.rs               入口, AppState定义 (含 network_monitor), 命令注册,
                           初始化流程
    commands/            Tauri命令层 (12个模块): llm, session, workspace, document,
                            handler, settings, agent, template, log, lsp, update (desktop)
    services/
      agent/             Agent调度引擎: executor, context (对话上下文管理)
        prompts/         System Prompt: document_design, prompt_loader,
                            task_type, token_budget
      llm/               LLM多Provider适配: router, provider (trait),
                            openai_adapter, anthropic_adapter, gemini_adapter,
                            context_presets (30+模型上下文窗口预设表)
      handler/           Handler引擎: registry (注册表), builtin (文档处理器)
      tool/              Tool引擎: registry, builtin (16个工具), trait_def
      document/          Python Sidecar进程管理 (自动重启、超时、重试)
      attachment.rs      文件附件处理
      network_monitor.rs 网络状态监控
      fs_watcher.rs      文件系统监听
    db/                  数据库层: init, session_repo, message_repo, snapshot_repo,
                            template_repo, session_summary_repo, user_preference_repo
    config/              配置管理: app_settings, llm_config, workspace_config
    models/              数据模型: message, session, document, llm, handler,
                            workspace, template, tool, context_memory
    events/              事件系统: types, emitter
    utils/               工具: logger (双输出日志)
    errors.rs            统一错误码

sidecar/                 Python 文档处理引擎
  main.py                stdin/stdout JSON 行协议入口
  handlers/              文档处理器: word, excel, ppt, pdf, markdown,
                            font_utils, validator (文档验证)
  tests/                 test_skills_integration.py (645行集成测试)

shared/                  前后端共享TypeScript类型
  types.ts

docs/                    详细开发文档
  tech_architecture.md, tauri_commands.md, database_design.md,
  handler_development.md, component_design.md, task_breakdown.md,
  PRD_Samoyed-Work.md,
  plans/                  设计文档 (10+ 设计文档)
  tests/                  e2e_test.md, tools_handlers_validation.md
  prototypes/             samoyed-work-prototype.html
```

## 核心架构要点

### 前后端通信
- **`invoke()`**: 请求-响应式调用（查询数据、操作触发），命令名 `snake_case`
- **`emit()/listen()`**: 事件推送（Agent流式输出、进度更新、需确认操作等），事件名 `namespace:action`
- Agent 事件: `agent:thinking`, `agent:deep_thinking`, `agent:content`, `agent:tool_call`, `agent:tool_result`, `agent:confirm`, `agent:context_update`, `agent:network_retry`, `agent:done`, `agent:error`, `agent:stopped`, `agent:compaction_start`, `agent:compaction_done`, `agent:sub_agent_status`, `agent:sub_agent_tool_call`, `agent:question`
- 系统事件: `session:updated`, `workspace:directory_deleted`, `file:change`, `llm:provider_switch`, `system:network_change`

### Agent 执行流程
1. 前端 `useAgent` hook 调用 `start_agent` 命令
2. Rust `AgentExecutor` 构建上下文（System Prompt + Handler/Tool Definitions + 历史消息）
3. 循环: 调用 LLM (流式) → 解析响应 → 执行 Tool Calling (Handler/Tool) → 返回结果给 LLM
4. 高风险操作（删除/修改/批量）需用户确认，通过 `confirm_channels` oneshot channel 同步等待（5分钟超时）
5. 每轮迭代后增量持久化消息到 SQLite，防止崩溃丢失
6. Handler/Tool 执行时短暂持锁获取 Arc 引用后立即释放，避免阻塞注册表
7. 支持用户手动停止（`stop_agent`），通过 `should_stop` 闭包检查；停止时状态流转变为 `stopping` → `cancelled`
8. 子 Agent 委托（阶段 4）: 主 Agent 可通过 `task` 工具委托子任务给独立子 Agent 执行，子 Agent 拥有独立上下文窗口，继承父 Agent 的 AgentMode、系统提示词和工作区配置；子 Agent 嵌套深度限制为 3 层，默认不允许递归调用 Task 工具；子 Agent 工具执行受权限系统控制（Ask 视为 Allow，因子 Agent 无法与用户交互）

### LLM Provider 系统
- **OpenAI 适配器**: 兼容 OpenAI API 格式的 Provider（含 Ollama、兼容第三方）
- **Anthropic 适配器**: 原生 Anthropic Messages API
- **Gemini 适配器**: 原生 Gemini API
- **LlmRouter**: 管理多个 Provider，支持默认选择、顺序 Fallback、健康检查（5分钟自动恢复）、延迟 EMA 追踪
- Provider 配置含 `context_window`（上下文窗口大小，自动推断）、`supports_vision`（是否支持图片多模态）、`extra_params`（扩展参数）
- Provider 类型 (前端): `openai | anthropic | ollama | gemini | custom`；Rust 端以 String 存储，兼容更多类型
- `context_presets.rs` 内置 30+ 模型家族的默认上下文窗口预设表（OpenAI/Anthropic/Gemini/DeepSeek/Llama/Qwen/Kimi/GLM/ERNIE/Doubao/MiniMax/Yi/Baichuan/Spark/Mistral/Hunyuan 等）
- 每 5 分钟后台自动执行健康检查，自动标记不可用 Provider；Provider 切换时发射 `llm:provider_switch` 事件

### Handler 系统（文档处理，始终启用）
- 每个 Handler 实现 `Handler` trait: `handler_name()`, `description()`, `parameters()` (JSON Schema), `execute()`
- 内置 4 个文档类型 Handler（均通过 Python Sidecar 执行）:
  - `docx_handler`: Word 文档处理（读取/转换/分析）
  - `xlsx_handler`: Excel 文档处理（读取/转换/分析）
  - `pptx_handler`: PPT 文档处理（读取/转换/分析）
  - `pdf_handler`: PDF 文档处理（读取/转换/分析）
- Handler 始终启用，前端 HandlersTab 仅展示信息

### Tool 系统（基础操作，始终启用）
- Tool 是轻量级、始终启用的基础操作工具，与 Handler 平行但不可禁用
- 每个 Tool 实现 `Tool` trait（与 Handler 相似的接口: `tool_name()`, `description()`, `parameters()`, `execute()`）
- 内置 24 个 Tool（纯 Rust 实现，不依赖 Python Sidecar）（实验性开关开启时为 25 个）:
  - `list_directory`: 列出目录内容（支持深度控制、扩展名过滤、排序，含路径遍历安全校验）
  - `search_files`: 按文件名/内容搜索文件（支持扩展名过滤、内容预览）
  - `read_file`: 读取纯文本文件（.txt/.md/.csv/.json 等，1MB 上限，含路径校验）
  - `file_info`: 获取文件元数据（大小、修改时间、类型分类）
  - `file_exists`: 检查文件/目录是否存在
  - `delete_file`: 删除文件（可选备份，强制工作区路径校验）
  - `create_directory`: 创建目录（支持递归创建）
  - `write_text_file`: 写入纯文本文件（支持追加模式，自动创建父目录）
  - `rename_file`: 重命名/移动文件
  - `copy_file`: 复制文件
  - `delete_directory`: 删除目录（可选递归）
  - `get_file_hash`: 计算文件 SHA-256 哈希
  - `edit`: 精确字符串替换工具（阶段 1 编程 Agent 改造，支持单次/全部替换）
  - `glob`: glob 模式查找工具（阶段 1 编程 Agent 改造）
  - `grep`: 正则表达式搜索工具（阶段 1 编程 Agent 改造，基于 ripgrep）
  - `scratchpad`: 智能体草稿本（按 session_id 隔离的笔记工具，支持写入/读取/清空/刷新摘要，每轮迭代自动注入摘要）
  - `write_script`: 将智能体生成的脚本写入系统临时目录 `<temp_dir>/samoyed_work/scripts/`
  - `run_command`: 通过 Git Bash 执行命令（运行脚本），支持工作目录和超时控制（LLM 通过 timeout 参数自主控制，最大 300 秒）；高风险命令（rm -rf、format、shutdown 等）需用户确认；Git Bash 路径优先使用用户配置，为空时从 PATH 自动检测（先查找 bash.exe，再从 git.exe 推断 `<git_root>/bin/bash.exe`）
  - `todo_write`: 结构化任务管理（按 session_id 隔离并持久化到数据库）
  - `source_code`: 基于 tree-sitter 的代码语义搜索（支持按符号类型和名称通配符查询）
  - `task`: 委托子任务给子 Agent 执行（阶段 4，支持 single/batch 模式，子 Agent 拥有独立上下文，继承父 Agent 配置；嵌套深度限制 3 层，默认禁止递归调用）
  - `webfetch`: 获取 URL 内容并转为 Markdown（阶段 4，受权限系统控制，URL 验证拒绝内网地址和非 HTTP 协议）
  - `websearch`: 网络搜索（阶段 4，支持 MCP/Tavily/SerpAPI 后端，受权限系统控制）
  - `question`: 向用户提问并等待回答（阶段 4，通过 AGENT_QUESTION 事件推送问题，前端通过 submit_question_answer 命令回复，5 分钟超时）
  - `lsp`: LSP 代码智能工具（实验性，需 `lsp.experimental_enabled=true` 开启），通过 `operation` 参数路由 8 种操作：
    - `definition`: 跳转到符号定义
    - `references`: 查找符号引用
    - `hover`: 获取符号悬停信息（类型、文档）
    - `diagnostics`: 获取文件诊断信息
    - `document_symbol`: 获取文档符号列表
    - `workspace_symbol`: 搜索工作区符号
    - `implementation`: 跳转到实现
    - `call_hierarchy`: 获取调用层级（direction=incoming|outgoing）
- Tool 注册在 `ToolRegistry` 中，通过 `Arc<dyn Tool>` 共享访问
- 共同路径安全机制：所有文件操作通过 executor 注入 `workspace_root`，拒绝路径遍历攻击
- TaskTool 采用延迟注入模式：先注册不含 SubAgentExecutor 的实例，后续在 lib.rs setup 中通过 `set_sub_executor` 注入（解决 TaskTool ↔ SubAgentExecutor 循环依赖）

### Scratchpad 系统
- 全局唯一 `SharedScratchpadStates`（`Arc<RwLock<HashMap<String, ScratchpadState>>>`），按 session_id 隔离
- ScratchpadTool 持有写权限，AgentContext 持有读权限（每轮迭代刷新摘要注入上下文）
- 设计参考 Anthropic《Effective Context Engineering for AI Agents》的 "Structured Note-taking" 模式
- 替代外部硬编码迭代元数据注入，让 Agent 自主管理笔记

### 情景记忆系统
- `ContextSessionSummary` 模型：记录会话的用户目标、执行摘要、涉及文件、使用工具、错误及解决方案
- `UserPreference` 模型：记录用户的偏好信息（格式偏好、命名习惯等）
- 对应数据库表 `session_summaries` 和 `user_preferences`
- Agent 执行完成时自动生成摘要，新会话启动时可检索同工作区的历史摘要注入上下文

### 文档验证系统
- `sidecar/handlers/validator.py` 中的 `DocumentValidator` 类
- 在文档生成/修改后执行质量检查，检测常见问题
- 支持 Word/Excel/PPT/PDF 文档的验证
- 返回验证结果（警告列表），供 LLM 决定是否需要修正

### AppState 全局状态
```
AppState {
    db: Arc<Database>,
    config: Arc<Mutex<ConfigManager>>,
    active_agents: Arc<Mutex<HashMap<String, bool>>>,
    confirm_channels: Arc<Mutex<HashMap<String, oneshot::Sender<ConfirmDecision>>>>,
    permission_channels: Arc<Mutex<HashMap<String, oneshot::Sender<PermissionDecision>>>>,
    question_channels: QuestionChannels,
    permission_registry: Arc<PermissionRegistry>,
    session_whitelist: Arc<SessionWhitelist>,
    doom_loop_detector: Arc<DoomLoopDetector>,
    agent_mode_manager: Arc<AgentModeManager>,
    doc_service: Arc<DocumentService>,
    llm_router: Arc<RwLock<Arc<LlmRouter>>>,
    tool_registry: Arc<ToolRegistry>,
    sub_executor: Arc<SubAgentExecutor>,
    handler_registry: Arc<Mutex<HandlerRegistry>>,
    fs_watcher: Arc<FsWatcherService>,
    network_monitor: Arc<NetworkMonitor>,
    scratchpad_states: SharedScratchpadStates,
    skill_registry: Arc<SkillRegistry>,
    lsp_manager: Arc<LspServerManager>,
}
```
- `tool_registry` 在运行时不变，无需 Mutex 保护；`handler_registry` 使用 Mutex 保护运行时注册访问
- 锁获取原则：Handler 执行时短暂持锁获取 `Arc` 引用后立即释放，避免阻塞注册表
- `sub_executor`（阶段 4）在 setup 中创建，通过延迟注入模式注入到 TaskTool（解决循环依赖）

### 前端组件要点
- **懒加载**: PreviewOverlay、SettingsDialog 通过 `React.lazy` 延迟加载，减少首屏体积
- **PDF 预览**: `PdfCanvasViewer` 使用 `pdfjs-dist` 在 Canvas 上渲染，支持缩放(0.5x-3x)、翻页、自适应宽度/页面模式
- **Toast 通知**: 全局 `ToastContainer` 渲染在固定右上角，3 秒自动消失，支持 error/success/warning 三种类型，最大 5 条同时显示
- **错误边界**: `ErrorBoundary` 包裹应用根组件，捕获渲染异常，提供"恢复页面"和"重启应用"操作
- **虚拟滚动**: 工作流时间线/文件树等长列表使用虚拟滚动优化大量节点渲染性能
- **文件图标**: `fileIcons.tsx` 提供按文件扩展名映射的 SVG 图标集合

### Python Sidecar 通信协议
- stdin/stdout JSON 行协议，SidecarManager 自动管理进程生命周期（启动、停止、崩溃时自动重启）
- 请求: `{"id": "...", "action": "read|convert|analyze|execute|ping|validate", "type": "docx|xlsx|pptx|pdf|md|txt", "params": {...}}`
- 响应: `{"id": "...", "success": true|false, "data": {...}, "error": "..."}`
- 默认请求超时 120 秒，超时后自动重启 Sidecar 进程并重试一次

### 后台健康检查
- LLM Provider 健康检查: 每 5 分钟执行一次 `health_check_all()`，自动标记不可用 Provider；切换时发射 `llm:provider_switch` 事件
- Sidecar 健康检查: 每 3 分钟执行一次，不健康时记录警告日志
- 网络状态监控: `NetworkMonitor` 定时检测网络连通性，状态变化时发射 `system:network_change` 事件；断网时自动暂停 LLM 请求并在恢复后重试

### 文档设计提示
`src-tauri/src/services/agent/prompts/document_design.rs` 包含专业的文档生成规范（作为 System Prompt 注入 Agent），覆盖：
- Word: 页面尺寸 DXA 计算、EMU 单位、样式规范、表格/列表/图片/页眉页脚规范
- Excel/PPT/PDF 类似的结构化设计指导

### 错误码体系
统一通过 `CommandError` 结构体（定义在 `errors.rs`）返回给前端，结构为 `{ code: u32, message: String }`。Rust 标准错误类型通过 `From` trait 自动转换：
- `rusqlite::Error` → DB 错误 (4000)
- `reqwest::Error` → LLM 错误 (1000)，按 timeout/connect/status 细化
- `serde_json::Error` → 配置错误 (5000)
- `std::io::Error` → 文件系统错误 (6000)，按 kind 映射
- `tauri::Error` → 运行时错误 (7000)

错误码按模块分段：
- 1000-1999: LLM (连接失败/认证/限流/超时/Provider不可用等)
- 2000-2999: Agent (已运行/最大迭代/确认超时/Handler不存在等)
- 3000-3999: 文档处理 (文件不存在/格式不支持/解析错误/Sidecar错误等)
- 4000-4999: 数据库 (连接/查询/记录不存在/约束冲突/迁移失败等)
- 5000-5999: 配置 (格式无效/字段缺失/Provider不存在等)
- 6000-6999: 文件系统 (路径不存在/权限/已存在/IO错误等)
- 7000-7999: 运行时 (事件发射错误)
- 8000-8999: 更新 (检查/下载/安装失败)
- 9000-9999: 工具/处理器 (不存在/参数无效/执行失败/路径越界)

### 附件系统与多模态支持
- `attachment.rs` 负责处理用户上传的附件文件（图片/文档/文本）
- 支持的图片 MIME 类型: PNG/JPEG/GIF/WebP，自动 base64 编码后通过 `ContentPart::Image` 传递给支持视觉的 LLM
- 支持的文本 MIME 类型: TXT/Markdown/CSV/HTML/JSON/XML/YAML/TOML/INI/Log 等
- 前端 `useAttachmentStore` 管理待发送附件，支持工作区内（相对路径）和工作区外（绝对路径）文件
- `AttachmentMeta` 类型包含文件路径、文件名、MIME 类型、大小和 base64 数据
- Provider 通过 `supports_vision` 字段标识是否支持图片多模态

### 国际化 (i18n)
- 基于 `react-i18next` + `i18next`，支持中文（zh-CN）和英文（en-US）
- 语言配置存储在 `localStorage`（key: `i18n-language`），默认为中文
- 翻译文件位于 `src/i18n/locales/`（zh-CN.json / en-US.json），各约 500+ 翻译键
- 语言切换在 AppearanceTab 的 `language` 和 `languageFollowSystem` 设置项

### 网络监控
- `NetworkMonitor`（Rust 端）定时检测网络连通性，状态变化时发射 `system:network_change` 事件（含 current/previous 状态）
- 前端 `NetworkStatusBanner` 组件显示断网横幅，恢复时短暂显示"已恢复"提示（3秒自动消失）
- `useNetworkStore` 同步前端网络状态，Agent 可在断网时暂停并自动重试 LLM 请求

### 应用设置
`AppSettings` 含以下子配置（JSON 文件存储），前端 SettingsDialog 含 8 个标签页：
- `GeneralSettings`: 作者名、作者邮箱、作者公司、确认级别(Always/DeleteOnly/Never)、`git_bash_path`（String，空表示自动检测）→ **GeneralTab**（含"代码执行环境"区域）
- `AppearanceSettings`: 主题模式(light/dark/system)、界面语言(language)、跟随系统语言(languageFollowSystem) → **AppearanceTab**
- `VersionSnapshot`: 保留策略(ByCount/ByDays/Both)、最大数量/天数
- `WorkspaceDefaults`: 默认工作区 ID → **WorkspaceTab**
- `Shortcuts`: 快捷键配置（newSession/closeSession/sendMessage/toggleSidebar/quickPrompt）→ **ShortcutsTab**
- `UpdateSettings`: 自动检查更新(autoCheck) → 与 **GeneralTab** 关联
- LLM Provider 配置管理 → **LLMConfig**（含 ProviderFormDialog 子弹窗，Provider 支持 contextWindow/supportsVision/extraParams）
- Prompt 模板管理 → **TemplatesTab**（含 TemplateEditDialog 子弹窗，支持带变量的 Prompt 模板）

### 文件监听服务
- 基于 `notify` crate 的 `RecommendedWatcher`，递归监听工作区目录
- 文件变更时发射 `file:change` 事件到前端，用于实时刷新文件树
- 支持监听器切换（切换到新工作区时自动停止旧监听器）

### 状态管理 (前端)
- `useWorkflowStore`: 工作流节点列表、执行状态、确认回调、迭代分组
- `useSessionStore`: 会话 CRUD 和当前会话（含附件列表）
- `useWorkspaceStore`: 工作区列表、当前工作区、切换逻辑
- `useSettingsStore`: 应用设置（弹窗开关等）
- `useFileTreeStore`: 文件树数据与加载状态
- `useToastStore`: Toast 通知队列管理（error/success/warning，自动消失）
- `useAttachmentStore`: 当前待发送附件管理（添加/移除/清空）
- `useNetworkStore`: 网络状态跟踪（online/offline，前端状态同步）
- `useUpdateStore`: 更新状态管理（待安装更新包路径）

### 数据存储
- SQLite: 会话、消息、版本快照、Prompt 模板、会话摘要、用户偏好
- JSON 文件: LLM Provider 配置、应用设置、工作区配置
- 文件系统: 工作区文档和 Sidecar 日志 (`log/samoyed_work.log`)
- 应用数据目录: `<app_data_dir>/samoyed_work.db` + `config/` 目录

### 日志系统
- 双输出: 控制台(stderr) + 日志文件(`log/samoyed_work.log`)
- 开发模式(debug): DEBUG 级别；发布模式(release): INFO 级别
- 每次启动覆盖日志文件 (Create + Truncate)
- 日志文件创建失败时降级为仅控制台输出

### Tauri 安全与权限
- 无边框窗口 (`decorations: false`)，自定义窗口控件
- CSP 限制严格: 仅允许 `http://localhost:*` 和 `http://127.0.0.1:*` 的 connect-src（用于 LLM API 调用）
- 使用 `capabilities/` 目录配置插件权限（shell、dialog 等）
- Tauri 插件: `tauri-plugin-shell`, `tauri-plugin-dialog`；桌面端额外注册 `tauri-plugin-updater` + `tauri-plugin-process`
- 40+ 注册命令覆盖 LLM 管理、会话 CRUD、工作区操作、文档处理、Handler 管理、工具管理、设置、模板 CRUD、日志读取、DevTools 切换、更新检查/安装等

### 自动更新
- 通过 `tauri-plugin-updater` 实现自动更新，NSIS 安装器打包
- `#[cfg(desktop)]` 条件编译更新命令和插件，仅桌面平台可用
- 更新产物签名验证，公钥和端点配置在 `tauri.conf.json` 的 `plugins.updater` 中

### 开发构建脚本
- `scripts/sync_sidecar_dev.ps1`: 开发模式下同步 Sidecar 源码到 sidecar_dist（pretauri:dev 自动执行）
- `scripts/build_sidecar.ps1`: 构建 Python Sidecar 为可分发包（PyInstaller 打包）

## TypeScript 路径别名

配置在 `tsconfig.json` 中:
```json
{
  "paths": {
    "@/*": ["src/*"]
  }
}
```
示例: `import { Button } from "@/components/common/Button"`

## 开发文档参考

`docs/` 目录包含详细的开发规范文档，在实现相关功能前应优先查阅:
- `tech_architecture.md` — 完整技术架构与技术选型理由
- `tauri_commands.md` — 所有 Tauri 命令、事件、错误码的完整接口规范
- `handler_development.md` — Handler 接口规范与开发指南
- `database_design.md` — 数据库表结构与迁移策略
- `component_design.md` — 前端组件层级与交互设计
- `task_breakdown.md` — 阶段任务分解与进度
- `PRD_Samoyed-Work.md` — 产品需求文档
- `plans/` — 设计文档 (上下文窗口设计、LLM 缓存优化等)
- `tests/e2e_test.md` — E2E 测试计划
- `tests/tools_handlers_validation.md` — Tools/Handlers 验证方案

## 提交规范

使用 Conventional Commits 格式:
```
<类型>(可选范围): <中文标题>

<可选的详细描述>

<可选的脚注>
```
类型: feat(新功能) / fix(修复) / docs(文档) / style(格式) / refactor(重构) / perf(性能) / test(测试) / chore(构建/工具/依赖)

## 关键约束

- 前端与 Rust 后端类型需手动同步（`shared/types.ts` 中的基础类型 + `src/types/` 下的各模块类型，与 Rust `models/` 目录对齐）
- Provider 类型含 `contextWindow`、`supportsVision`（图片多模态）、`extraParams`（扩展参数）字段
- Tauri 命令名 `snake_case`，前端封装函数名 `camelCase`（见 `src/services/tauri.ts`）
- Rust 事件 payload 使用 `#[serde(rename_all = "camelCase")]`，前端直接接收 camelCase 字段
- Python Sidecar 的 `input_path` 要映射为 handler 期望的 `path` 参数
- Handler/Tool 的 `workspace_root` 由 executor 注入，不信任 LLM 提供的值，防止路径遍历攻击
- 文档预览: 普通文件返回文本 `PreviewContent`，PDF 文件通过 `get_pdf_data` 返回 base64 数据由前端 `PdfCanvasViewer` 渲染
- 版本历史: `VersionHistoryPanel` 组件展示文档版本快照列表，支持版本对比（diff）和回滚操作
- 所有文件操作（创建/删除/重命名）通过 Tauri 命令在 Rust 端执行，前端不直接操作文件系统
- 命令超时由 LLM 通过 run_command 的 `timeout` 参数自主控制，最大 300 秒（无全局超时配置）
- 应用初始化顺序: 应用数据目录 → 日志系统 → 数据库（含损坏检测+自动重建） → 配置管理器 → LLM Config → LLM Router → Sidecar → Handler 注册表 + builtin handlers → 权限系统组件（permission_registry/session_whitelist/doom_loop_detector/agent_mode_manager）→ Tool 注册表 + builtin tools（读取 `git_bash_path` 和 `web_search` 配置后传入 `register_builtin_tools`，含 task/webfetch/websearch/question 工具）→ SubAgentExecutor 创建并通过 `set_sub_executor` 延迟注入 TaskTool → Skill 注册表 → LSP 服务器管理器/路由器/缓存（阶段 5，读取 `lsp` 配置后初始化，注册服务器配置并传入 `register_builtin_tools`，仅在 `lsp.experimental_enabled=true` 时注册 LspTool；启动 LSP 健康检查后台任务） → AppState 注册 → FS 监听器 → 网络状态监控器 → 后台健康检查任务（LLM 每5分钟、Sidecar 每3分钟、网络监控、LSP 按配置间隔）
- 应用安装了自定义 panic hook，将 panic 信息记录到日志文件并尝试发射 `runtime:error` 事件到前端
