# WorkMolde AI 技术架构文档

> 版本：v2.0 | 日期：2026-06-14

---

## 1. 技术栈总览

| 层级 | 技术 | 版本 | 用途 |
|------|------|------|------|
| 桌面框架 | Tauri | 2.x | Rust后端 + Web前端的桌面应用框架 |
| 前端框架 | React | 19.x | UI组件化开发 |
| 类型系统 | TypeScript | 5.x | 前端类型安全 |
| 构建工具 | Vite | 6.x | 前端构建与热更新 |
| UI组件库 | Shadcn/ui + Radix | latest | 无样式原语组件 + 定制主题 |
| 样式方案 | Tailwind CSS | 4.x | 原子化CSS |
| 状态管理 | Zustand | 5.x | 轻量级全局状态管理 |
| 后端语言 | Rust | 1.80+ (edition 2021) | Tauri后端逻辑 |
| 本地数据库 | SQLite (rusqlite, bundled) | 0.31 | 会话历史、版本快照 |
| 配置存储 | JSON (serde_json) | - | LLM配置、应用设置、工作区配置 |
| Python Sidecar | Python | 3.12+ | 文档处理脚本运行时 |
| 文档处理 | python-docx / openpyxl / python-pptx / PyMuPDF / reportlab / pdfminer.six | - | Word/Excel/PPT/PDF生成与修改 |
| Markdown渲染 | react-markdown + remark-gfm + rehype-highlight | - | Markdown实时渲染预览 |
| PDF预览 | pdfjs-dist | - | 应用内PDF渲染 |
| 图表绘制 | recharts | - | Token用量统计图表 |
| 差异对比 | diff 库 | - | 版本差异对比 |
| 国际化 | react-i18next + i18next | - | 中英文界面 |

---

## 2. 项目目录结构

```
workmolde/
├── src-tauri/                        # Tauri Rust后端
│   ├── Cargo.toml                    # Rust依赖配置 (version = "0.1.6")
│   ├── tauri.conf.json               # Tauri配置 (无边框窗口, CSP)
│   ├── capabilities/                 # Tauri权限配置
│   ├── src/
│   │   ├── main.rs                   # 入口
│   │   ├── lib.rs                    # 库入口，AppState定义，初始化流程，命令注册
│   │   ├── errors.rs                 # 统一错误码体系（9个模块，1000-9999）
│   │   ├── commands/                 # Tauri命令层（前端可调用）
│   │   │   ├── mod.rs
│   │   │   ├── llm.rs                # LLM Provider 增删改查/测试/健康检查
│   │   │   ├── agent.rs              # start/stop/confirm/is_running/context_usage
│   │   │   ├── session.rs            # 会话 CRUD + clear_all
│   │   │   ├── workspace.rs          # 工作区 CRUD + file_tree + search
│   │   │   ├── document.rs           # 文档预览/版本/回滚 + 文件操作
│   │   │   ├── handler.rs            # Handler/Tool 查询
│   │   │   ├── settings.rs           # 设置读写
│   │   │   ├── template.rs           # 模板 CRUD
│   │   │   ├── log.rs                # 日志路径/错误日志读取
│   │   │   └── update.rs             # 更新检查/安装 (desktop-only)
│   │   ├── services/                 # 业务逻辑层
│   │   │   ├── mod.rs
│   │   │   ├── agent/                # Agent调度引擎
│   │   │   │   ├── mod.rs
│   │   │   │   ├── executor.rs       # Agent执行器 (Tool Calling循环)
│   │   │   │   ├── context.rs        # 对话上下文管理
│   │   │   │   └── prompts/          # System Prompt 集合
│   │   │   │       ├── mod.rs
│   │   │   │       ├── document_design.rs    # 文档生成规范
│   │   │   │       ├── prompt_loader.rs      # Prompt加载工具
│   │   │   │       ├── task_type.rs          # 任务类型分类
│   │   │   │       └── token_budget.rs       # Token预算管理
│   │   │   ├── llm/                 # LLM多Provider适配
│   │   │   │   ├── mod.rs
│   │   │   │   ├── provider.rs       # Provider trait定义
│   │   │   │   ├── router.rs         # 路由+Fallback+健康检查+EMA追踪
│   │   │   │   ├── openai_adapter.rs  # OpenAI兼容API适配
│   │   │   │   ├── anthropic_adapter.rs # Anthropic Messages API适配
│   │   │   │   ├── gemini_adapter.rs # Google Gemini API适配
│   │   │   │   └── context_presets.rs # 各模型上下文窗口预设
│   │   │   ├── handler/             # Handler执行引擎
│   │   │   │   ├── mod.rs
│   │   │   │   ├── registry.rs      # Handler注册表
│   │   │   │   └── builtin.rs       # 4个文档类型内置Handler注册
│   │   │   ├── tool/                # Tool执行引擎（始终启用）
│   │   │   │   ├── mod.rs
│   │   │   │   ├── trait_def.rs     # Tool trait定义
│   │   │   │   ├── registry.rs      # Tool注册表
│   │   │   │   └── builtin.rs       # 10个内置Tool注册（文件系统+脚本写入与命令执行）
│   │   │   ├── document/            # Python Sidecar进程管理
│   │   │   │   └── mod.rs           # DocumentService + SidecarManager
│   │   │   ├── attachment.rs        # 文件附件处理 (base64编码, MIME校验)
│   │   │   ├── fs_watcher.rs        # 文件系统监听 (notify crate)
│   │   │   └── network_monitor.rs   # 网络状态监控 (Online/Offline)
│   │   ├── db/                      # 数据库层
│   │   │   ├── mod.rs               # Database结构体定义
│   │   │   ├── init.rs              # 初始化+建表+索引+种子数据
│   │   │   ├── session_repo.rs      # 会话表操作
│   │   │   ├── session_summary_repo.rs # 会话摘要表操作
│   │   │   ├── message_repo.rs      # 消息表操作
│   │   │   ├── snapshot_repo.rs     # 快照表操作
│   │   │   ├── template_repo.rs     # 模板表操作
│   │   │   └── user_preference_repo.rs # 用户偏好表操作
│   │   ├── config/                  # 配置管理
│   │   │   ├── mod.rs               # ConfigManager
│   │   │   ├── app_settings.rs      # AppSettings (General/Appearance/Shortcuts等)
│   │   │   ├── llm_config.rs        # LLM Provider配置
│   │   │   └── workspace_config.rs  # 工作区配置
│   │   ├── models/                  # 数据模型
│   │   │   ├── mod.rs
│   │   │   ├── session.rs
│   │   │   ├── message.rs           # 含 AttachmentMeta/AttachmentType
│   │   │   ├── document.rs
│   │   │   ├── llm.rs               # ContentPart/ProviderConfig/ContextUsageInfo
│   │   │   ├── handler.rs
│   │   │   ├── tool.rs
│   │   │   ├── template.rs
│   │   │   ├── workspace.rs
│   │   │   └── context_memory.rs
│   │   ├── events/                  # 事件系统
│   │   │   ├── mod.rs
│   │   │   ├── types.rs             # 15+事件名常量+Payload结构体
│   │   │   └── emitter.rs           # AgentEmitter封装
│   │   └── utils/                   # 工具函数
│   │       ├── mod.rs
│   │       └── logger.rs            # 双输出日志 (文件+stderr)
│   └── resources/                   # 打包资源
├── src/                             # React前端 (TypeScript)
│   ├── main.tsx                     # 入口 (ErrorBoundary包裹)
│   ├── App.tsx                      # 根组件 (~1048行，事件监听+状态管理+快捷键)
│   ├── components/
│   │   ├── layout/                  # 布局组件
│   │   │   ├── TopBar.tsx           # 顶部栏 (历史/新建/设置/工作区选择)
│   │   │   ├── MainLayout.tsx       # 主布局容器
│   │   │   ├── MainArea.tsx         # 主内容区域
│   │   │   ├── Sidebar.tsx          # 右侧栏容器
│   │   │   ├── InputArea.tsx        # 输入框 (自动高度/快捷键/模板标签)
│   │   │   ├── WindowControls.tsx   # 自定义窗口控件 (最小化/最大化/关闭)
│   │   │   ├── WorkspaceSelector.tsx # 工作区选择器
│   │   │   └── NetworkStatusBanner.tsx # 断网横幅
│   │   ├── workflow/                # 工作流组件
│   │   │   ├── WorkflowTimeline.tsx  # 工作流时间线 (虚拟滚动)
│   │   │   ├── WorkflowNode.tsx      # 多态节点渲染器
│   │   │   ├── UserNode.tsx          # 用户消息节点
│   │   │   ├── ThinkingNode.tsx      # 思考过程节点
│   │   │   ├── ContentNode.tsx       # 回复内容节点
│   │   │   ├── ToolNode.tsx          # 工具调用节点
│   │   │   ├── ConfirmNode.tsx       # 确认请求节点
│   │   │   └── ErrorNode.tsx         # 错误节点 (含重试按钮)
│   │   ├── sidebar/                 # 右侧栏组件
│   │   │   ├── FileTreeSection.tsx   # 文件树区域
│   │   │   ├── AgentInfoSection.tsx  # Agent信息 (LLM名称/作者名)
│   │   ├── preview/                 # 预览组件
│   │   │   ├── PreviewOverlay.tsx    # 预览浮层 (懒加载)
│   │   │   ├── MarkdownPreview.tsx   # Markdown渲染 (react-markdown+rehype)
│   │   │   ├── PdfCanvasViewer.tsx   # PDF Canvas渲染 (pdfjs-dist)
│   │   │   └── VersionHistoryPanel.tsx # 版本历史面板 (懒加载)
│   │   ├── settings/                # 设置弹窗组件
│   │   │   ├── SettingsDialog.tsx    # 设置弹窗 (懒加载, 9个标签页)
│   │   │   ├── LLMConfig.tsx        # LLM Provider增删改查+测试
│   │   │   ├── ProviderFormDialog.tsx # Provider表单子弹窗
│   │   │   ├── WorkspaceTab.tsx     # 工作区管理
│   │   │   ├── AddWorkspaceDialog.tsx # 添加工作区子弹窗
│   │   │   ├── HandlersTab.tsx      # Handler/Tool列表
│   │   │   ├── TemplatesTab.tsx     # Prompt模板管理
│   │   │   ├── TemplateEditDialog.tsx # 模板编辑子弹窗
│   │   │   ├── GeneralTab.tsx       # 通用设置 (作者名/确认级别/快照策略)
│   │   │   ├── AppearanceTab.tsx    # 外观 (主题/语言/字体缩放)
│   │   │   ├── ShortcutsTab.tsx     # 快捷键自定义
│   │   │   └── HelpTab.tsx          # 帮助信息
│   │   ├── session/                 # 会话组件
│   │   └── common/                  # 通用组件
│   │       ├── Button.tsx
│   │       ├── Icon.tsx             # SVG图标组件
│   │       ├── ContextMenu.tsx      # 右键上下文菜单
│   │       ├── DeleteConfirmDialog.tsx # 删除确认弹窗
│   │       ├── ErrorBoundary.tsx    # 错误边界 (包裹App根组件)
│   │       ├── Toast.tsx            # Toast通知 (3秒自动消失)
│   │       └── UpdateNotification.tsx # 更新通知 (懒加载)
│   ├── stores/                      # Zustand 状态管理
│   │   ├── useWorkflowStore.ts      # 工作流节点/执行状态/迭代分组
│   │   ├── useSessionStore.ts       # 会话 CRUD
│   │   ├── useWorkspaceStore.ts     # 工作区列表/切换
│   │   ├── useSettingsStore.ts      # 应用设置
│   │   ├── useFileTreeStore.ts      # 文件树数据
│   │   ├── useToastStore.ts         # Toast 通知队列
│   │   ├── useAttachmentStore.ts    # 待发送附件管理
│   │   ├── useNetworkStore.ts       # 网络状态
│   │   └── useUpdateStore.ts        # 更新状态
│   ├── services/                    # 前端服务层
│   │   ├── tauri.ts                 # invoke封装 (40+命令的camelCase包装)
│   │   ├── event.ts                 # 事件监听+Payload类型定义
│   │   └── errorHandler.ts          # 错误解析+用户友好提示
│   ├── hooks/
│   │   └── useAgent.ts              # Agent交互核心Hook
│   ├── types/                       # TypeScript类型 (与Rust models同步)
│   │   ├── index.ts
│   │   ├── workflow.ts / session.ts / settings.ts / workspace.ts / document.ts
│   ├── i18n/                        # 国际化
│   │   ├── index.ts                 # i18n初始化
│   │   └── locales/zh-CN.json, en-US.json
│   ├── utils/
│   │   ├── format.ts                # 格式化工具
│   │   ├── fileIcons.tsx            # 文件图标映射
│   │   └── logger.ts                # 前端日志
│   └── styles/
│       └── globals.css              # 全局样式 (Tailwind CSS)
├── sidecar/                         # Python 文档处理引擎
│   ├── main.py                      # stdin/stdout JSON行协议入口
│   ├── requirements.txt             # Python依赖 (13个库)
│   ├── handlers/
│   │   ├── word_handler.py          # Word文档处理
│   │   ├── excel_handler.py         # Excel文档处理
│   │   ├── ppt_handler.py           # PPT文档处理
│   │   ├── pdf_handler.py           # PDF文档处理
│   │   ├── markdown_handler.py      # Markdown/文本处理
│   │   ├── validator.py             # 文档校验器
│   │   └── font_utils.py            # 字体工具
│   └── tests/
├── docs/                            # 开发文档
├── shared/                          # 前后端共享类型
│   └── types.ts
├── package.json
├── tsconfig.json
├── vite.config.ts
└── README.md / README_zh.md
```

---

## 3. 模块架构

### 3.1 整体架构图

```
┌───────────────────────────────────────────────────────────────┐
│                     React Frontend                             │
│  ┌──────────┐ ┌───────────┐ ┌──────────┐ ┌───────────────┐   │
│  │ Workflow  │ │  Sidebar  │ │ Preview  │ │  Settings     │   │
│  │ Timeline  │ │ 4 Sections│ │ Overlay  │ │  Dialog (9탭) │   │
│  └────┬──────┘ └────┬──────┘ └────┬─────┘ └───────┬───────┘   │
│       │              │             │               │           │
│  ┌────┴──────────────┴─────────────┴───────────────┴───────┐   │
│  │              Zustand Stores (9) + useAgent Hook          │   │
│  └──────────────────────────┬──────────────────────────────┘   │
│                             │ invoke / event                    │
├─────────────────────────────┼──────────────────────────────────┤
│                        Tauri IPC Layer                          │
├─────────────────────────────┼──────────────────────────────────┤
│                             ▼                                   │
│  ┌────────────────────────────────────────────────────────┐    │
│  │            Tauri Commands (40+ 命令, 10模块)             │    │
│  │  llm | agent | session | workspace | document | handler  │    │
│  │          settings | template | log | update              │    │
│  └──────────────────────┬─────────────────────────────────┘    │
│                         │                                       │
│  ┌──────────────────────┴─────────────────────────────────┐    │
│  │                Services (Rust)                          │    │
│  │  ┌──────────┐ ┌──────────┐ ┌────────┐ ┌──────────┐    │    │
│  │  │  Agent   │ │   LLM    │ │Handler │ │   Tool   │    │    │
│  │  │  Engine  │ │  Router  │ │Registry│ │ Registry │    │    │
│  │  └────┬─────┘ └────┬─────┘ └───┬────┘ └────┬─────┘    │    │
│  │       │             │           │           │           │    │
│  │  ┌────┴─────────────┴───────────┴───────────┴──────┐   │    │
│  │  │              Document Service                     │   │    │
│  │  │         (Python Sidecar Process Manager)          │   │    │
│  │  └──────────────────┬──────────────────────────────┘   │    │
│  │                     │                                    │    │
│  │  ┌──────────────────┴────────┐ ┌────────────────────┐   │    │
│  │  │   FsWatcher / Network     │ │   Config Manager   │   │    │
│  │  │   Monitor / Attachment    │ │   + DB Layer       │   │    │
│  │  └───────────────────────────┘ └────────────────────┘   │    │
│  └────────────────────────────────────────────────────────┘    │
│                                                                │
│  ┌────────────────────────────────────────────────────────┐    │
│  │                Data Layer                                │    │
│  │  ┌──────────┐  ┌──────────────┐  ┌─────────────────┐   │    │
│  │  │  SQLite   │  │  JSON Config  │  │  File System    │   │    │
│  │  │ (6 tables)│  │  (4 files)    │  │  (Workspace)    │   │    │
│  │  └──────────┘  └──────────────┘  └─────────────────┘   │    │
│  └────────────────────────────────────────────────────────┘    │
└────────────────────────────────────────────────────────────────┘
                            │
                            ▼
                ┌──────────────────────────┐
                │   Python Sidecar          │
                │  (文档处理子进程)          │
                │  word/excel/ppt/pdf/md    │
                │  + write_script/run_command Tool │
                └──────────────────────────┘
```

### 3.2 AppState 全局状态

```rust
pub struct AppState {
    db: Arc<Database>,                                    // SQLite 数据库
    config: Arc<Mutex<ConfigManager>>,                    // JSON 配置管理器
    active_agents: Arc<Mutex<HashMap<String, bool>>>,     // 活动 Agent 会话
    confirm_channels: Arc<Mutex<HashMap<String, oneshot::Sender<ConfirmDecision>>>>, // 确认通道
    doc_service: Arc<DocumentService>,                    // Python Sidecar 服务
    llm_router: Arc<RwLock<Arc<LlmRouter>>>,              // LLM 路由器（含 Fallback）
    tool_registry: Arc<ToolRegistry>,                     // Tool 注册表（始终启用）
    handler_registry: Arc<Mutex<HandlerRegistry>>,        // Handler 注册表（始终启用）
    fs_watcher: Arc<FsWatcherService>,                    // 文件系统监听
    network_monitor: Arc<NetworkMonitor>,                  // 网络状态监控
}
```

### 3.3 模块职责

| 模块 | 职责 | 关键接口 |
|------|------|---------|
| **LLM Router** | 统一多Provider管理，流式响应，顺序Fallback，健康检查(5分钟间隔)，延迟EMA追踪 | `Provider` trait, `LlmRouter` |
| **Agent Engine** | Agent调度核心，Tool Calling循环(最大20轮)，增量持久化，用户停止支持 | `AgentExecutor::execute()` |
| **Handler Engine** | 文档处理Handler注册和执行，通过Python Sidecar实现 | `HandlerRegistry` (4个内置文档Handler) |
| **Tool Engine** | 文件系统基础操作+脚本写入与命令执行，始终启用，纯Rust实现 | `ToolRegistry` (10个内置Tool) |
| **Document Service** | Python Sidecar进程生命周期管理（启动/停止/自动重启/超时/重试） | `SidecarManager` |
| **Config Manager** | JSON配置文件读写、验证、合并默认值、热更新 | `ConfigManager` |
| **DB Layer** | SQLite 6表+索引，含损坏检测和自动恢复 | `Database`, 各 Repository |

---

## 4. 核心数据流

### 4.1 Agent 执行流程

```
用户输入
  │
  ▼
[前端] InputArea → handleSend → invoke("start_agent")
  │
  ▼
[Rust] start_agent 命令
  │
  ▼
AgentExecutor
  │
  ├─→ 构建上下文（System Prompt + 历史消息 + Handler/Tool 定义）
  │     System Prompts: document_design, task_type, token_budget
  │
  ▼ (循环)
LLM Adapter → HTTP请求 → LLM API (流式)
  │
  ├─→ agent:deep_thinking (Extended Thinking / reasoning_content)
  ├─→ agent:thinking (普通思考链)
  ├─→ agent:content (回复内容增量)
  ├─→ agent:tool_call → Tool/Handler 执行
  │   ├─→ Handler (文档处理) → Python Sidecar → 结果
  │   └─→ Tool (文件系统) → 纯 Rust → 结果
  ├─→ agent:tool_result (执行结果)
  ├─→ agent:context_update (Token用量更新)
  │
  ▼ (本轮结束)
  ├─→ 增量持久化消息到 SQLite
  ├─→ 是否需确认 → agent:confirm → 用户回复 → confirm_operation
  ├─→ 是否停止 → agent:stopped
  │
  ▼ (循环结束)
  ├─→ agent:done (执行完成)
  └─→ 完整持久化所有消息
```

### 4.2 事件协议

| 事件名 | Payload | 说明 |
|--------|---------|------|
| `agent:thinking` | `ThinkingPayload` | 普通思考链（增量） |
| `agent:deep_thinking` | `DeepThinkingPayload` | 深度思考链/Extended Thinking（增量） |
| `agent:content` | `ContentPayload` | 回复内容（增量，含iteration分组） |
| `agent:tool_call` | `ToolCallPayload` | Tool调用开始（含iteration分组） |
| `agent:tool_result` | `ToolResultPayload` | Tool执行结果 |
| `agent:confirm` | `ConfirmPayload` | 需要用户确认（含risk_level） |

| `agent:context_update` | `ContextUsagePayload` | Token用量更新 |
| `agent:network_retry` | `NetworkRetryPayload` | LLM网络重试通知 |
| `agent:done` | `DonePayload` | 执行完成 |
| `agent:error` | `ErrorPayload` | 执行出错（含recoverable标记） |
| `agent:stopped` | `StoppedPayload` | 用户中断 |
| `session:updated` | `SessionUpdatePayload` | 会话变更 |
| `workspace:change` | `WorkspaceChangePayload` | 工作区切换 |
| `workspace:directory_deleted` | `WorkspaceDirectoryDeletedPayload` | 工作区目录被外部删除 |
| `file:change` | `FileChangePayload` | 文件变更 |
| `llm:provider_switch` | `ProviderSwitchPayload` | Provider自动切换 |
| `system:network_change` | `NetworkChangePayload` | 网络状态变化 |

### 4.3 文档处理流程

```
Handler/Tool 调用 → 根据类型分发
  │
  ├─→ 文档 Handler (docx/xlsx/pptx/pdf/md)
  │    ├─ read:    读取文档内容
  │    ├─ convert: 格式转换
  │    └─ analyze: 文档分析
  │
  └─→ 文件系统 Tool (纯 Rust，10个)
       ├─ list_directory: 列出目录
       ├─ search_files:   搜索文件
       ├─ read_file:      读取文本文件
       ├─ write_text_file: 写入文本文件
       ├─ delete_file:    删除文件
       ├─ file_info:      获取文件元数据
       ├─ file_exists:    检查文件存在
       ├─ create_directory: 创建目录
       ├─ write_script:   将智能体生成的脚本写入系统临时目录
       └─ run_command:    通过 Git Bash 执行命令（运行脚本）
```

### 4.4 确认机制流程

```
高风险操作（run_command 执行危险命令）
  │
  ├─→ 检查确认级别 (Always/EditOnly/Never)
  │    ├─ Never → 直接执行
  │    ├─ Always → 必须确认
  │    └─ EditOnly (默认) → 生成/修改操作需确认
  │
  ├─→ agent:confirm → 前端显示确认弹窗
  │    ├─ 代码功能描述 + 代码摘要(前200字符)
  │    └─ 风险等级 (high/normal)
  │
  ├─→ 用户确认/拒绝 → confirm_operation
  │    ├─ 同意 → 执行并返回结果
  │    └─ 拒绝 → 返回拒绝消息给LLM
  │
  └─→ 超时 (5分钟) → 自动拒绝
```

---

## 5. 前后端通信

| 方式 | 场景 | 特点 |
|------|------|------|
| `invoke()` | 请求-响应式调用 | 同步等待结果，命令名 snake_case |
| `emit()/listen()` | 事件推送 | 异步单向，命名 `namespace:action` |
| `oneshot::channel` | 确认操作同步等待 | Agent等待用户确认时使用，5分钟超时 |

**命名规范：**
- Tauri 命令：`snake_case`，如 `start_agent`
- 前端封装：`camelCase`，如 `startAgent()`
- 事件名：`namespace:action`，如 `agent:thinking`
- Rust Payload：`#[serde(rename_all = "camelCase")]`

---

## 6. 安全设计

### 6.1 API Key 存储

- API Key 以明文 JSON 格式存储在 `llm_config.json` 中（当前版本未加密）
- 仅 LLM API 调用需要联网，使用 HTTPS (rustls-tls)
- 可配置环境变量 `WORKMOLDE_PYTHON` 指定 Python 解释器路径

### 6.2 文件系统安全

- 所有文件操作通过 executor 注入 `workspace_root`，拒绝路径遍历攻击
- Tool 路径安全校验：拒绝访问工作区外的路径
- 删除操作可选备份，强制工作区路径校验
- run_command 命令执行安全：高风险命令（rm -rf、format 等）需用户确认
- write_script 写入路径限定为系统临时目录 `<temp_dir>/workmolde/scripts/`

### 6.3 网络安全

- CSP限制严格：仅允许 `http://localhost:*` 和 `http://127.0.0.1:*` 的 connect-src
- Tauri 插件权限通过 `capabilities/` 目录配置
- 网络状态监控：NetworkMonitor 定时检测，断网时自动暂停 LLM 请求

---

## 7. 错误码体系

| 范围 | 模块 | 说明 |
|------|------|------|
| 1000-1999 | LLM | 连接失败/认证/限流/超时/Provider不可用等 (14个码) |
| 2000-2999 | Agent | 已运行/最大迭代/确认超时/Handler不存在等 (8个码) |
| 3000-3999 | 文档处理 | 文件不存在/格式不支持/解析错误/Sidecar错误等 (12个码) |
| 4000-4999 | 数据库 | 连接/查询/记录不存在/约束冲突/迁移失败等 (7个码) |
| 5000-5999 | 配置 | 格式无效/字段缺失/Provider不存在等 (8个码) |
| 6000-6999 | 文件系统 | 路径不存在/权限/已存在/IO错误等 (8个码) |
| 7000-7999 | 运行时 | 事件发射错误 (1个码) |
| 8000-8999 | 更新 | 检查/下载/安装失败 (5个码) |
| 9000-9999 | Tool | 不存在/参数无效/执行失败/路径越界 (4个码) |

---

## 8. Python Sidecar 通信协议

### 8.1 通信架构

```
Rust后端 ──JSON──> stdin  ┌──────────────┐  stdout ──JSON──> Rust后端
                         │  Python       │
                         │  Sidecar      │
                         │  (文档处理)    │
                         └──────────────┘
```

- 通信：JSON over stdin/stdout，UTF-8编码
- 消息分隔：每条 JSON 以换行符 `\n` 结尾
- 请求：`{"id": "...", "action": "read|convert|analyze|ping|validate", "type": "docx|xlsx|pptx|pdf|md|txt", "params": {...}}`
- 响应：`{"id": "...", "success": true|false, "data": {...}, "error": "..."}`
- 默认超时：60秒（文档操作）

### 8.2 Handler 与 Tool 分工

- **Handler**（4个，通过 Python Sidecar）：文档类型专业处理（word/excel/ppt/pdf），仅支持 read/convert/analyze
- **Tool**（10个，纯 Rust）：文件系统基础操作 + 脚本写入与命令执行（write_script + run_command），始终启用

---

## 9. 初始化顺序

1. 应用数据目录创建
2. 日志系统初始化（双输出：控制台+日志文件）
3. 数据库初始化（含损坏检测+自动重建）：建6表+索引+种子模板数据
4. 配置管理器初始化（加载 llm_config → app_settings → workspaces）
5. LLM Config 加载 → LlmRouter 创建
6. Python Sidecar 进程启动（路径检测：环境变量 `WORKMOLDE_PYTHON` / py / python / python3）
7. DocumentService + HandlerRegistry（4个内置文档处理器）
8. ToolRegistry（10个内置工具：文件系统操作 + write_script + run_command）
9. AppState 注册
10. FsWatcher 初始化（自动监听活动工作区）
11. NetworkMonitor 初始化
12. 后台任务启动：Provider健康检查(5min) + Sidecar健康检查(3min) + 工作区存在性检查(10s) + 网络监控

---

## 10. 性能优化策略

| 层面 | 策略 |
|------|------|
| 前端 | 虚拟滚动（工作流时间线+文件树），React.lazy懒加载（预览/设置/历史面板/更新通知），ErrorBoundary错误边界 |
| 后端 | LLM流式响应减少首字延迟，SQLite WAL模式读写并发，Sidecar进程常驻避免冷启动 |
| Sidecar | 代码执行子进程超时控制(60s)，stdout截断(10000字节)，内存限制(512MB) |
