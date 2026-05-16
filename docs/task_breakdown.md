# DocAgent AI 文档处理桌面应用 - 开发任务分解文档

> 技术栈：Tauri 2 + React + TypeScript + Rust + Python Sidecar
> 文档版本：v1.0
> 最后更新：2026-05-14

---

## 目录

- [一、项目概览](#一项目概览)
- [二、Phase 1 - MVP（核心可用）](#二phase-1---mvp核心可用)
- [三、Phase 2 - 格式扩展](#三phase-2---格式扩展)
- [四、Phase 3 - 增强体验](#四phase-3---增强体验)
- [五、Phase 4 - 打磨发布](#五phase-4---打磨发布)
- [六、关键路径分析](#六关键路径分析)
- [七、风险评估](#七风险评估)
- [八、总工期汇总](#八总工期汇总)

---

## 一、项目概览

DocAgent 是一款基于 AI Agent 的文档处理桌面应用，用户通过自然语言对话即可完成文档的生成、修改、格式转换等操作。项目采用 Tauri 2 框架，前端使用 React + TypeScript，后端核心逻辑使用 Rust，文档处理能力通过 Python Sidecar 提供。

### 开发阶段总览

| Phase | 名称 | Sprint 数量 | 预估周期 |
|-------|------|-------------|----------|
| Phase 1 | MVP（核心可用） | Sprint 1-5 | 10 周 |
| Phase 2 | 格式扩展 | Sprint 6-7 | 4 周 |
| Phase 3 | 增强体验 | Sprint 8-9 | 4 周 |
| Phase 4 | 打磨发布 | Sprint 10 | 2 周 |
| **合计** | | **10 个 Sprint** | **20 周** |

---

## 二、Phase 1 - MVP（核心可用）

> 目标：实现从用户输入到文档生成的完整闭环，支持 Word 文档的生成与修改。

### Sprint 1：项目搭建 + 基础框架（第1-2周）

#### T1.1：Tauri 2 + React + TS 项目初始化

| 属性 | 内容 |
|------|------|
| 任务ID | T1.1 |
| 名称 | Tauri 2 + React + TS 项目初始化 |
| 描述 | 使用 `create-tauri-app` 脚手架初始化项目，配置 Tauri 2 + React + TypeScript 基础工程，确保前后端通信链路畅通。包括：Tauri 2 核心依赖安装、React 18 + TypeScript 配置、Vite 构建配置、开发环境热重载验证。 |
| 前置依赖 | 无 |
| 涉及文件 | `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `vite.config.ts`, `tsconfig.json` |
| 预估工时 | 1.5 人天 |
| 验收标准 | 1) `cargo tauri dev` 可正常启动应用窗口；2) 前端热重载正常工作；3) Tauri Command 可从前端成功调用并返回结果；4) TypeScript 类型检查无报错 |

#### T1.2：项目目录结构搭建

| 属性 | 内容 |
|------|------|
| 任务ID | T1.2 |
| 名称 | 项目目录结构搭建 |
| 描述 | 按照架构设计建立完整的项目目录结构，包括前端组件目录、Rust 后端模块目录、Python Sidecar 目录、共享类型定义目录等。创建各模块的入口文件和模块声明，确保后续开发可直接在对应目录下开展工作。 |
| 前置依赖 | T1.1 |
| 涉及文件 | `src/components/`, `src/hooks/`, `src/stores/`, `src/types/`, `src-tauri/src/llm/`, `src-tauri/src/agent/`, `src-tauri/src/skills/`, `src-tauri/src/db/`, `src-tauri/src/config/`, `sidecar/`, `shared/` |
| 预估工时 | 1 人天 |
| 验收标准 | 1) 目录结构与架构设计文档一致；2) 各模块入口文件已创建；3) Rust 模块声明（mod.rs）正确关联；4) 前端目录可正确导入引用 |

#### T1.3：Tailwind CSS + Shadcn/ui 配置

| 属性 | 内容 |
|------|------|
| 任务ID | T1.3 |
| 名称 | Tailwind CSS + Shadcn/ui 配置 |
| 描述 | 集成 Tailwind CSS 作为样式方案，配置 Shadcn/ui 组件库。包括：Tailwind CSS 安装与配置、PostCSS 配置、Shadcn/ui 初始化与主题配置、全局样式变量定义（颜色、间距、圆角等）、暗色模式支持预留。 |
| 前置依赖 | T1.1 |
| 涉及文件 | `tailwind.config.js`, `postcss.config.js`, `src/styles/globals.css`, `components.json`, `src/lib/utils.ts` |
| 预估工时 | 1.5 人天 |
| 验收标准 | 1) Tailwind 工具类在组件中可正常使用；2) Shadcn/ui 组件可通过 CLI 正确添加；3) 主题变量（颜色、间距）可通过 CSS 变量统一调整；4) 暗色模式 CSS 变量已预留 |

#### T1.4：SQLite 数据库初始化与迁移

| 属性 | 内容 |
|------|------|
| 任务ID | T1.4 |
| 名称 | SQLite 数据库初始化与迁移 |
| 描述 | 在 Rust 端集成 SQLite 数据库（使用 rusqlite 或 sqlx），实现数据库的自动初始化和版本迁移机制。包括：数据库连接管理、初始表结构创建（sessions、documents、versions、messages 等）、迁移脚本框架、数据库文件路径管理。 |
| 前置依赖 | T1.2 |
| 涉及文件 | `src-tauri/src/db/mod.rs`, `src-tauri/src/db/migrations/`, `src-tauri/src/db/models.rs`, `src-tauri/src/db/connection.rs` |
| 预估工时 | 2 人天 |
| 验收标准 | 1) 应用启动时自动创建/连接 SQLite 数据库；2) 初始表结构正确创建，字段类型与数据库设计文档一致；3) 迁移脚本可正确执行版本升级；4) 数据库文件存储在应用数据目录下 |

#### T1.5：JSON 配置管理模块

| 属性 | 内容 |
|------|------|
| 任务ID | T1.5 |
| 名称 | JSON 配置管理模块 |
| 描述 | 实现基于 JSON 文件的配置管理系统，用于存储 LLM API Key、模型选择、默认参数等用户配置。包括：配置文件读写、默认配置生成、配置校验、配置变更通知、敏感信息（API Key）加密存储。 |
| 前置依赖 | T1.2 |
| 涉及文件 | `src-tauri/src/config/mod.rs`, `src-tauri/src/config/schema.rs`, `src-tauri/src/config/encryption.rs`, `src-tauri/src/config/defaults.rs` |
| 预估工时 | 1.5 人天 |
| 验收标准 | 1) 配置文件在应用数据目录自动创建；2) API Key 等敏感信息加密存储；3) 配置变更可通过 Tauri Command 读写；4) 配置格式错误时提供明确提示并回退到默认值 |

---

### Sprint 2：LLM 接入 + Agent 核心（第3-4周）

#### T1.6：LLM Provider trait 定义

| 属性 | 内容 |
|------|------|
| 任务ID | T1.6 |
| 名称 | LLM Provider trait 定义 |
| 描述 | 在 Rust 端定义统一的 LLM Provider trait，抽象不同 LLM 服务商的接口差异。包括：chat completion 接口（含流式）、token 计数接口、模型列表查询接口、错误类型定义、请求/响应结构体定义。设计需兼容 OpenAI、Claude、Gemini 等不同 API 格式。 |
| 前置依赖 | T1.2 |
| 涉及文件 | `src-tauri/src/llm/mod.rs`, `src-tauri/src/llm/provider.rs`, `src-tauri/src/llm/types.rs`, `src-tauri/src/llm/error.rs` |
| 预估工时 | 2 人天 |
| 验收标准 | 1) Provider trait 定义完整，包含 chat/stream/count_tokens 方法；2) 请求/响应结构体字段覆盖主流 LLM API 的核心参数；3) 错误类型可区分网络错误、API 错误、解析错误等；4) trait 设计可通过 mock 实现进行单元测试 |

#### T1.7：OpenAI 适配器实现（含流式响应）

| 属性 | 内容 |
|------|------|
| 任务ID | T1.7 |
| 名称 | OpenAI 适配器实现（含流式响应） |
| 描述 | 实现 Provider trait 的 OpenAI 适配器，支持与 OpenAI API（及兼容接口）的通信。包括：HTTP 客户端配置（reqwest）、请求构建与签名、SSE 流式响应解析、重试机制、速率限制处理、API Key 管理。需兼容 OpenAI 官方 API 及第三方兼容接口（如 Azure OpenAI）。 |
| 前置依赖 | T1.5, T1.6 |
| 涉及文件 | `src-tauri/src/llm/openai.rs`, `src-tauri/src/llm/stream.rs`, `src-tauri/src/llm/retry.rs` |
| 预估工时 | 3 人天 |
| 验收标准 | 1) 可成功调用 OpenAI Chat Completion API 并返回结果；2) 流式响应可逐 token 解析并推送；3) API 错误（401/429/500 等）可正确处理和重试；4) 支持自定义 base_url 以兼容第三方接口；5) 单元测试覆盖核心逻辑 |

#### T1.8：Agent 执行引擎（Tool Calling 循环）

| 属性 | 内容 |
|------|------|
| 任务ID | T1.8 |
| 名称 | Agent 执行引擎（Tool Calling 循环） |
| 描述 | 实现 AI Agent 的核心执行引擎，支持 Tool Calling 循环。包括：对话上下文管理、Tool 定义与注册、Tool Calling 解析与调度、执行结果回注、循环终止条件判断、最大迭代次数限制、中间状态持久化。Agent 在收到用户输入后，循环执行"LLM 推理 -> Tool 调用 -> 结果回注"直到任务完成。 |
| 前置依赖 | T1.6, T1.11 |
| 涉及文件 | `src-tauri/src/agent/mod.rs`, `src-tauri/src/agent/engine.rs`, `src-tauri/src/agent/context.rs`, `src-tauri/src/agent/executor.rs` |
| 预估工时 | 4 人天 |
| 验收标准 | 1) Agent 可根据用户意图自动选择并调用 Tool；2) Tool Calling 循环可正确执行多轮；3) 循环在任务完成或达到最大次数时正确终止；4) 每轮中间状态可通过事件系统推送；5) 对话上下文在多轮 Tool Calling 中正确维护 |

#### T1.9：Tauri 事件系统（流式推送）

| 属性 | 内容 |
|------|------|
| 任务ID | T1.9 |
| 名称 | Tauri 事件系统（流式推送） |
| 描述 | 基于 Tauri 2 的事件系统实现后端到前端的流式数据推送。包括：事件类型定义（token 流、Tool 调用状态、Agent 思考过程、错误通知等）、事件序列化与反序列化、前端事件监听与取消、事件缓冲与背压处理。确保 LLM 的流式输出可实时显示在前端。 |
| 前置依赖 | T1.2 |
| 涉及文件 | `src-tauri/src/events/mod.rs`, `src-tauri/src/events/types.rs`, `src-tauri/src/events/emitter.rs`, `src/hooks/useAgentEvents.ts` |
| 预估工时 | 2 人天 |
| 验收标准 | 1) Rust 端可向前端推送自定义事件；2) 前端可实时接收并处理流式 token；3) 事件类型可区分文本输出、Tool 调用、状态变更等；4) 组件卸载时正确取消事件监听，无内存泄漏 |

#### T1.10：前端 Agent 交互 Hook

| 属性 | 内容 |
|------|------|
| 任务ID | T1.10 |
| 名称 | 前端 Agent 交互 Hook |
| 描述 | 封装前端与 Agent 交互的核心逻辑为 React Hook，提供统一的 Agent 调用接口。包括：`useAgent` Hook（发送消息、接收流式响应、管理对话状态）、`useAgentStream` Hook（处理流式 token 拼接与显示）、加载状态管理、错误状态管理、中断/取消请求能力。 |
| 前置依赖 | T1.9 |
| 涉及文件 | `src/hooks/useAgent.ts`, `src/hooks/useAgentStream.ts`, `src/types/agent.ts`, `src/stores/agentStore.ts` |
| 预估工时 | 2.5 人天 |
| 验收标准 | 1) `useAgent` 可发送消息并接收流式响应；2) 流式 token 可实时渲染到 UI；3) 支持中断正在进行的 Agent 执行；4) 错误状态可正确捕获和展示；5) 对话历史在 Hook 中正确维护 |

---

### Sprint 3：基础 Skill + 文档处理（第5-6周）

#### T1.11：Skill 接口与注册表

| 属性 | 内容 |
|------|------|
| 任务ID | T1.11 |
| 名称 | Skill 接口与注册表 |
| 描述 | 定义 Skill 的统一接口和注册表机制。Skill 是 Agent 可调用的工具，包括 Rust 原生 Skill 和 Python Sidecar Skill 两种类型。包括：Skill trait 定义（名称、描述、参数 schema、执行方法）、Skill 注册表（注册/查询/列举）、Skill 参数校验（基于 JSON Schema）、Skill 调用结果标准化。 |
| 前置依赖 | T1.2 |
| 涉及文件 | `src-tauri/src/skills/mod.rs`, `src-tauri/src/skills/trait.rs`, `src-tauri/src/skills/registry.rs`, `src-tauri/src/skills/schema.rs` |
| 预估工时 | 2 人天 |
| 验收标准 | 1) Skill trait 定义清晰，包含 name/description/parameters/execute；2) 注册表支持动态注册和查询；3) 参数校验基于 JSON Schema 可正确拦截非法输入；4) Skill 列表可序列化为 LLM Tool 定义格式 |

#### T1.12：Python Sidecar 管理器

| 属性 | 内容 |
|------|------|
| 任务ID | T1.12 |
| 名称 | Python Sidecar 管理器 |
| 描述 | 实现对 Python Sidecar 进程的生命周期管理。包括：Sidecar 进程启动与关闭、进程健康检查、stdin/stdout 通信协议（JSON-RPC 风格）、请求超时处理、进程异常重启、Python 环境检测与依赖检查。Sidecar 作为独立进程运行，通过标准输入输出与 Rust 通信。 |
| 前置依赖 | T1.2, T1.11 |
| 涉及文件 | `src-tauri/src/sidecar/mod.rs`, `src-tauri/src/sidecar/manager.rs`, `src-tauri/src/sidecar/protocol.rs`, `sidecar/main.py`, `sidecar/requirements.txt` |
| 预估工时 | 3 人天 |
| 验收标准 | 1) Sidecar 进程可随应用启动和关闭；2) Rust 可通过 stdin/stdout 向 Python 发送请求并接收响应；3) 进程异常退出时可自动重启；4) 请求超时后可正确回收资源；5) Python 依赖缺失时给出明确提示 |

#### T1.13：generate_document Skill（Word）

| 属性 | 内容 |
|------|------|
| 任务ID | T1.13 |
| 名称 | generate_document Skill（Word） |
| 描述 | 实现基于 python-docx 的 Word 文档生成 Skill。Agent 通过此 Skill 可根据用户描述生成结构化的 Word 文档。包括：文档创建、标题/段落/列表/表格等元素生成、样式应用、文件保存到工作区、生成结果返回（文件路径、页数等元信息）。 |
| 前置依赖 | T1.11, T1.12 |
| 涉及文件 | `sidecar/skills/generate_document.py`, `sidecar/skills/docx_utils.py`, `sidecar/requirements.txt` |
| 预估工时 | 2.5 人天 |
| 验收标准 | 1) 可根据 Agent 传入的参数生成 Word 文档；2) 支持标题、段落、列表、表格等常见元素；3) 文档保存到指定工作区目录；4) 返回文件路径和基本元信息；5) 生成的文档可在 Word/WPS 中正常打开 |

#### T1.14：modify_document Skill（Word）

| 属性 | 内容 |
|------|------|
| 任务ID | T1.14 |
| 名称 | modify_document Skill（Word） |
| 描述 | 实现基于 python-docx 的 Word 文档修改 Skill。Agent 通过此 Skill 可对已有文档进行修改操作。包括：文档打开与解析、内容查找与替换、段落插入/删除/移动、样式修改、表格编辑、修改后保存（自动版本备份）。 |
| 前置依赖 | T1.11, T1.12, T1.13 |
| 涉及文件 | `sidecar/skills/modify_document.py`, `sidecar/skills/docx_utils.py` |
| 预估工时 | 2.5 人天 |
| 验收标准 | 1) 可打开已有 Word 文档并进行修改；2) 支持内容查找替换、段落操作、样式修改；3) 修改前自动创建版本备份；4) 修改后文档格式不损坏；5) 返回修改摘要信息 |

#### T1.15：read_document Skill

| 属性 | 内容 |
|------|------|
| 任务ID | T1.15 |
| 名称 | read_document Skill |
| 描述 | 实现文档内容读取 Skill，Agent 通过此 Skill 可读取文档内容以了解当前状态。包括：Word 文档内容提取（文本、结构、样式信息）、文档元数据读取（页数、字数、创建时间等）、大文档分段读取、内容摘要生成。 |
| 前置依赖 | T1.11, T1.12 |
| 涉及文件 | `sidecar/skills/read_document.py`, `sidecar/skills/docx_utils.py` |
| 预估工时 | 1.5 人天 |
| 验收标准 | 1) 可正确读取 Word 文档的文本内容；2) 可提取文档结构信息（标题层级、段落、表格）；3) 大文档可分段读取避免超时；4) 返回结构化的文档内容描述 |

---

### Sprint 4：基础 UI（第7-8周）

#### T1.16：顶部栏 + 工作区选择器

| 属性 | 内容 |
|------|------|
| 任务ID | T1.16 |
| 名称 | 顶部栏 + 工作区选择器 |
| 描述 | 实现应用顶部栏组件，包含应用标题、工作区选择器、全局操作按钮等。工作区选择器支持切换当前工作区、创建新工作区。顶部栏需适配 Tauri 窗口的自定义标题栏（拖拽区域、最小化/最大化/关闭按钮）。 |
| 前置依赖 | T1.3 |
| 涉及文件 | `src/components/TopBar.tsx`, `src/components/WorkspaceSelector.tsx`, `src/components/WindowControls.tsx` |
| 预估工时 | 1.5 人天 |
| 验收标准 | 1) 顶部栏正确显示应用标题和工作区选择器；2) 工作区选择器可切换和创建工作区；3) 窗口控制按钮（最小化/最大化/关闭）功能正常；4) 顶部栏支持拖拽移动窗口 |

#### T1.17：主界面区布局

| 属性 | 内容 |
|------|------|
| 任务ID | T1.17 |
| 名称 | 主界面区布局 |
| 描述 | 实现应用主界面的整体布局框架，采用三栏布局：左侧栏（会话列表）、中间区（对话 + 时间线）、右侧栏（Agent 信息 + Todo + Token 统计）。包括：响应式布局、面板折叠/展开、布局状态持久化、拖拽调整面板宽度。 |
| 前置依赖 | T1.3 |
| 涉及文件 | `src/components/MainLayout.tsx`, `src/components/LeftPanel.tsx`, `src/components/CenterPanel.tsx`, `src/components/RightPanel.tsx`, `src/stores/layoutStore.ts` |
| 预估工时 | 2 人天 |
| 验收标准 | 1) 三栏布局正确渲染，各面板内容区域可滚动；2) 面板可折叠/展开，动画流畅；3) 布局状态在应用重启后保持；4) 窗口缩放时布局自适应 |

#### T1.18：工作流时间线组件

| 属性 | 内容 |
|------|------|
| 任务ID | T1.18 |
| 名称 | 工作流时间线组件 |
| 描述 | 实现工作流时间线组件，可视化展示 Agent 的执行过程。包括：时间线节点渲染（用户输入、Agent 思考、Tool 调用、执行结果）、节点状态标识（进行中/成功/失败）、节点展开/折叠详情、自动滚动到最新节点、代码块高亮显示。 |
| 前置依赖 | T1.10, T1.17 |
| 涉及文件 | `src/components/Timeline.tsx`, `src/components/TimelineNode.tsx`, `src/components/TimelineToolCall.tsx`, `src/components/CodeBlock.tsx` |
| 预估工时 | 3 人天 |
| 验收标准 | 1) 时间线可正确渲染 Agent 执行的各步骤；2) 不同类型节点有明确的视觉区分；3) Tool 调用节点可展开查看参数和结果；4) 流式输出时时间线自动滚动；5) 代码块支持语法高亮 |

#### T1.19：输入框组件

| 属性 | 内容 |
|------|------|
| 任务ID | T1.19 |
| 名称 | 输入框组件 |
| 描述 | 实现用户输入框组件，支持多行输入和快捷操作。包括：多行文本输入（自动增高）、Enter 发送 / Shift+Enter 换行、发送按钮状态管理（发送中/可发送）、输入历史（上下箭头翻阅）、字数统计、粘贴大文本提示。 |
| 前置依赖 | T1.10, T1.17 |
| 涉及文件 | `src/components/InputBox.tsx`, `src/components/SendButton.tsx` |
| 预估工时 | 1.5 人天 |
| 验收标准 | 1) 输入框支持多行输入并自动增高；2) Enter 发送、Shift+Enter 换行正常工作；3) Agent 执行中输入框显示为禁用/取消状态；4) 输入历史可通过上下箭头翻阅 |

#### T1.20：右侧栏（Agent 信息 + Todo + Token 统计）

| 属性 | 内容 |
|------|------|
| 任务ID | T1.20 |
| 名称 | 右侧栏（Agent 信息 + Todo + Token 统计） |
| 描述 | 实现右侧栏的三个核心面板：Agent 信息面板（当前使用的模型、Provider、状态）、Todo 面板（Agent 生成的任务清单，可勾选完成）、Token 统计面板（当前会话的 Token 用量、预估费用）。 |
| 前置依赖 | T1.10, T1.17 |
| 涉及文件 | `src/components/AgentInfoPanel.tsx`, `src/components/TodoPanel.tsx`, `src/components/TokenStatsPanel.tsx` |
| 预估工时 | 2 人天 |
| 验收标准 | 1) Agent 信息面板正确显示当前模型和状态；2) Todo 面板可展示 Agent 生成的任务列表并支持勾选；3) Token 统计面板实时更新用量数据；4) 三个面板可折叠/展开 |

---

### Sprint 5：会话 + 版本（第9-10周）

#### T1.21：会话管理（创建/切换/持久化）

| 属性 | 内容 |
|------|------|
| 任务ID | T1.21 |
| 名称 | 会话管理（创建/切换/持久化） |
| 描述 | 实现会话的完整生命周期管理。包括：会话创建（自动命名）、会话切换、会话删除、会话列表查询、对话消息持久化到 SQLite、会话元数据管理（创建时间、最后活跃时间、消息数）、会话搜索。 |
| 前置依赖 | T1.4, T1.8 |
| 涉及文件 | `src-tauri/src/db/session_repo.rs`, `src-tauri/src/agent/session.rs`, `src-tauri/src/commands/session.rs`, `src/stores/sessionStore.ts`, `src/components/SessionList.tsx` |
| 预估工时 | 2.5 人天 |
| 验收标准 | 1) 可创建新会话并自动命名；2) 会话切换后对话历史正确加载；3) 会话列表按最后活跃时间排序；4) 对话消息持久化到数据库，应用重启后可恢复；5) 可删除会话及其关联数据 |

#### T1.22：版本快照服务

| 属性 | 内容 |
|------|------|
| 任务ID | T1.22 |
| 名称 | 版本快照服务 |
| 描述 | 实现文档版本快照管理服务。每次文档修改前自动创建版本快照，支持版本回溯和对比。包括：版本快照创建（文件复制 + 元数据记录）、版本列表查询、版本回滚（恢复到指定版本）、版本元数据管理（时间、触发操作、Agent 描述）、版本存储空间管理（自动清理旧版本）。 |
| 前置依赖 | T1.4, T1.14 |
| 涉及文件 | `src-tauri/src/db/version_repo.rs`, `src-tauri/src/services/version_service.rs`, `src-tauri/src/commands/version.rs` |
| 预估工时 | 2 人天 |
| 验收标准 | 1) 文档修改前自动创建版本快照；2) 可查询文档的版本历史列表；3) 可回滚到任意历史版本；4) 版本元数据包含时间、操作描述等信息；5) 版本存储空间超限时自动清理最旧版本 |

#### T1.23：历史会话面板

| 属性 | 内容 |
|------|------|
| 任务ID | T1.23 |
| 名称 | 历史会话面板 |
| 描述 | 实现左侧栏的历史会话面板，展示用户的会话历史列表。包括：会话列表渲染（名称、时间、消息预览）、会话搜索/筛选、会话分组（按日期：今天/昨天/本周/更早）、会话重命名、会话删除（带确认）、右键上下文菜单。 |
| 前置依赖 | T1.21, T1.17 |
| 涉及文件 | `src/components/SessionList.tsx`, `src/components/SessionItem.tsx`, `src/components/SessionSearch.tsx` |
| 预估工时 | 2 人天 |
| 验收标准 | 1) 会话列表按时间分组显示；2) 支持按关键词搜索会话；3) 会话可重命名和删除；4) 切换会话时 UI 正确更新；5) 右键菜单功能完整 |

#### T1.24：MVP 集成测试与 Bug 修复

| 属性 | 内容 |
|------|------|
| 任务ID | T1.24 |
| 名称 | MVP 集成测试与 Bug 修复 |
| 描述 | 对 MVP 阶段的所有功能进行集成测试，确保核心流程端到端可用。包括：用户输入 -> Agent 推理 -> Tool 调用 -> 文档生成/修改 -> 版本保存的完整流程测试、边界条件测试、错误恢复测试、性能基准测试、Bug 修复。 |
| 前置依赖 | T1.15, T1.18, T1.19, T1.20, T1.21, T1.22, T1.23 |
| 涉及文件 | 全项目 |
| 预估工时 | 3 人天 |
| 验收标准 | 1) 核心流程（对话生成文档）端到端可用；2) 无阻塞性 Bug（P0/P1）；3) 流式输出延迟 < 500ms；4) 文档生成/修改功能稳定；5) 会话和版本数据持久化可靠 |

---

## 三、Phase 2 - 格式扩展

> 目标：支持更多 LLM Provider 和文档格式，增加预览和格式转换能力。

### Sprint 6：多 Provider + 多格式（第11-12周）

#### T2.1：Claude 适配器

| 属性 | 内容 |
|------|------|
| 任务ID | T2.1 |
| 名称 | Claude 适配器 |
| 描述 | 实现 Provider trait 的 Claude（Anthropic）适配器。包括：Claude API 请求格式适配（Messages API）、流式响应解析（SSE 格式差异处理）、Claude 特有参数支持（system 消息格式、thinking 模式）、Tool Calling 格式适配、错误码映射。 |
| 前置依赖 | T1.6, T1.7 |
| 涉及文件 | `src-tauri/src/llm/claude.rs` |
| 预估工时 | 2.5 人天 |
| 验收标准 | 1) 可成功调用 Claude Messages API 并返回结果；2) 流式响应正确解析；3) Tool Calling 格式正确适配；4) Claude 特有错误码可正确处理 |

#### T2.2：Gemini 适配器

| 属性 | 内容 |
|------|------|
| 任务ID | T2.2 |
| 名称 | Gemini 适配器 |
| 描述 | 实现 Provider trait 的 Gemini（Google）适配器。包括：Gemini API 请求格式适配（generateContent / streamGenerateContent）、流式响应解析、Gemini 特有参数支持（safetySettings、generationConfig）、Function Calling 格式适配、错误码映射。 |
| 前置依赖 | T1.6, T1.7 |
| 涉及文件 | `src-tauri/src/llm/gemini.rs` |
| 预估工时 | 2.5 人天 |
| 验收标准 | 1) 可成功调用 Gemini API 并返回结果；2) 流式响应正确解析；3) Function Calling 格式正确适配；4) Gemini 特有错误码可正确处理 |

#### T2.3：Fallback 机制

| 属性 | 内容 |
|------|------|
| 任务ID | T2.3 |
| 名称 | Fallback 机制 |
| 描述 | 实现 LLM Provider 的 Fallback 机制，当主 Provider 不可用时自动切换到备用 Provider。包括：Provider 优先级配置、健康检查与自动切换、切换策略（按优先级/按延迟/按成本）、切换通知、失败回退记录。 |
| 前置依赖 | T2.1, T2.2 |
| 涉及文件 | `src-tauri/src/llm/fallback.rs`, `src-tauri/src/llm/health.rs`, `src-tauri/src/config/schema.rs` |
| 预估工时 | 2 人天 |
| 验收标准 | 1) 主 Provider 失败时自动切换到备用 Provider；2) 切换过程对用户透明，仅显示通知；3) 可配置 Provider 优先级；4) 切换记录可查询 |

#### T2.4：Excel 处理 Skill（openpyxl）

| 属性 | 内容 |
|------|------|
| 任务ID | T2.4 |
| 名称 | Excel 处理 Skill（openpyxl） |
| 描述 | 实现基于 openpyxl 的 Excel 文档处理 Skill，支持生成和修改 Excel 文件。包括：工作簿创建与打开、工作表操作（增删改查）、单元格读写（含公式）、样式设置、图表生成、数据透视表、大文件流式读取。 |
| 前置依赖 | T1.11, T1.12 |
| 涉及文件 | `sidecar/skills/excel_skill.py`, `sidecar/requirements.txt` |
| 预估工时 | 2.5 人天 |
| 验收标准 | 1) 可生成包含多工作表、公式、样式的 Excel 文件；2) 可修改已有 Excel 文件的内容和样式；3) 大文件（>10MB）可流式读取不超时；4) 生成的文件可在 Excel/WPS 中正常打开 |

#### T2.5：PPT 处理 Skill（python-pptx）

| 属性 | 内容 |
|------|------|
| 任务ID | T2.5 |
| 名称 | PPT 处理 Skill（python-pptx） |
| 描述 | 实现基于 python-pptx 的 PPT 文档处理 Skill，支持生成和修改 PowerPoint 文件。包括：演示文稿创建与打开、幻灯片操作（增删改查）、文本框与形状操作、图片插入、表格插入、母版与布局应用、动画基础支持。 |
| 前置依赖 | T1.11, T1.12 |
| 涉及文件 | `sidecar/skills/ppt_skill.py`, `sidecar/requirements.txt` |
| 预估工时 | 2.5 人天 |
| 验收标准 | 1) 可生成包含多幻灯片、文本、图片、表格的 PPT 文件；2) 可修改已有 PPT 文件；3) 支持应用母版布局；4) 生成的文件可在 PowerPoint/WPS 中正常打开 |

#### T2.6：PDF 处理 Skill（reportlab/pdfkit）

| 属性 | 内容 |
|------|------|
| 任务ID | T2.6 |
| 名称 | PDF 处理 Skill（reportlab/pdfkit） |
| 描述 | 实现基于 reportlab 和 pdfkit 的 PDF 文档处理 Skill，支持生成 PDF 文件。包括：PDF 文档创建、文本排版（段落、标题、列表）、表格生成、图片嵌入、页眉页脚、页码、目录生成、HTML 转 PDF（pdfkit）。注意：PDF 修改能力有限，主要支持生成。 |
| 前置依赖 | T1.11, T1.12 |
| 涉及文件 | `sidecar/skills/pdf_skill.py`, `sidecar/requirements.txt` |
| 预估工时 | 2 人天 |
| 验收标准 | 1) 可生成包含文本、表格、图片的 PDF 文件；2) 支持页眉页脚和页码；3) 支持 HTML 转 PDF；4) 生成的 PDF 可在阅读器中正常显示 |

---

### Sprint 7：格式转换 + 预览（第13-14周）

#### T2.7：convert_format Skill

| 属性 | 内容 |
|------|------|
| 任务ID | T2.7 |
| 名称 | convert_format Skill |
| 描述 | 实现文档格式转换 Skill，支持不同文档格式之间的转换。包括：Word -> PDF、Excel -> PDF、PPT -> PDF、Markdown -> Word、Markdown -> PDF、HTML -> Word、HTML -> PDF。转换基于已实现的各格式 Skill 组合实现，确保格式保真度。 |
| 前置依赖 | T1.13, T2.4, T2.5, T2.6 |
| 涉及文件 | `sidecar/skills/convert_format.py`, `sidecar/skills/converter/` |
| 预估工时 | 3 人天 |
| 验收标准 | 1) Word/Excel/PPT 可正确转换为 PDF；2) Markdown 可正确转换为 Word/PDF；3) 转换后格式保真度 > 90%（核心内容不丢失）；4) 转换失败时给出明确错误信息 |

#### T2.8：Markdown 预览组件（react-markdown）

| 属性 | 内容 |
|------|------|
| 任务ID | T2.8 |
| 名称 | Markdown 预览组件（react-markdown） |
| 描述 | 实现基于 react-markdown 的 Markdown 预览组件，用于在应用内预览 Markdown 内容和 Agent 输出。包括：Markdown 渲染（标题、列表、代码块、表格、链接、图片）、代码语法高亮、Mermaid 图表支持、数学公式支持（KaTeX）、自定义样式适配。 |
| 前置依赖 | T1.3 |
| 涉及文件 | `src/components/MarkdownPreview.tsx`, `src/components/CodeHighlight.tsx` |
| 预估工时 | 1.5 人天 |
| 验收标准 | 1) Markdown 内容正确渲染，样式美观；2) 代码块支持语法高亮；3) 表格、链接、图片正确显示；4) 组件性能良好，大文档渲染不卡顿 |

#### T2.9：Word/Excel/PDF 预览

| 属性 | 内容 |
|------|------|
| 任务ID | T2.9 |
| 名称 | Word/Excel/PDF 预览 |
| 描述 | 实现文档预览功能，支持在应用内预览 Word、Excel、PDF 文件。包括：PDF 预览（内嵌 PDF.js 渲染）、Word 预览（转换为 HTML 或使用在线预览）、Excel 预览（表格渲染）、预览面板（独立窗口或侧边面板）、缩放与翻页。 |
| 前置依赖 | T2.7, T2.8 |
| 涉及文件 | `src/components/DocumentPreview.tsx`, `src/components/PdfPreview.tsx`, `src/components/ExcelPreview.tsx`, `src/components/WordPreview.tsx` |
| 预估工时 | 3 人天 |
| 验收标准 | 1) PDF 文件可在应用内正确渲染和翻页；2) Word 文件可预览核心内容；3) Excel 文件可预览表格数据；4) 预览面板支持缩放操作 |

#### T2.10：差异对比预览

| 属性 | 内容 |
|------|------|
| 任务ID | T2.10 |
| 名称 | 差异对比预览 |
| 描述 | 实现文档修改前后的差异对比预览功能。包括：文本差异计算（diff 算法）、差异可视化（新增/删除/修改高亮）、并排对比视图、内联对比视图、版本间差异对比。 |
| 前置依赖 | T1.22, T2.9 |
| 涉及文件 | `src/components/DiffViewer.tsx`, `src/components/SideBySideDiff.tsx`, `src/components/InlineDiff.tsx` |
| 预估工时 | 2 人天 |
| 验收标准 | 1) 可正确计算文本差异；2) 新增/删除/修改内容有不同颜色高亮；3) 并排对比视图可同步滚动；4) 可对比任意两个版本间的差异 |

#### T2.11：工作区文件树组件

| 属性 | 内容 |
|------|------|
| 任务ID | T2.11 |
| 名称 | 工作区文件树组件 |
| 描述 | 实现工作区的文件树浏览组件，展示当前工作区下的所有文档文件。包括：文件树渲染（目录/文件图标、文件名）、文件操作（打开/重命名/删除）、文件类型图标区分、拖拽排序、右键上下文菜单、文件搜索。 |
| 前置依赖 | T1.17 |
| 涉及文件 | `src/components/FileTree.tsx`, `src/components/FileTreeItem.tsx`, `src/hooks/useFileTree.ts` |
| 预估工时 | 2 人天 |
| 验收标准 | 1) 文件树正确展示工作区目录结构；2) 不同文件类型有对应图标；3) 支持文件打开/重命名/删除操作；4) 右键菜单功能完整；5) 文件搜索可快速定位文件 |

---

## 四、Phase 3 - 增强体验

> 目标：提升用户体验，增加高级功能，完善系统健壮性。

### Sprint 8：扩展功能（第15-16周）

#### T3.1：自定义 Skill 系统

| 属性 | 内容 |
|------|------|
| 任务ID | T3.1 |
| 名称 | 自定义 Skill 系统 |
| 描述 | 实现用户自定义 Skill 的能力，允许用户编写和注册自己的 Skill。包括：Skill 模板生成、Skill 编辑器（代码编辑 + 参数定义）、Skill 注册/注销、Skill 沙箱执行（安全隔离）、Skill 分享（导入/导出）、Skill 调试工具。 |
| 前置依赖 | T1.11, T1.12 |
| 涉及文件 | `sidecar/skills/custom/`, `src-tauri/src/skills/custom_loader.rs`, `src-tauri/src/skills/sandbox.rs`, `src/components/SkillEditor.tsx`, `src/components/SkillManager.tsx` |
| 预估工时 | 4 人天 |
| 验收标准 | 1) 用户可创建自定义 Skill 并注册到系统；2) 自定义 Skill 可被 Agent 正确调用；3) Skill 执行在沙箱环境中，不影响主进程；4) Skill 可导入/导出为文件；5) Skill 编辑器支持代码高亮和参数定义 |

#### T3.2：Prompt 模板系统

| 属性 | 内容 |
|------|------|
| 任务ID | T3.2 |
| 名称 | Prompt 模板系统 |
| 描述 | 实现 Prompt 模板管理系统，用户可创建和管理常用 Prompt 模板。包括：模板创建/编辑/删除、模板分类管理、模板变量占位符、模板快速插入、内置模板库（常见文档场景）、模板分享。 |
| 前置依赖 | T1.4, T1.17 |
| 涉及文件 | `src-tauri/src/db/template_repo.rs`, `src-tauri/src/commands/template.rs`, `src/components/PromptTemplate.tsx`, `src/components/TemplateLibrary.tsx` |
| 预估工时 | 2.5 人天 |
| 验收标准 | 1) 可创建/编辑/删除 Prompt 模板；2) 模板支持变量占位符并在使用时填充；3) 内置模板库覆盖常见文档场景；4) 模板可快速插入到输入框 |

#### T3.3：Token 预算与统计

| 属性 | 内容 |
|------|------|
| 任务ID | T3.3 |
| 名称 | Token 预算与统计 |
| 描述 | 实现 Token 用量的预算管理和统计分析功能。包括：单次会话 Token 统计、日/周/月用量统计、费用估算（基于各 Provider 定价）、Token 预算设置与告警、用量趋势图表、导出用量报告。 |
| 前置依赖 | T1.20, T1.4 |
| 涉及文件 | `src-tauri/src/db/token_repo.rs`, `src-tauri/src/services/token_service.rs`, `src/components/TokenBudget.tsx`, `src/components/UsageChart.tsx` |
| 预估工时 | 2 人天 |
| 验收标准 | 1) 可实时统计当前会话的 Token 用量；2) 可查看历史用量趋势；3) 可设置 Token 预算并在接近时告警；4) 费用估算基于实际定价计算 |

#### T3.4：文档内容搜索

| 属性 | 内容 |
|------|------|
| 任务ID | T3.4 |
| 名称 | 文档内容搜索 |
| 描述 | 实现跨文档的内容搜索功能，用户可在工作区内搜索文档内容。包括：全文搜索（基于倒排索引或简单扫描）、搜索结果高亮、搜索结果按相关度排序、搜索历史、正则表达式支持。 |
| 前置依赖 | T1.15, T2.11 |
| 涉及文件 | `sidecar/skills/search.py`, `src-tauri/src/commands/search.rs`, `src/components/SearchPanel.tsx`, `src/components/SearchResults.tsx` |
| 预估工时 | 2 人天 |
| 验收标准 | 1) 可在工作区内搜索文档内容；2) 搜索结果高亮显示匹配内容；3) 搜索响应时间 < 2s（100 个文档内）；4) 支持基本正则表达式搜索 |

#### T3.5：操作确认机制（可配置级别）

| 属性 | 内容 |
|------|------|
| 任务ID | T3.5 |
| 名称 | 操作确认机制（可配置级别） |
| 描述 | 实现可配置的操作确认机制，在 Agent 执行关键操作前请求用户确认。包括：确认级别配置（自动执行/仅高风险确认/全部确认）、操作风险等级分类、确认对话框 UI、确认超时自动执行/取消、批量操作确认。 |
| 前置依赖 | T1.8, T1.17 |
| 涉及文件 | `src-tauri/src/agent/confirmation.rs`, `src-tauri/src/config/schema.rs`, `src/components/ConfirmDialog.tsx`, `src/components/ConfirmationSettings.tsx` |
| 预估工时 | 2 人天 |
| 验收标准 | 1) 高风险操作（删除/覆盖）触发确认对话框；2) 确认级别可配置；3) 确认对话框显示操作详情；4) 超时行为可配置（自动执行/取消） |

---

### Sprint 9：多工作区 + 元数据（第17-18周）

#### T3.6：多工作区切换

| 属性 | 内容 |
|------|------|
| 任务ID | T3.6 |
| 名称 | 多工作区切换 |
| 描述 | 实现多工作区管理功能，用户可创建和切换不同的工作区。包括：工作区创建（指定目录）、工作区切换、工作区配置独立（LLM 设置、Skill 配置等）、工作区数据隔离、工作区导入/导出、最近工作区快速切换。 |
| 前置依赖 | T1.16, T1.21, T1.4 |
| 涉及文件 | `src-tauri/src/db/workspace_repo.rs`, `src-tauri/src/services/workspace_service.rs`, `src-tauri/src/commands/workspace.rs`, `src/stores/workspaceStore.ts`, `src/components/WorkspaceManager.tsx` |
| 预估工时 | 2.5 人天 |
| 验收标准 | 1) 可创建新工作区并指定目录；2) 工作区切换后数据正确隔离；3) 各工作区配置独立；4) 最近工作区可快速切换；5) 工作区可导入/导出 |

#### T3.7：作者元数据管理

| 属性 | 内容 |
|------|------|
| 任务ID | T3.7 |
| 名称 | 作者元数据管理 |
| 描述 | 实现文档作者元数据的管理功能，用户可配置默认作者信息，Agent 在生成文档时自动填充。包括：作者信息配置（姓名、邮箱、公司等）、元数据模板管理、文档属性自动填充、元数据批量修改。 |
| 前置依赖 | T1.5, T1.13 |
| 涉及文件 | `src-tauri/src/config/metadata.rs`, `sidecar/skills/metadata_utils.py`, `src/components/MetadataSettings.tsx` |
| 预估工时 | 1.5 人天 |
| 验收标准 | 1) 可配置默认作者信息；2) 生成文档时自动填充元数据；3) 支持多个元数据模板切换；4) 可批量修改已有文档的元数据 |

#### T3.8：流式输出优化

| 属性 | 内容 |
|------|------|
| 任务ID | T3.8 |
| 名称 | 流式输出优化 |
| 描述 | 优化 Agent 流式输出的性能和用户体验。包括：Markdown 增量渲染优化（避免全量重渲染）、流式 token 缓冲与批量更新、大段代码块渲染优化、虚拟滚动支持（长对话）、渲染性能监控。 |
| 前置依赖 | T1.10, T1.18, T2.8 |
| 涉及文件 | `src/hooks/useAgentStream.ts`, `src/components/Timeline.tsx`, `src/components/MarkdownPreview.tsx`, `src/components/VirtualList.tsx` |
| 预估工时 | 2.5 人天 |
| 验收标准 | 1) 流式输出渲染帧率 > 30fps；2) 长对话（>100 条消息）滚动流畅；3) Markdown 增量渲染不闪烁；4) 内存占用在长对话场景下稳定 |

#### T3.9：设置面板完善

| 属性 | 内容 |
|------|------|
| 任务ID | T3.9 |
| 名称 | 设置面板完善 |
| 描述 | 完善应用的设置面板，整合所有配置项。包括：LLM 配置（Provider/模型/API Key/参数）、工作区配置、Skill 配置、确认机制配置、外观设置（主题/字体大小）、快捷键配置、数据管理（导出/清除）、关于页面。 |
| 前置依赖 | T1.5, T3.5 |
| 涉及文件 | `src/components/SettingsPanel.tsx`, `src/components/settings/LLMSettings.tsx`, `src/components/settings/SkillSettings.tsx`, `src/components/settings/AppearanceSettings.tsx`, `src/components/settings/ShortcutSettings.tsx` |
| 预估工时 | 2.5 人天 |
| 验收标准 | 1) 所有配置项可通过设置面板管理；2) 设置变更即时生效；3) API Key 等敏感信息安全存储和显示；4) 设置面板布局清晰，分类合理 |

---

## 五、Phase 4 - 打磨发布

> 目标：性能优化、稳定性提升、发布准备。

### Sprint 10：优化 + 发布（第19-20周）

#### T4.1：性能优化

| 属性 | 内容 |
|------|------|
| 任务ID | T4.1 |
| 名称 | 性能优化 |
| 描述 | 对应用进行全面的性能优化。包括：前端 Bundle 体积优化（代码分割、Tree Shaking）、首屏加载优化（懒加载、骨架屏）、Rust 端内存优化、数据库查询优化（索引、缓存）、Sidecar 进程池优化、大文件处理优化（流式处理）。 |
| 前置依赖 | Phase 1-3 全部任务 |
| 涉及文件 | 全项目 |
| 预估工时 | 3 人天 |
| 验收标准 | 1) 首屏加载时间 < 2s；2) 前端 Bundle 体积 < 2MB（gzip）；3) 数据库查询响应 < 100ms；4) 内存占用在正常使用场景下 < 500MB；5) 大文件（>50MB）处理不卡顿 |

#### T4.2：错误处理与恢复完善

| 属性 | 内容 |
|------|------|
| 任务ID | T4.2 |
| 名称 | 错误处理与恢复完善 |
| 描述 | 完善应用的错误处理和恢复机制。包括：全局错误边界（React Error Boundary）、Rust 端 panic 恢复、Sidecar 进程崩溃恢复、网络异常处理与重试、数据损坏恢复、错误日志收集与上报、用户友好的错误提示。 |
| 前置依赖 | Phase 1-3 全部任务 |
| 涉及文件 | `src/components/ErrorBoundary.tsx`, `src-tauri/src/error.rs`, `src-tauri/src/sidecar/manager.rs`, `src-tauri/src/recovery.rs` |
| 预估工时 | 2.5 人天 |
| 验收标准 | 1) 前端组件错误不会导致白屏；2) Rust panic 不导致应用崩溃；3) Sidecar 崩溃后可自动恢复；4) 网络异常有明确提示和重试选项；5) 错误日志可导出用于问题排查 |

#### T4.3：UI 细节打磨

| 属性 | 内容 |
|------|------|
| 任务ID | T4.3 |
| 名称 | UI 细节打磨 |
| 描述 | 对 UI 进行细节打磨，提升视觉品质和交互体验。包括：动画与过渡效果优化、暗色模式完善、无障碍访问支持（键盘导航、屏幕阅读器）、响应式布局微调、空状态设计、加载状态设计、微交互反馈。 |
| 前置依赖 | Phase 1-3 全部任务 |
| 涉及文件 | 全项目前端组件 |
| 预估工时 | 2.5 人天 |
| 验收标准 | 1) 所有交互有适当的动画反馈；2) 暗色模式下所有界面可正常使用；3) 核心功能支持键盘操作；4) 空状态和加载状态有友好提示；5) 无明显的 UI 对齐/间距问题 |

#### T4.4：自动更新机制

| 属性 | 内容 |
|------|------|
| 任务ID | T4.4 |
| 名称 | 自动更新机制 |
| 描述 | 基于 Tauri 2 的 updater 插件实现应用自动更新。包括：更新检查（定时/启动时）、更新通知、增量更新支持、更新回滚、更新通道管理（stable/beta）、更新日志展示。 |
| 前置依赖 | T4.1 |
| 涉及文件 | `src-tauri/tauri.conf.json`, `src-tauri/src/updater.rs`, `src/components/UpdateNotification.tsx` |
| 预估工时 | 1.5 人天 |
| 验收标准 | 1) 应用启动时自动检查更新；2) 有新版本时弹出通知；3) 可选择立即更新或稍后更新；4) 更新失败可回滚到上一版本；5) 支持稳定版和测试版通道 |

#### T4.5：应用打包与分发

| 属性 | 内容 |
|------|------|
| 任务ID | T4.5 |
| 名称 | 应用打包与分发 |
| 描述 | 完成应用的打包和分发准备。包括：Windows 安装包（NSIS/MSI）配置、macOS DMG 配置（如需）、应用图标与资源打包、代码签名、Sidecar 打包（Python 环境嵌入）、CI/CD 构建流水线配置。 |
| 前置依赖 | T4.1, T4.2 |
| 涉及文件 | `src-tauri/tauri.conf.json`, `.github/workflows/build.yml`, `sidecar/bundle.py`, `assets/` |
| 预估工时 | 2.5 人天 |
| 验收标准 | 1) Windows 安装包可正常安装和卸载；2) 安装包包含 Python Sidecar 运行环境；3) 应用图标和资源正确打包；4) CI/CD 可自动构建安装包；5) 安装包体积合理（< 200MB） |

#### T4.6：用户文档编写

| 属性 | 内容 |
|------|------|
| 任务ID | T4.6 |
| 名称 | 用户文档编写 |
| 描述 | 编写面向用户的帮助文档。包括：快速入门指南、功能说明文档、常见问题 FAQ、快捷键列表、Skill 开发指南、更新日志。文档集成到应用内（帮助面板）和在线文档站点。 |
| 前置依赖 | Phase 1-4 全部功能稳定 |
| 涉及文件 | `docs/user-guide/`, `src/components/HelpPanel.tsx` |
| 预估工时 | 2 人天 |
| 验收标准 | 1) 快速入门指南覆盖核心使用流程；2) 所有功能有对应的说明文档；3) FAQ 覆盖常见问题；4) 文档可在应用内访问 |

---

## 六、关键路径分析

### 关键路径定义

关键路径是项目中最长的任务依赖链，决定了项目的最短完成时间。任何关键路径上的任务延迟都会直接导致项目延期。

### 关键路径 1：Agent 核心链路（最长路径）

```
T1.1 → T1.2 → T1.6 → T1.7 → T1.8 → T1.13 → T1.14 → T1.24
                  ↓
             T1.11 → T1.12 → T1.13
```

**详细路径与工时：**

| 步骤 | 任务 | 工时 | 累计 |
|------|------|------|------|
| 1 | T1.1 项目初始化 | 1.5d | 1.5d |
| 2 | T1.2 目录结构搭建 | 1d | 2.5d |
| 3 | T1.6 LLM Provider trait | 2d | 4.5d |
| 4 | T1.7 OpenAI 适配器 | 3d | 7.5d |
| 5 | T1.8 Agent 执行引擎 | 4d | 11.5d |
| 6 | T1.13 generate_document Skill | 2.5d | 14d |
| 7 | T1.14 modify_document Skill | 2.5d | 16.5d |
| 8 | T1.24 MVP 集成测试 | 3d | 19.5d |

**关键路径 1 总工时：19.5 人天**

### 关键路径 2：UI 完整链路

```
T1.1 → T1.3 → T1.17 → T1.18 → T1.24
```

| 步骤 | 任务 | 工时 | 累计 |
|------|------|------|------|
| 1 | T1.1 项目初始化 | 1.5d | 1.5d |
| 2 | T1.3 Tailwind + Shadcn/ui | 1.5d | 3d |
| 3 | T1.17 主界面区布局 | 2d | 5d |
| 4 | T1.18 工作流时间线 | 3d | 8d |
| 5 | T1.24 MVP 集成测试 | 3d | 11d |

**关键路径 2 总工时：11 人天**

### 关键路径 3：格式扩展链路

```
T1.13/T2.4/T2.5/T2.6 → T2.7 → T2.9 → T2.10
```

| 步骤 | 任务 | 工时 | 累计 |
|------|------|------|------|
| 1 | T2.4 Excel Skill | 2.5d | 2.5d |
| 2 | T2.7 convert_format | 3d | 5.5d |
| 3 | T2.9 文档预览 | 3d | 8.5d |
| 4 | T2.10 差异对比 | 2d | 10.5d |

**关键路径 3 总工时：10.5 人天**

### 可并行任务分析

以下任务之间无直接依赖关系，可并行开发：

| 并行组 | 可并行的任务 | 建议人员分配 |
|--------|-------------|-------------|
| Sprint 1 并行组 | T1.3（前端样式） / T1.4（数据库） / T1.5（配置管理） | 前端 1人 + 后端 2人 |
| Sprint 2 并行组 | T1.7（OpenAI适配器） / T1.9（事件系统） / T1.11（Skill接口） | 后端 3人 |
| Sprint 3 并行组 | T1.13（生成Skill） / T1.15（读取Skill） | Python 2人 |
| Sprint 4 并行组 | T1.16（顶部栏） / T1.18（时间线） / T1.19（输入框） / T1.20（右侧栏） | 前端 2-3人 |
| Sprint 6 并行组 | T2.1（Claude） / T2.2（Gemini） / T2.4（Excel） / T2.5（PPT） / T2.6（PDF） | 后端 2人 + Python 2人 |
| Sprint 8 并行组 | T3.1（自定义Skill） / T3.2（Prompt模板） / T3.4（搜索） | 后端 1人 + 前端 1人 + Python 1人 |

### 关键里程碑

| 里程碑 | 时间节点 | 交付物 |
|--------|---------|--------|
| M1：框架就绪 | 第2周末 | 项目框架搭建完成，前后端通信链路畅通 |
| M2：Agent 可用 | 第4周末 | Agent 可调用 LLM 并执行 Tool Calling 循环 |
| M3：文档可生成 | 第6周末 | 可通过对话生成和修改 Word 文档 |
| M4：MVP 完成 | 第10周末 | 核心流程端到端可用，可进行内部演示 |
| M5：多格式支持 | 第14周末 | 支持 Word/Excel/PPT/PDF 及格式转换和预览 |
| M6：功能完整 | 第18周末 | 所有功能开发完成 |
| M7：正式发布 | 第20周末 | 应用打包发布 |

---

## 七、风险评估

### 风险矩阵

| 风险编号 | 风险描述 | 可能性 | 影响度 | 风险等级 | 责任人 |
|----------|---------|--------|--------|---------|--------|
| R1 | LLM API 兼容性风险 | 高 | 高 | 严重 | 后端 |
| R2 | Python Sidecar 稳定性风险 | 中 | 高 | 严重 | 后端 |
| R3 | 文档格式处理复杂度风险 | 高 | 中 | 较高 | Python |
| R4 | 跨平台兼容性风险 | 中 | 中 | 中等 | 全栈 |
| R5 | Tauri 2 生态成熟度风险 | 低 | 高 | 中等 | 全栈 |
| R6 | 性能瓶颈风险 | 中 | 中 | 中等 | 全栈 |
| R7 | LLM API 成本风险 | 中 | 低 | 较低 | 产品 |

### 风险详细分析

#### R1：LLM API 兼容性风险 [严重]

**描述：** 不同 LLM Provider 的 API 格式差异较大，尤其是 Tool Calling 的实现方式各不相同。OpenAI 使用 function_call/function，Claude 使用 tool_use 内容块，Gemini 使用 functionCall。此外，各家 API 的错误码、速率限制策略、流式响应格式也有差异。

**影响：** 如果适配层设计不够灵活，每新增一个 Provider 都需要大量适配工作；Tool Calling 格式差异可能导致 Agent 执行失败。

**缓解措施：**
1. 在 T1.6 阶段充分设计 Provider trait 的抽象层，预留足够的扩展性
2. 建立统一的 Tool Calling 中间格式，各适配器负责格式转换
3. 为每个适配器编写完整的集成测试用例
4. 持续关注各 Provider 的 API 变更，及时更新适配器
5. 预留 T2.1/T2.2 的缓冲时间

#### R2：Python Sidecar 稳定性风险 [严重]

**描述：** Python Sidecar 作为独立进程运行，可能面临进程崩溃、内存泄漏、通信超时、僵尸进程等问题。Python 的 GIL 限制可能影响并发处理能力。Sidecar 的打包和分发（嵌入 Python 环境）也是技术难点。

**影响：** Sidecar 不稳定会导致文档处理功能不可用，严重影响用户体验。进程管理不当可能导致资源泄漏。

**缓解措施：**
1. 在 T1.12 阶段实现完善的进程健康检查和自动重启机制
2. 为 Sidecar 通信增加超时和重试机制
3. 实现请求级别的资源隔离，避免单个请求影响整体服务
4. 监控 Sidecar 进程的内存和 CPU 使用，异常时主动重启
5. 评估使用 PyInstaller 或嵌入式 Python 打包方案，提前验证可行性
6. 考虑备选方案：将核心 Skill 用 Rust 重写（长期方案）

#### R3：文档格式处理复杂度风险 [较高]

**描述：** Word/Excel/PPT 等文档格式本身非常复杂，python-docx/openpyxl/python-pptx 等库虽然提供了基础操作能力，但在复杂场景下（如嵌套表格、复杂样式、宏、VBA 等）支持有限。格式转换的保真度难以保证 100%。

**影响：** 用户可能遇到文档格式丢失、样式异常等问题，影响对产品质量的信任。

**缓解措施：**
1. 在 Skill 设计中明确支持的范围和限制，不支持的特性给出明确提示
2. 优先保证核心元素（文本、表格、图片、基础样式）的正确性
3. 建立文档处理测试集，覆盖常见场景
4. 对于复杂格式转换，提供"尽力转换 + 手动调整"的工作流
5. 在 Agent Prompt 中引导用户使用简单清晰的文档结构

#### R4：跨平台兼容性风险 [中等]

**描述：** 应用需要在 Windows 上稳定运行，未来可能扩展到 macOS 和 Linux。Tauri 2 的跨平台能力虽然较好，但 Python Sidecar 的打包和路径管理在不同操作系统上可能有差异。文件系统路径、编码、权限等问题需要特别注意。

**影响：** 可能出现特定平台上的功能异常或崩溃。

**缓解措施：**
1. 使用 Tauri 提供的跨平台 API 处理文件路径和系统调用
2. 避免使用平台特定的 API 和硬编码路径
3. 在关键路径上增加平台检测和兼容处理
4. MVP 阶段聚焦 Windows 平台，确保核心功能稳定
5. 预留跨平台测试时间

#### R5：Tauri 2 生态成熟度风险 [中等]

**描述：** Tauri 2 相比 Tauri 1 有较大架构变更，部分插件可能尚未完全适配。社区资源和文档可能不如 Electron 丰富。遇到问题时可能需要深入源码排查。

**影响：** 开发过程中可能遇到框架层面的 Bug 或限制，需要额外时间解决。

**缓解措施：**
1. 项目初期（T1.1）充分验证 Tauri 2 的核心能力
2. 关注 Tauri 2 的 GitHub Issues 和 Release Notes
3. 优先使用 Tauri 官方插件，减少第三方依赖
4. 遇到框架问题时及时向社区反馈
5. 保留降级到 Tauri 1 或 Electron 的备选方案（最后手段）

#### R6：性能瓶颈风险 [中等]

**描述：** 大文档处理、长对话历史、流式输出渲染等场景可能出现性能瓶颈。SQLite 在高并发写入时可能成为瓶颈。前端长列表渲染可能导致卡顿。

**影响：** 用户体验下降，操作响应缓慢。

**缓解措施：**
1. 在 T3.8 阶段专门优化流式输出性能
2. 在 T4.1 阶段进行全面的性能优化
3. 大文档采用流式处理，避免一次性加载到内存
4. 前端长列表使用虚拟滚动
5. 数据库操作使用连接池和预编译语句
6. 建立性能基准测试，持续监控

#### R7：LLM API 成本风险 [较低]

**描述：** Agent 的 Tool Calling 循环可能消耗大量 Token，尤其是复杂任务需要多轮调用时。用户可能对 API 费用缺乏预期。

**影响：** 用户可能因费用问题减少使用，影响产品采纳。

**缓解措施：**
1. 在 T3.3 阶段实现 Token 预算和费用统计功能
2. 在 Agent 策略中优化 Prompt 长度，减少不必要的上下文
3. 支持配置 Token 预算上限，超出时提醒用户
4. 提供本地模型（Ollama 等）支持作为低成本替代方案

---

## 八、总工期汇总

### 各 Phase 工时统计

| Phase | 任务数 | 总工时（人天） | Sprint 数量 | 日历周期 |
|-------|--------|---------------|-------------|---------|
| Phase 1 - MVP | 24 | 56 人天 | 5 | 10 周 |
| Phase 2 - 格式扩展 | 11 | 28 人天 | 2 | 4 周 |
| Phase 3 - 增强体验 | 9 | 21.5 人天 | 2 | 4 周 |
| Phase 4 - 打磨发布 | 6 | 14 人天 | 1 | 2 周 |
| **合计** | **50** | **119.5 人天** | **10** | **20 周** |

### 团队配置建议

| 角色 | 人数 | 职责范围 |
|------|------|---------|
| Rust 后端开发 | 2 | Tauri 命令、LLM 适配器、Agent 引擎、数据库、Sidecar 管理 |
| React 前端开发 | 2 | UI 组件、状态管理、Hook 封装、样式实现 |
| Python 开发 | 1 | Sidecar Skill 开发、文档处理逻辑、格式转换 |
| 全栈/DevOps | 1 | 项目搭建、CI/CD、打包分发、性能优化 |

### 工时分布图（按 Sprint）

```
Sprint 1  ████████████████████░░░░░░░░░░  7.5 人天
Sprint 2  ██████████████████████████████  13.5 人天
Sprint 3  ████████████████████████████░░  11.5 人天
Sprint 4  ██████████████████████████░░░░  10 人天
Sprint 5  ████████████████████████████░░  9.5 人天
Sprint 6  ██████████████████████████████  14 人天
Sprint 7  ████████████████████████████░░  11.5 人天
Sprint 8  ██████████████████████████████  12.5 人天
Sprint 9  ████████████████████████░░░░░░  9 人天
Sprint 10 ██████████████████████████░░░░  14 人天
```

### 缓冲时间建议

考虑到技术风险和不确定性，建议在以下节点预留缓冲时间：

| 节点 | 缓冲时间 | 原因 |
|------|---------|------|
| Sprint 2 结束后 | +3 天 | LLM 适配和 Agent 引擎是核心难点 |
| Sprint 3 结束后 | +2 天 | Python Sidecar 稳定性需要额外验证 |
| Sprint 6 结束后 | +2 天 | 多 Provider 适配和格式处理复杂度 |
| Sprint 10 结束后 | +5 天 | 发布前缓冲，处理遗留问题 |

**含缓冲的总工期：约 22-23 周（5.5 个月）**

---

> 本文档将随项目进展持续更新，任务状态和工时估算可能根据实际情况调整。
