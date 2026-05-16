# DocAgent 技术架构文档

> 版本：v1.0 | 日期：2026-05-14

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
| 后端语言 | Rust | 1.80+ | Tauri后端逻辑 |
| 本地数据库 | SQLite (rusqlite) | - | 会话历史、版本快照、Token统计 |
| 配置存储 | JSON (serde_json) | - | LLM配置、应用设置、工作区配置 |
| Python Sidecar | Python | 3.12+ | 文档处理脚本运行时 |
| 文档处理 | python-docx / openpyxl / python-pptx / reportlab | - | Word/Excel/PPT/PDF生成与修改 |
| Markdown渲染 | react-markdown + remark/rehype | - | Markdown实时渲染预览 |
| PDF预览 | pdfjs-dist | - | 应用内PDF渲染 |
| 代码高亮 | Shiki | - | 代码块语法高亮 |
| 数学公式 | KaTeX | - | LaTeX数学公式渲染 |

---

## 2. 项目目录结构

```
docagent/
├── src-tauri/                    # Tauri Rust后端
│   ├── Cargo.toml
│   ├── tauri.conf.json           # Tauri配置
│   ├── capabilities/             # Tauri权限配置
│   ├── src/
│   │   ├── main.rs               # 入口
│   │   ├── lib.rs                # 库入口，注册命令
│   │   ├── commands/             # Tauri命令（前端可调用）
│   │   │   ├── mod.rs
│   │   │   ├── llm.rs            # LLM相关命令
│   │   │   ├── workspace.rs      # 工作区相关命令
│   │   │   ├── document.rs       # 文档操作命令
│   │   │   ├── session.rs        # 会话管理命令
│   │   │   ├── skill.rs          # Skill管理命令
│   │   │   ├── settings.rs       # 设置相关命令
│   │   │   └── version.rs        # 版本快照命令
│   │   ├── services/             # 业务逻辑层
│   │   │   ├── mod.rs
│   │   │   ├── llm/              # LLM适配层
│   │   │   │   ├── mod.rs
│   │   │   │   ├── provider.rs   # Provider trait定义
│   │   │   │   ├── openai.rs     # OpenAI适配器
│   │   │   │   ├── claude.rs     # Claude适配器
│   │   │   │   ├── gemini.rs     # Gemini适配器
│   │   │   │   └── router.rs     # Provider路由与Fallback
│   │   │   ├── agent/            # Agent调度引擎
│   │   │   │   ├── mod.rs
│   │   │   │   ├── executor.rs   # Agent执行器
│   │   │   │   ├── tool_call.rs  # Tool Calling处理
│   │   │   │   └── context.rs    # 上下文管理
│   │   │   ├── skill/            # Skill执行引擎
│   │   │   │   ├── mod.rs
│   │   │   │   ├── registry.rs   # Skill注册表
│   │   │   │   ├── runner.rs     # Skill执行器
│   │   │   │   └── builtin/      # 内置Skill实现
│   │   │   │       ├── mod.rs
│   │   │   │       ├── generate.rs
│   │   │   │       ├── modify.rs
│   │   │   │       ├── delete.rs
│   │   │   │       ├── convert.rs
│   │   │   │       ├── read.rs
│   │   │   │       ├── search.rs
│   │   │   │       ├── analyze.rs
│   │   │   │       ├── list.rs
│   │   │   │       └── batch.rs
│   │   │   ├── document/         # 文档处理服务
│   │   │   │   ├── mod.rs
│   │   │   │   ├── sidecar.rs    # Python Sidecar管理
│   │   │   │   └── pipeline.rs   # 文档处理管道
│   │   │   └── version/          # 版本快照服务
│   │   │       ├── mod.rs
│   │   │       └── snapshot.rs
│   │   ├── db/                   # 数据库层
│   │   │   ├── mod.rs
│   │   │   ├── init.rs           # 数据库初始化与迁移
│   │   │   ├── session.rs        # 会话表操作
│   │   │   ├── message.rs        # 消息表操作
│   │   │   ├── snapshot.rs       # 快照表操作
│   │   │   └── token_usage.rs    # Token统计表操作
│   │   ├── config/               # 配置管理
│   │   │   ├── mod.rs
│   │   │   ├── llm_config.rs     # LLM配置读写
│   │   │   ├── app_settings.rs   # 应用设置读写
│   │   │   └── workspace.rs      # 工作区配置读写
│   │   └── utils/                # 工具函数
│   │       ├── mod.rs
│   │       ├── crypto.rs         # API Key加密
│   │       └── fs.rs             # 文件系统工具
│   └── resources/                # 打包资源
│       └── sidecar/              # Python Sidecar
│           ├── main.py           # Sidecar入口
│           ├── requirements.txt
│           └── handlers/         # 文档处理handler
│               ├── word.py
│               ├── excel.py
│               ├── ppt.py
│               ├── pdf.py
│               ├── markdown.py
│               └── convert.py
├── src/                          # React前端
│   ├── main.tsx                  # 入口
│   ├── App.tsx                   # 根组件
│   ├── components/               # UI组件
│   │   ├── layout/               # 布局组件
│   │   │   ├── TopBar.tsx
│   │   │   ├── MainArea.tsx
│   │   │   ├── Sidebar.tsx
│   │   │   └── InputArea.tsx
│   │   ├── workflow/             # 工作流组件
│   │   │   ├── WorkflowTimeline.tsx
│   │   │   ├── WorkflowNode.tsx
│   │   │   ├── UserNode.tsx
│   │   │   ├── ThinkingNode.tsx
│   │   │   ├── ToolNode.tsx
│   │   │   ├── ResultNode.tsx
│   │   │   └── ReplyNode.tsx
│   │   ├── sidebar/              # 右侧栏组件
│   │   │   ├── FileTree.tsx
│   │   │   ├── AgentInfo.tsx
│   │   │   ├── TodoList.tsx
│   │   │   └── TokenStats.tsx
│   │   ├── preview/              # 预览组件
│   │   │   ├── PreviewPanel.tsx
│   │   │   ├── MarkdownPreview.tsx
│   │   │   ├── DiffView.tsx
│   │   │   └── VersionHistory.tsx
│   │   ├── settings/             # 设置组件
│   │   │   ├── SettingsDialog.tsx
│   │   │   ├── LLMConfig.tsx
│   │   │   ├── WorkspaceManager.tsx
│   │   │   ├── SkillManager.tsx
│   │   │   ├── TemplateManager.tsx
│   │   │   └── GeneralSettings.tsx
│   │   ├── session/              # 会话组件
│   │   │   ├── HistoryPanel.tsx
│   │   │   └── SessionList.tsx
│   │   └── common/               # 通用组件
│   │       ├── ConfirmDialog.tsx
│   │       ├── Icon.tsx
│   │       └── Button.tsx
│   ├── stores/                   # Zustand状态管理
│   │   ├── useWorkflowStore.ts   # 工作流状态
│   │   ├── useSessionStore.ts    # 会话状态
│   │   ├── useWorkspaceStore.ts  # 工作区状态
│   │   ├── useSettingsStore.ts   # 设置状态
│   │   ├── useFileTreeStore.ts   # 文件树状态
│   │   └── useTokenStore.ts      # Token统计状态
│   ├── services/                 # 前端服务层
│   │   ├── tauri.ts              # Tauri命令调用封装
│   │   ├── llm.ts                # LLM流式调用封装
│   │   └── event.ts              # Tauri事件监听封装
│   ├── hooks/                    # 自定义Hooks
│   │   ├── useAgent.ts           # Agent交互Hook
│   │   ├── useFileTree.ts        # 文件树Hook
│   │   └── useTokenCounter.ts    # Token计数Hook
│   ├── types/                    # TypeScript类型定义
│   │   ├── llm.ts
│   │   ├── workflow.ts
│   │   ├── session.ts
│   │   ├── workspace.ts
│   │   ├── skill.ts
│   │   ├── document.ts
│   │   └── settings.ts
│   ├── utils/                    # 工具函数
│   │   ├── format.ts             # 格式化
│   │   └── fileIcons.ts          # 文件图标映射
│   └── styles/                   # 全局样式
│       └── globals.css
├── docs/                         # 开发文档
├── package.json
├── tsconfig.json
├── vite.config.ts
├── tailwind.config.ts
└── .env.example
```

---

## 3. 模块架构

### 3.1 整体架构图

```
┌─────────────────────────────────────────────────────────┐
│                    React Frontend                        │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌───────────┐  │
│  │ Workflow  │ │ Sidebar  │ │ Preview  │ │ Settings   │  │
│  │ Components│ │Components│ │Components│ │ Components │  │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘ └─────┬─────┘  │
│       │             │            │              │        │
│  ┌────┴─────────────┴────────────┴──────────────┴─────┐  │
│  │                  Zustand Stores                     │  │
│  └────────────────────┬───────────────────────────────┘  │
│                       │ invoke / event                    │
├───────────────────────┼──────────────────────────────────┤
│                  Tauri IPC Layer                          │
├───────────────────────┼──────────────────────────────────┤
│                       ▼                                   │
│  ┌────────────────────────────────────────────────────┐  │
│  │              Tauri Commands (Rust)                  │  │
│  │  llm | workspace | document | session | skill | .. │  │
│  └────────────────────┬───────────────────────────────┘  │
│                       │                                   │
│  ┌────────────────────┴───────────────────────────────┐  │
│  │              Services (Rust)                        │  │
│  │  ┌─────────┐ ┌─────────┐ ┌──────────┐             │  │
│  │  │  Agent   │ │   LLM   │ │  Skill   │             │  │
│  │  │ Engine   │ │ Adapter │ │ Engine   │             │  │
│  │  └────┬────┘ └────┬────┘ └─────┬────┘             │  │
│  │       │           │            │                    │  │
│  │  ┌────┴───────────┴────────────┴────┐              │  │
│  │  │        Document Service          │              │  │
│  │  │   (Python Sidecar Management)    │              │  │
│  │  └──────────────┬───────────────────┘              │  │
│  │                 │                                   │  │
│  │  ┌──────────────┴──────┐  ┌──────────────────┐     │  │
│  │  │   Version Service   │  │    Config Mgr     │     │  │
│  │  └─────────────────────┘  └──────────────────┘     │  │
│  └────────────────────────────────────────────────────┘  │
│                                                           │
│  ┌────────────────────────────────────────────────────┐  │
│  │              Data Layer                             │  │
│  │  ┌──────────┐  ┌──────────────┐  ┌─────────────┐  │  │
│  │  │  SQLite  │  │  JSON Config  │  │  File System │  │  │
│  │  └──────────┘  └──────────────┘  └─────────────┘  │  │
│  └────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────┘
                          │
                          ▼
              ┌───────────────────────┐
              │   Python Sidecar      │
              │  (文档处理进程)        │
              │  word/excel/ppt/pdf   │
              └───────────────────────┘
```

### 3.2 模块职责

| 模块 | 职责 | 关键接口 |
|------|------|---------|
| **LLM Adapter** | 统一多LLM Provider的API调用，处理流式响应和Tool Calling | `LLMProvider` trait |
| **Agent Engine** | Agent调度核心，管理Tool Calling循环和上下文 | `execute_agent()` |
| **Skill Engine** | Skill注册、发现、执行，管理内置和自定义Skill | `SkillRegistry`, `SkillRunner` |
| **Document Service** | 管理Python Sidecar生命周期，调度文档处理任务 | `process_document()` |
| **Version Service** | 文档版本快照的创建、查询、回滚、清理 | `create_snapshot()`, `rollback()` |
| **Config Manager** | JSON配置文件的读写、验证、热更新 | `load_config()`, `save_config()` |
| **DB Layer** | SQLite数据库操作，会话/消息/快照/Token统计的CRUD | 各表对应的Repository |

---

## 4. 核心数据流

### 4.1 Agent执行流程

```
用户输入
  │
  ▼
[前端] InputArea组件 → useAgent Hook → invoke("start_agent")
  │
  ▼
[Rust] start_agent命令
  │
  ▼
Agent Engine
  │
  ├─→ 构建上下文（历史消息 + System Prompt + Skill描述）
  │
  ▼
LLM Adapter → HTTP请求 → LLM API
  │
  ▼ (流式响应)
  │
  ├─→ thinking内容 → emit("agent:thinking", chunk)
  ├─→ content内容 → emit("agent:content", chunk)
  ├─→ tool_call → Skill Engine
  │                    │
  │                    ├─→ 内置Skill → 直接执行
  │                    ├─→ 自定义Skill → Sidecar执行
  │                    │
  │                    ▼
  │               Skill执行结果
  │                    │
  │                    ├─→ 版本快照（如需修改文档）
  │                    ├─→ emit("agent:tool_result", result)
  │                    │
  │                    ▼
  │               返回结果给LLM → 继续推理循环
  │
  ▼ (循环结束)
  │
  ├─→ emit("agent:done", summary)
  ├─→ 保存消息到SQLite
  ├─→ 更新Token统计
  │
  ▼
[前端] 事件监听 → 更新Workflow Store → UI刷新
```

### 4.2 流式事件协议

Agent执行过程中，Rust后端通过Tauri Event向前端推送事件：

| 事件名 | Payload | 说明 |
|--------|---------|------|
| `agent:thinking` | `{ session_id, content, chunk }` | LLM思考链内容（增量） |
| `agent:content` | `{ session_id, content, chunk }` | LLM回复内容（增量） |
| `agent:tool_call` | `{ session_id, tool_name, args }` | Tool调用开始 |
| `agent:tool_result` | `{ session_id, tool_name, success, data }` | Tool执行结果 |
| `agent:confirm` | `{ session_id, operation, file_path, description }` | 需要用户确认 |
| `agent:todo_update` | `{ session_id, todos: [...] }` | Todo列表更新 |
| `agent:done` | `{ session_id, summary, usage }` | Agent执行完成 |
| `agent:error` | `{ session_id, error }` | 执行出错 |
| `agent:stopped` | `{ session_id }` | 用户中断 |

### 4.3 文档处理流程

```
Skill调用 → Document Service
  │
  ├─→ 判断操作类型
  │    ├─ 生成文档 → 构建参数 → Sidecar执行
  │    ├─ 修改文档 → 创建版本快照 → Sidecar执行
  │    ├─ 格式转换 → Sidecar执行
  │    └─ 读取文档 → Sidecar执行 / 直接读取
  │
  ▼
Sidecar管理器
  │
  ├─→ 启动Python进程（如未运行）
  ├─→ 通过stdin/stdin JSON协议通信
  │    请求: { "action": "generate", "type": "docx", "params": {...} }
  │    响应: { "success": true, "file_path": "...", "size": 12345 }
  │
  ├─→ 超时控制（默认60秒）
  ├─→ 错误处理与重试
  │
  ▼
文件系统操作
  │
  ├─→ 写入用户工作区目录
  ├─→ 临时文件写入应用数据目录
  └─→ 操作完成后清理临时文件
```

---

## 5. 前后端通信

### 5.1 通信方式

| 方式 | 场景 | 特点 |
|------|------|------|
| `invoke()` | 请求-响应式调用（如读取配置、查询数据） | 同步等待结果 |
| `emit()/listen()` | 事件推送（如流式输出、进度更新） | 异步单向 |
| `Channel` | 双向流式通信（如Agent执行过程） | Tauri 2新特性 |

### 5.2 命名规范

- Tauri命令：`snake_case`，如 `start_agent`, `list_workspaces`
- 前端调用封装：`camelCase`，如 `startAgent()`, `listWorkspaces()`
- 事件名：`namespace:action`，如 `agent:thinking`, `session:updated`
- Store action：`camelCase`，如 `addWorkflowNode()`, `updateTokenUsage()`

---

## 6. 安全设计

### 6.1 API Key加密

- 使用系统密钥链（Windows Credential Manager / macOS Keychain）存储API Key
- 备选方案：AES-256-GCM加密，密钥派生自机器特征码
- 配置文件中仅存储加密后的密文

### 6.2 文件系统安全

- Tauri Scope限制：仅允许访问用户指定的工作区目录和应用数据目录
- Python Sidecar：通过Tauri的scope控制文件访问范围
- 自定义Skill：在UI中明确提示安全风险

### 6.3 网络安全

- 仅LLM API调用需要联网，使用HTTPS
- 不收集、不上传任何用户数据
- 可配置HTTP代理

---

## 7. 错误处理策略

### 7.1 LLM调用错误

| 错误类型 | 处理策略 |
|---------|---------|
| 网络超时 | 自动重试3次（指数退避：1s/2s/4s），然后Fallback |
| API Key无效 | 立即通知用户，不重试 |
| 速率限制 | 等待后重试，或Fallback |
| 模型不可用 | Fallback到备用模型 |
| 响应格式错误 | 尝试解析，失败则重试1次 |

### 7.2 文档操作错误

| 错误类型 | 处理策略 |
|---------|---------|
| 文件不存在 | 返回错误信息给Agent，由LLM告知用户 |
| 权限不足 | 提示用户检查文件权限 |
| 格式不支持 | 返回错误信息，建议可用格式 |
| Sidecar崩溃 | 自动重启Sidecar，重试1次 |
| 操作超时 | 取消操作，通知用户 |

### 7.3 数据库错误

- 使用事务保证原子性
- 写操作失败时回滚
- 定期自动备份SQLite数据库

---

## 8. 性能优化策略

### 8.1 前端

- 工作流节点虚拟滚动（大量节点时）
- 文件树懒加载（按需展开目录）
- Markdown预览防抖渲染
- 组件按需加载（Settings等弹窗lazy import）

### 8.2 后端

- LLM流式响应，首字延迟最小化
- Sidecar进程常驻，避免冷启动
- SQLite WAL模式，读写并发
- 版本快照增量存储（未来优化）

### 8.3 Sidecar

- Python进程池（处理并发请求）
- 大文件分块处理
- 临时文件自动清理
