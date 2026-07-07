# DocAgent - AI文档处理Agent桌面应用 产品需求文档（PRD）

> 版本：v2.0  
> 日期：2026-06-14  
> 状态：已实现 ✓

---

## 1. 产品概述

### 1.1 产品定位

DocAgent 是一款专注于文档处理的 AI Agent 桌面应用，面向软件开发者及频繁处理文档的用户群体。用户通过自然语言对话驱动 Agent 完成文档的生成、读取、修改、格式转换等操作，支持 Word、Excel、PPT、PDF、Markdown 等多种文档格式。

### 1.2 核心价值

- **自然语言驱动**：用户无需学习复杂工具，通过对话即可完成文档操作
- **多格式覆盖**：一站式处理 Word/Excel/PPT/PDF/Markdown 等主流文档格式
- **本地优先**：除 LLM API 调用外，所有功能均在本地运行，保障数据安全
- **高度可配**：用户自主配置 LLM 服务、Prompt 模板、快捷键等

### 1.3 目标用户

| 用户类型 | 典型场景 |
|---------|---------|
| 软件开发者 | 生成技术文档、API文档、README、将Markdown转为Word/PPT |
| 项目经理 | 生成周报/月报、整理Excel数据、制作汇报PPT |
| 技术写作者 | Markdown写作与排版、文档格式转换 |
| 数据分析师 | Excel数据处理与报表生成、数据可视化文档输出 |
| 学生/研究人员 | 论文排版、文献整理、笔记转正式文档 |

---

## 2. 技术架构（实际实现）

### 2.1 技术栈

| 层级 | 技术选型 | 说明 |
|------|---------|------|
| 桌面框架 | Tauri 2.2.x | Rust后端，轻量高性能，无边框窗口 |
| 前端框架 | React 19 + TypeScript 5 | 组件化开发，类型安全 |
| UI组件库 | Shadcn/ui + Radix | 白色简约风格 |
| 样式方案 | Tailwind CSS 4 | 工具类优先 |
| 状态管理 | Zustand 5 | 轻量状态管理 |
| 本地数据库 | SQLite (rusqlite, bundled) | 会话历史、版本快照、模板 |
| 配置存储 | JSON文件 | LLM配置、应用设置、工作区配置 |
| 文档处理 | python-docx / openpyxl / python-pptx / PyMuPDF / reportlab | 通过 Tauri Sidecar 调用 Python 脚本 |
| Markdown渲染 | react-markdown + remark-gfm + rehype-highlight | 实时渲染预览 |
| PDF预览 | pdfjs-dist | 应用内 PDF 渲染 |
| 代码执行 | write_script + run_command Tool（Git Bash）| 智能体编写脚本并通过 bash 执行 |
| 国际化 | react-i18next + i18next | zh-CN / en-US |

### 2.2 架构概览

```
+--------------------------------------------------+
|                   Tauri 2 Shell                   |
|  +--------------------------------------------+  |
|  |            React 19 + TypeScript UI        |  |
|  |  +------------------+  +----------------+  |  |
|  |  |   主界面区(3/4)    |  |  右侧栏(1/4)  |  |  |
|  |  |  - Agent工作流    |  |  - 文件树     |  |  |
|  |  |  - LLM输出       |  |  - Agent信息  |  |  |
|  |  |  - 输入框        |  |  - Token统计  |  |  |
|  |  |  - 预览浮层      |  |               |  |  |
|  |  +------------------+  +----------------+  |  |
|  +--------------------------------------------+  |
|  +--------------------------------------------+  |
|  |              Tauri Rust Backend             |  |
|  |  - LLM API适配层（OpenAI/Claude/Gemini）    |  |
|  |  - AgentExecutor（Tool Calling循环）         |  |
|  |  - 4个Handler（read/convert/analyze）       |  |
|  |  - 10个Tool（文件系统+脚本执行）             |  |
|  |  - SQLite数据库管理（6张表）                  |  |
|  |  - 版本快照管理                              |  |
|  +--------------------------------------------+  |
|  +--------------------------------------------+  |
|  |           Python Sidecar (文档处理)          |  |
|  |  - 5个文档处理器 (word/excel/ppt/pdf/md)    |  |
|  |  - write_script + run_command Tool         |  |
|  |  - 格式转换                                 |  |
|  +--------------------------------------------+  |
+--------------------------------------------------+
```

### 2.3 数据流

```
用户输入 → AgentExecutor → LLM API（流式响应）
                              ↓
                         Tool Calling决策
                              ↓
              ┌───────────────┼───────────────┐
              │               │               │
         Handler          Tool           Tool
         (read/          (write_script/ (list/search/
          convert/        run_command)   read/write/
          analyze)                       delete/...)
              │               │               │
              ▼               ▼               ▼
        Python Sidecar    Rust 原生        Rust 原生
              │               │               │
              └───────┬───────┴───────────────┘
                      ▼
              文件系统操作（工作区目录）
                      │
                      ▼
              版本快照自动保存 → SQLite
                      │
                      ▼
              UI更新（工作流 + 文件树）
```

---

## 3. 功能需求（实际实现）

### 3.1 LLM服务配置

#### 支持的 Provider 类型
| Provider | API格式 | 适配器 |
|----------|--------|--------|
| OpenAI | OpenAI Chat Completions | openai_adapter.rs |
| Anthropic | Claude Messages API | anthropic_adapter.rs |
| Gemini | Gemini API | gemini_adapter.rs |
| Ollama | OpenAI 兼容格式 | openai_adapter.rs（复用） |
| 自定义 | OpenAI 兼容格式 | openai_adapter.rs（复用） |

#### 配置项
每个 Provider 配置包含：ID、名称、类型、API Base URL、API Key、模型名、是否默认、context_window、supports_vision、extra_params、temperature、max_tokens、top_p

#### Fallback 机制
- LlmRouter 管理多个 Provider
- 支持顺序 Fallback（按配置列表顺序尝试）
- 每 5 分钟自动健康检查，标记不可用 Provider
- Provider 切换时发射 `llm:provider_switch` 事件

### 3.2 工作区管理

- 多工作区支持（创建/切换/删除）
- 工作区列表持久化（workspaces.json）
- 文件树浏览（展开/折叠/搜索/右键菜单）
- 文件操作（创建/重命名/删除/预览/在文件管理器中打开）
- 文件监听服务（notify crate 递归监听，自动刷新）
- 路径安全校验（拒绝路径遍历攻击）

### 3.3 Agent系统

#### Tool Calling 循环
1. 用户输入自然语言指令
2. 构建上下文（System Prompt + Handler/Tool Definitions + 历史消息）
3. LLM 流式响应，分析意图
4. 返回 tool_calls 时进入执行循环
5. Agent 执行对应 Handler/Tool
6. 高风险操作需用户确认
7. 结果回注给 LLM，继续推理
8. 直到 LLM 返回纯文本响应或达到最大迭代次数（20）

#### 工作流展示
- WorkflowTimeline 时间线视图
- 7 种节点类型：User / Thinking / Content / Tool / Result / Confirm / Error
- 节点展开/折叠，流式内容实时更新
- 虚拟滚动优化长列表
- 迭代组分组展示

#### 内置 Handler（4个）
| Handler | 功能 |
|---------|------|
| docx_handler | Word 文档 read/convert/analyze |
| xlsx_handler | Excel 文档 read/convert/analyze |
| pptx_handler | PPT 文档 read/convert/analyze |
| pdf_handler | PDF 文档 read/convert/analyze |

#### 内置 Tool（10个）
| Tool | 功能 |
|------|------|
| list_directory | 列出目录内容 |
| search_files | 搜索文件 |
| read_file | 读取文本文件 |
| write_text_file | 写入文本文件 |
| delete_file | 删除文件 |
| file_info | 获取文件元数据 |
| file_exists | 检查文件/目录存在 |
| create_directory | 创建目录 |
| write_script | 将智能体生成的脚本写入临时目录 |
| run_command | 通过 Git Bash 执行命令（运行脚本） |

#### 操作确认机制
- 确认级别：Always / EditOnly / Never（在 General Settings 中配置）
- run_command 执行高风险命令需要确认
- 确认弹窗显示代码功能描述和代码摘要
- 5 分钟超时自动取消
- 通过 oneshot channel 同步等待

### 3.4 文档处理

| 格式 | read | convert | analyze | 生成/修改 |
|------|------|---------|---------|-----------|
| Word (.docx) | Handler | Handler | Handler | write_script + run_command |
| Excel (.xlsx) | Handler | Handler | Handler | write_script + run_command |
| PPT (.pptx) | Handler | Handler | Handler | write_script + run_command |
| PDF (.pdf) | Handler | Handler | Handler | write_script + run_command |
| Markdown (.md) | Handler | Handler | Handler | Tool write_text_file |

生成和修改通过 write_script + run_command Tool 编写脚本并执行。

### 3.5 文档预览

| 格式 | 预览方式 |
|------|---------|
| Markdown | react-markdown + remark-gfm + rehype-highlight |
| Word | WordDocumentView 结构化渲染（段落/标题/表格/属性） |
| Excel | ExcelTableRenderer 表格渲染（工作表标签/表头/数据） |
| PPT | PptDocumentView 结构化渲染（幻灯片/形状/文本） |
| PDF | PdfCanvasViewer（pdfjs-dist Canvas 渲染，缩放/翻页） |
| 文本 | TextPreview 纯文本渲染 |

### 3.6 版本管理

- 自动版本快照（修改前创建）
- 版本历史列表（按时间倒序）
- 版本对比（DiffView 并排/内联对比）
- 一键回滚

### 3.7 会话管理

- 会话 CRUD（create/list/get/delete/update/clear）
- 消息持久化到 SQLite
- 会话摘要管理（session_summaries 表）
- 历史会话面板（搜索/切换/删除）

### 3.8 Prompt 模板系统

- 模板 CRUD（create/get/list/update/delete）
- 模板存储在 SQLite（templates 表）
- 变量占位符 `{{variable}}`
- 内置模板 + 自定义模板

### 3.9 命令执行安全

- write_script Tool：将智能体生成的脚本写入系统临时目录 `<temp_dir>/docagent/scripts/`
- run_command Tool：通过 Git Bash 执行命令（运行脚本），支持工作目录和超时控制
- Git Bash 路径优先使用用户配置，为空时从 PATH 环境变量自动检测
- 高风险命令需要用户确认
- 默认超时 60 秒，可在设置中配置

---

## 4. 界面设计（实际实现）

### 4.1 整体布局

```
+------------------------------------------------------------------+
| [窗口控件] 工作区名称 ▼ | [历史] [新会话]  [设置]                  |
+------------------------------------------------------------------+
|                                        |                          |
|   工作流时间线                          |  文件树                   |
|   ┌─────────────────────────────────┐  |  ┌────────────────────┐  |
|   │ [用户] 帮我生成一份报告          │  |  │ 📁 工作区           │  |
|   │ [思考] 分析需求...              │  |  │  ├── report.docx   │  |
|   │ [内容] 我来帮你生成...           │  |  │  ├── data.xlsx     │  |
|   │ [工具] 调用 write_script + run_command │  |  │  └── slides.pptx   │  |
|   │ [结果] 文档已生成               │  |  └────────────────────┘  |
|   │ [内容] 已创建项目报告.docx      │  |  Agent信息              |
|   └─────────────────────────────────┘  |  ┌────────────────────┐  |
|                                        |  │ 🤖 GPT-4o          │  |
|   ┌─────────────────────────────────┐  |  │ 👤 作者名           │  |
|   │ [模板▼] 输入消息... [发送]      │  |  └────────────────────┘  |
|   └─────────────────────────────────┘  |  Token统计              |
|                                        |  ┌────────────────────┐  |
|                                        |  │ 输入: 1,234 tokens  │  |
|                                        |  │ 输出: 567 tokens    │  |
|                                        |  └────────────────────┘  |
+----------------------------------------+--------------------------+
```

### 4.2 核心界面组件

- **TopBar**: 窗口控件 + 工作区选择器 + 操作按钮
- **WorkflowTimeline**: 工作流时间线（虚拟滚动）
- **InputArea**: 多行输入 + 模板选择器 + 发送按钮
- **Sidebar**: 文件树 + Agent信息 + Token统计
- **PreviewOverlay**: 文档预览浮层（懒加载）
- **SettingsDialog**: 8个标签页设置弹窗（懒加载）
- **SessionListSection**: 会话列表面板（按工作区分组）

---

## 5. 非功能需求（实际实现）

### 5.1 性能
- 首屏加载 < 2s（懒加载 + 虚拟滚动）
- 前端 Bundle < 2MB（gzip）
- 流式输出首字延迟 < 1s
- 长对话（>100 条消息）渲染流畅

### 5.2 安全
- API Key 明文存储（文件系统权限保护）
- 所有数据本地存储
- LLM API 通信使用 HTTPS
- 脚本执行

### 5.3 可靠性
- LLM 请求自动重试（指数退避）
- Sidecar 崩溃自动重启
- Agent 消息增量持久化
- 数据库损坏自动重建
- 错误边界 + 错误事件

### 5.4 兼容性
- 操作系统：Windows 10/11（首要支持）
- Python 运行时：3.12+
- 最低硬件：4GB RAM，500MB 磁盘空间

---

## 6. 未来迭代项

| 功能 | 优先级 | 说明 |
|------|--------|------|
| macOS/Linux 支持 | 高 | 跨平台适配 |
| 自定义 Handler 系统 | 中 | 用户可编写自定义 Handler |
| OCR增强 | 中 | 扫描件PDF文字识别 |
| 插件市场 | 低 | 社区共享插件 |
| 协作功能 | 低 | 多人共享工作区 |
| OCR增强 | 中 | 扫描件PDF的文字识别与提取 |
| 图表生成增强 | 中 | 更多图表类型和交互 |
| 语音输入 | 低 | 语音转文字输入 |
| Git集成 | 中 | 工作区Git版本控制集成 |
| 系统托盘 | 低 | 最小化到托盘 |
