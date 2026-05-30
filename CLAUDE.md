# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概览

DocAgent 是一个基于 Tauri 2.x 的 AI 文档处理桌面应用。用户通过对话式 AI Agent 完成 Word/Excel/PPT/PDF/Markdown 文档的生成、读取、修改、格式转换等操作。

## 技术栈

- **桌面框架**: Tauri 2.x (Rust + React/TypeScript)，无边框窗口（decorations: false）
- **前端**: React 19 + TypeScript 5 + Vite 6 + Tailwind CSS 4
- **UI 组件**: Shadcn/ui + Radix 原语组件
- **状态管理**: Zustand 5
- **后端语言**: Rust 1.80+ (edition 2021)，Tokio 异步运行时；reqwest 使用 rustls-tls（非 native-tls）
- **数据库**: SQLite (rusqlite, bundled)
- **配置存储**: JSON 文件 (serde)
- **文档处理**: Python 3.12+ Sidecar (python-docx / openpyxl / python-pptx / PyMuPDF / reportlab / pdfminer.six)
- **Markdown 渲染**: react-markdown + remark-gfm + rehype-highlight
- **PDF 预览**: pdfjs-dist
- **代码高亮**: Shiki
- **数学公式**: KaTeX

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

# 仅 TypeScript 类型检查（不生成产物）
npx tsc -b

# 预览生产构建
npm run preview

# Python Sidecar 依赖安装
pip install -r sidecar/requirements.txt
```

环境变量 `DOCAGENT_PYTHON` 可指定 Python 解释器路径。

### Rust 后端命令

```bash
# 编译（不运行）
cargo build -p docagent_lib

# 运行所有 Rust 测试
cargo test

# 运行特定测试
cargo test <test_name>

# Lint 检查
cargo clippy

# 代码格式化检查
cargo fmt --check
```

注意：目前项目**尚无前端测试和后端测试**，`cargo test` 无可运行测试用例。编写新功能时需注意手工验证。

### 其他有用命令

```bash
# 查看 Rust 依赖树
cargo tree

# 清理构建缓存
cargo clean
```

## 项目架构

```
┌─ src/                  React 前端 (TypeScript)
│  ├─ components/
│  │  ├─ layout/         布局组件: TopBar, MainArea, Sidebar, InputArea, WindowControls
│  │  ├─ workflow/       工作流时间线: WorkflowTimeline, WorkflowNode (User/Thinking/Tool/Result/Reply/Confirm)
│  │  ├─ sidebar/        右侧栏: FileTreeSection, AgentInfoSection, TodoSection
│  │  ├─ preview/        文档预览浮层: PreviewOverlay, MarkdownPreview, PdfCanvasViewer (pdfjs-dist canvas 渲染)
│  │  ├─ settings/       设置弹窗: SettingsDialog + 8 个标签页 (LLMConfig, WorkspaceTab, SkillsTab,
│  │  │                      TemplatesTab, AppearanceTab, ShortcutsTab, GeneralTab, HelpTab)
│  │  │                      + 子弹窗 (ProviderFormDialog, AddWorkspaceDialog, TemplateEditDialog,
│  │  │                        CustomSkillDialog)
│  │  ├─ session/        历史会话面板: HistoryPanel
│  │  └─ common/         通用组件: Button, Icon, ContextMenu, DeleteConfirmDialog, ErrorBoundary,
│  │                        ToastContainer, TemplatePicker
│  ├─ stores/            Zustand stores: workflow, session, settings, workspace, fileTree, toast
│  ├─ services/          前端服务层: tauri.ts (invoke封装), event.ts (事件监听/类型定义)
│  ├─ hooks/             useAgent.ts (Agent交互核心hook: sendMessage, stopAgent, confirmOperation 等)
│  └─ types/             TypeScript类型定义 (与Rust后端对齐: workflow, session, workspace, document, settings)
│
├─ src-tauri/            Rust 后端
│  ├─ tauri.conf.json    Tauri 配置 (无边框窗口, CSP, 构建命令)
│  ├─ capabilities/      Tauri 权限配置 (shell, dialog 等插件权限)
│  ├─ src/
│  │  ├─ lib.rs          入口, AppState定义, 命令注册(全部30+命令), 初始化流程
│  │  ├─ commands/       Tauri命令层 (10个模块): agent, document, llm, log, session,
│  │  │                       settings, skill, template, workspace
│  │  ├─ services/       业务逻辑层
│  │  │  ├─ agent/       Agent调度引擎: executor (Tool Calling循环), context (对话上下文管理)
│  │  │  ├─ llm/         LLM多Provider适配: router, provider (trait),
│  │  │  │                  openai_adapter, anthropic_adapter, gemini_adapter
│  │  │  ├─ skill/       Skill引擎: registry (注册表+禁用管理), builtin, custom (自定义Skill加载器)
│  │  │  ├─ document/    Python Sidecar进程管理 (自动重启、超时、重试)
│  │  │  └─ fs_watcher/  文件系统监听 (notify crate, 递归监听+事件发射)
│  │  ├─ db/             数据库层: init (迁移), session/message/snapshot/template repo
│  │  ├─ config/         配置管理: app_settings, llm_config, workspace_config, ConfigManager
│  │  ├─ models/         数据模型: message, session, document, llm, skill, workspace, template
│  │  ├─ events/         事件系统: types (事件名常量+payload), emitter (事件发射封装)
│  │  ├─ utils/          工具: logger (双输出日志: 文件+stderr)
│  │  └─ errors.rs       统一错误码 (按模块分段: LLM/Agent/Doc/DB/Config/FS/Runtime)
│  └─ Cargo.toml
│
├─ sidecar/              Python 文档处理引擎
│  ├─ main.py            stdin/stdout JSON 行协议入口
│  └─ handlers/          文档处理器: word, excel, ppt, pdf, markdown, font_utils
│
├─ shared/               前后端共享TypeScript类型 (需与Rust端手动同步)
│  └─ types.ts
│
└─ docs/                 详细开发文档
   ├─ tech_architecture.md     技术架构
   ├─ tauri_commands.md        Tauri命令/事件接口规范
   ├─ database_design.md       数据库设计
   ├─ skill_development.md     Skill开发规范
   ├─ component_design.md      前端组件设计
   ├─ task_breakdown.md        任务分解
   └─ e2e_test_plan.md         E2E测试计划
```

## 核心架构要点

### 前后端通信
- **`invoke()`**: 请求-响应式调用（查询数据、操作触发），命令名 `snake_case`
- **`emit()/listen()`**: 事件推送（Agent流式输出、进度更新、需确认操作等），事件名 `namespace:action`
- Agent 事件: `agent:thinking`, `agent:content`, `agent:tool_call`, `agent:tool_result`, `agent:confirm`, `agent:todo_update`, `agent:done`, `agent:error`, `agent:stopped`
- 系统事件: `session:updated`, `workspace:change`, `file:change`, `llm:provider_switch`（各事件均有对应 Payload 结构体）

### Agent 执行流程
1. 前端 `useAgent` hook 调用 `start_agent` 命令
2. Rust `AgentExecutor` 构建上下文（System Prompt + Skill Tool Definitions + 历史消息）
3. 循环: 调用 LLM (流式) → 解析响应 → 执行 Tool Calling (Skill) → 返回结果给 LLM
4. 高风险操作（删除/修改/批量）需用户确认，通过 `confirm_channels` oneshot channel 同步等待（5分钟超时）
5. 每轮迭代后增量持久化消息到 SQLite，防止崩溃丢失
6. Skill 执行时短暂持锁获取 Arc 引用后立即释放，避免阻塞注册表
7. 支持用户手动停止（`stop_agent`），通过 `should_stop` 闭包检查；停止时状态流转变为 `stopping` → `cancelled`

### LLM Provider 系统
- **OpenAI 适配器**: 兼容 OpenAI API 格式的 Provider（含 Ollama、兼容第三方）
- **Anthropic 适配器**: 原生 Anthropic Messages API
- **Gemini 适配器**: 原生 Gemini API
- **LlmRouter**: 管理多个 Provider，支持默认选择、顺序 Fallback、健康检查（5分钟自动恢复）、延迟 EMA 追踪
- Provider 类型 (前端): `openai | anthropic | ollama | custom`；Rust 端以 String 存储，兼容更多类型
- 每 5 分钟后台自动执行健康检查，自动标记不可用 Provider；Provider 切换时发射 `llm:provider_switch` 事件

### Skill 系统（文档处理，可禁用）
- 每个 Skill 实现 `Skill` trait: `skill_name()`, `description()`, `parameters()` (JSON Schema), `execute()`
- 内置 6 个文档处理 Skill（均通过 Python Sidecar 执行）:
  - `generate_document`: 生成 docx/xlsx/pptx/pdf/md（含 Excel 公式/条件格式、PPT 颜色方案/字体、PDF 水印/密码等）
  - `read_document`: 读取结构化文档内容，支持格式信息
  - `modify_document`: 修改文档（替换/添加段落/表格/书签/超链接/页眉页脚/目录等 30+ 操作类型）
  - `convert_format`: 格式转换（docx/pdf/md/txt/csv/html 等互转）
  - `analyze_document`: 分析文档结构和统计信息
  - `batch_process`: 批量处理（批量转换/修改/分析）
- Skill 可禁用/启用，前端 SkillsTab 管理；自定义 Skill 通过 JSON 文件加载
- 自定义 Skill 本质是 Prompt 模板（支持 `{{param_name}}` 占位符），LLM 调用时参数替换后渲染文本返回给 LLM

### Tool 系统（文件系统操作，始终启用）
- Tool 是轻量级、始终启用的基础文件系统操作，与 Skill 平行但不可禁用
- 每个 Tool 实现 `Tool` trait（与 Skill 相似的接口: `tool_name()`, `description()`, `parameters()`, `execute()`）
- 内置 8 个 Tool（纯 Rust 实现，不依赖 Python Sidecar）:
  - `list_directory`: 列出目录内容（支持深度控制、扩展名过滤、排序，含路径遍历安全校验）
  - `search_files`: 按文件名/内容搜索文件（支持扩展名过滤、内容预览）
  - `read_file`: 读取纯文本文件（.txt/.md/.csv/.json 等，1MB 上限，含路径校验）
  - `write_text_file`: 写入纯文本文件（支持追加模式，自动创建父目录）
  - `delete_file`: 删除文件（可选备份，强制工作区路径校验）
  - `file_info`: 获取文件元数据（大小、修改时间、类型分类）
  - `file_exists`: 检查文件/目录是否存在
  - `create_directory`: 创建目录（支持递归创建）
- Tool 注册在 `ToolRegistry` 中，通过 `Arc<dyn Tool>` 共享访问
- 共同路径安全机制：所有文件操作通过 executor 注入 `workspace_root`，拒绝路径遍历攻击

### AppState 全局状态
```
AppState {
    db: Arc<Database>,
    config: Arc<Mutex<ConfigManager>>,
    active_agents: Arc<Mutex<HashMap<String, bool>>>,
    confirm_channels: Arc<Mutex<HashMap<String, oneshot::Sender<ConfirmDecision>>>>,
    doc_service: Arc<DocumentService>,
    llm_router: Arc<RwLock<Arc<LlmRouter>>>,
    tool_registry: Arc<ToolRegistry>,                    // 工具（始终启用，不加 Mutex）
    skill_registry: Arc<Mutex<SkillRegistry>>,           // 技能（可禁用，需 Mutex）
    custom_skill_loader: Arc<CustomSkillLoader>,
    fs_watcher: Arc<FsWatcherService>,
}
```
- `tool_registry` 在运行时不变，无需 Mutex 保护；`skill_registry` 因运行时禁用/启用需 Mutex
- 锁获取原则：Skill 执行时短暂持锁获取 `Arc` 引用后立即释放，避免阻塞注册表

### 前端组件要点
- **懒加载**: PreviewOverlay、SettingsDialog、HistoryPanel 通过 `React.lazy` 延迟加载，减少首屏体积
- **PDF 预览**: `PdfCanvasViewer` 使用 `pdfjs-dist` 在 Canvas 上渲染，支持缩放(0.5x-3x)、翻页、自适应宽度/页面模式
- **Toast 通知**: 全局 `ToastContainer` 渲染在固定右上角，3 秒自动消失，支持 error/success/warning 三种类型，最大 5 条同时显示
- **错误边界**: `ErrorBoundary` 包裹应用根组件，捕获渲染异常，提供"恢复页面"和"重启应用"操作
- **虚拟滚动**: 工作流时间线/文件树等长列表使用虚拟滚动优化大量节点渲染性能

### Python Sidecar 通信协议
- stdin/stdout JSON 行协议，SidecarManager 自动管理进程生命周期（启动、停止、崩溃时自动重启）
- 请求: `{"id": "...", "action": "generate|read|modify|delete|convert|analyze", "type": "docx|xlsx|pptx|pdf|md", "params": {...}}`
- 响应: `{"id": "...", "success": true|false, "data": {...}, "error": "..."}`
- 默认请求超时 120 秒，超时后自动重启 Sidecar 进程并重试一次

### 后台健康检查
- LLM Provider 健康检查: 每 5 分钟执行一次 `health_check_all()`，自动标记不可用 Provider；切换时发射 `llm:provider_switch` 事件
- Sidecar 健康检查: 每 3 分钟执行一次，不健康时记录警告日志

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
- 2000-2999: Agent (已运行/最大迭代/确认超时/Skill不存在/禁用等)
- 3000-3999: 文档处理 (文件不存在/格式不支持/解析错误/Sidecar错误等)
- 4000-4999: 数据库 (连接/查询/记录不存在/约束冲突/迁移失败等)
- 5000-5999: 配置 (格式无效/字段缺失/Provider不存在等)
- 6000-6999: 文件系统 (路径不存在/权限/已存在/IO错误等)
- 7000-7999: 运行时 (事件发射错误)
- 8000-8999: 更新 (检查/下载/安装失败)
- 9000-9999: 工具 (不存在/参数无效/执行失败/路径越界)

### 应用设置
`AppSettings` 含以下子配置（JSON 文件存储），前端 SettingsDialog 含 8 个标签页：
- `GeneralSettings`: 作者名、确认级别(Always/EditOnly/Never)、语言 → **GeneralTab**
- `AppearanceSettings`: 主题模式(light/dark/system) → **AppearanceTab**
- `VersionSnapshot`: 保留策略(ByCount/ByDays/Both)、最大数量/天数
- `WorkspaceDefaults`: 默认工作区 ID → **WorkspaceTab**
- `Shortcuts`: 快捷键配置（newSession/closeSession/sendMessage/toggleSidebar/quickPrompt）→ **ShortcutsTab**
- `disabled_skills`: 已禁用 Skill 列表 → **SkillsTab**
- LLM Provider 配置管理 → **LLMConfig**（含 ProviderFormDialog 子弹窗）
- Prompt 模板管理 → **TemplatesTab**（含 TemplateEditDialog 子弹窗）

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
- `useToastStore`: Toast 通知队列管理（error/success/warning，自动消失）

### 数据存储
- SQLite: 会话、消息、版本快照、Prompt 模板
- JSON 文件: LLM Provider 配置、应用设置、工作区配置、自定义 Skill 配置 (`config/custom_skills/`)
- 文件系统: 工作区文档和 Sidecar 日志 (`log/docagent.log`)
- 应用数据目录: `<app_data_dir>/docagent.db` + `config/` 目录

### 日志系统
- 双输出: 控制台(stderr) + 日志文件(`log/docagent.log`)
- 开发模式(debug): DEBUG 级别；发布模式(release): INFO 级别
- 每次启动覆盖日志文件 (Create + Truncate)
- 日志文件创建失败时降级为仅控制台输出

### Tauri 安全与权限
- 无边框窗口 (`decorations: false`)，自定义窗口控件
- CSP 限制严格: 仅允许 `http://localhost:*` 和 `http://127.0.0.1:*` 的 connect-src（用于 LLM API 调用）
- 使用 `capabilities/` 目录配置插件权限（shell、dialog 等）
- Tauri 插件: `tauri-plugin-shell`, `tauri-plugin-dialog`；桌面端额外注册 `tauri-plugin-updater` + `tauri-plugin-process`
- 30+ 注册命令覆盖 LLM 管理、会话 CRUD、工作区操作、文档处理、Skill 管理、工具管理、设置、模板 CRUD、日志读取、DevTools 切换、更新检查/安装等

### 自动更新
- 通过 `tauri-plugin-updater` 实现自动更新，NSIS 安装器打包
- `#[cfg(desktop)]` 条件编译更新命令和插件，仅桌面平台可用
- 更新产物签名验证，公钥和端点配置在 `tauri.conf.json` 的 `plugins.updater` 中

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
- `skill_development.md` — Skill 接口规范与自定义 Skill 开发指南
- `database_design.md` — 数据库表结构与迁移策略
- `component_design.md` — 前端组件层级与交互设计
- `task_breakdown.md` — 阶段任务分解与进度
- `e2e_test_plan.md` — E2E 测试计划

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
- Tauri 命令名 `snake_case`，前端封装函数名 `camelCase`（见 `src/services/tauri.ts`）
- Rust 事件 payload 使用 `#[serde(rename_all = "camelCase")]`，前端直接接收 camelCase 字段
- Python Sidecar 的 `input_path` 要映射为 handler 期望的 `path` 参数
- Skill 的 `workspace_root` 由 executor 注入，不信任 LLM 提供的值，防止路径遍历攻击
- 文档预览: 普通文件返回文本 `PreviewContent`，PDF 文件通过 `get_pdf_data` 返回 base64 数据由前端 `PdfCanvasViewer` 渲染
- 所有文件操作（创建/删除/重命名）通过 Tauri 命令在 Rust 端执行，前端不直接操作文件系统
- 应用初始化顺序: 应用数据目录 → 日志系统 → 数据库（含损坏检测+自动重建） → 配置管理器 → LLM Config → LLM Router → Sidecar → Skill 注册表 + builtin skills → Tool 注册表 + builtin tools → Custom Skill 加载器 → AppState 注册 → FS 监听器 → 后台健康检查任务（LLM 每5分钟、Sidecar 每3分钟）
- 应用安装了自定义 panic hook，将 panic 信息记录到日志文件并尝试发射 `runtime:error` 事件到前端
