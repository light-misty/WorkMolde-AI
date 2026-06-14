# DocAgent - AI文档处理Agent桌面应用 产品需求文档（PRD）

> 版本：v1.0  
> 日期：2026-05-14  
> 状态：草案

---

## 1. 产品概述

### 1.1 产品定位

DocAgent 是一款专注于文档处理的 AI Agent 桌面应用，面向软件开发者及频繁处理文档的用户群体。用户通过自然语言对话驱动 Agent 完成文档的生成、修改、删除、格式转换等操作，支持 Word、Excel、PDF、PPT、Markdown 等多种文档格式。

### 1.2 核心价值

- **自然语言驱动**：用户无需学习复杂工具，通过对话即可完成文档操作
- **多格式覆盖**：一站式处理 Word/Excel/PPT/PDF/Markdown 等主流文档格式
- **本地优先**：除 LLM API 调用外，所有功能均在本地运行，保障数据安全
- **高度可配**：用户自主配置 LLM 服务、自定义 Handler、Prompt 模板等

### 1.3 目标用户

| 用户类型 | 典型场景 |
|---------|---------|
| 软件开发者 | 生成技术文档、API文档、README、将Markdown转为Word/PPT |
| 项目经理 | 生成周报/月报、整理Excel数据、制作汇报PPT |
| 技术写作者 | Markdown写作与排版、文档格式转换、批量文档处理 |
| 数据分析师 | Excel数据处理与报表生成、数据可视化文档输出 |
| 学生/研究人员 | 论文排版、文献整理、笔记转正式文档 |

### 1.4 设计原则

1. **简约专业**：白色基调、极简设计，聚焦工作流而非聊天
2. **本地安全**：所有数据本地存储，仅 LLM API 调用需要联网
3. **用户可控**：操作确认机制、版本快照回滚、可配置确认级别
4. **渐进增强**：核心功能开箱即用，高级功能（自定义Handler、模板）按需使用

---

## 2. 技术架构

### 2.1 技术栈

| 层级 | 技术选型 | 说明 |
|------|---------|------|
| 桌面框架 | Tauri 2 | Rust后端，轻量高性能，包体小 |
| 前端框架 | React + TypeScript | 组件化开发，类型安全 |
| UI组件库 | 待定（建议 Ant Design / Shadcn UI） | 白色简约风格适配 |
| 本地数据库 | SQLite | 会话历史、版本快照元数据 |
| 配置存储 | JSON文件 | LLM配置、应用设置、工作区配置 |
| 文档处理 | python-docx / openpyxl / python-pptx / reportlab 等 | 通过Tauri Sidecar调用Python脚本 |
| Markdown渲染 | react-markdown + remark/rehype插件 | 实时渲染预览 |
| PDF预览 | pdfjs-dist | 应用内PDF渲染 |
| PDF生成 | WeasyPrint / pdfkit | 文档转PDF |

### 2.2 架构概览

```
+--------------------------------------------------+
|                   Tauri 2 Shell                   |
|  +--------------------------------------------+  |
|  |            React + TypeScript UI            |  |
|  |  +------------------+  +----------------+  |  |
|  |  |   主界面区(3/4)   |  |  右侧栏(1/4)   |  |  |
|  |  |  - Agent工作流    |  |  - Todo列表     |  |  |
|  |  |  - LLM思考链     |  |  - LLM名称      |  |  |
|  |  |  - 文档预览      |  |  - 作者名       |  |  |
|  |  +------------------+  +----------------+  |  |
|  +--------------------------------------------+  |
|  +--------------------------------------------+  |
|  |              Tauri Rust Backend             |  |
|  |  - LLM API适配层（OpenAI/Claude/Gemini）    |  |
|  |  - Handler执行引擎                            |  |
|  |  - 文件系统操作                              |  |
|  |  - SQLite数据库管理                          |  |
|  |  - 版本快照管理                              |  |
|  +--------------------------------------------+  |
|  +--------------------------------------------+  |
|  |           Python Sidecar (文档处理)          |  |
|  |  - Word处理 (python-docx)                   |  |
|  |  - Excel处理 (openpyxl)                     |  |
|  |  - PPT处理 (python-pptx)                    |  |
|  |  - PDF处理 (reportlab/pdfkit)               |  |
|  |  - 格式转换                                 |  |
|  +--------------------------------------------+  |
+--------------------------------------------------+
```

### 2.3 数据流

```
用户输入 → Agent调度器 → LLM API（流式响应）
                         ↓
                    Tool Calling决策
                         ↓
              +----------+----------+
              |         |          |
          内置Handler  自定义Handler  格式转换
              |         |          |
              ↓         ↓          ↓
        Python Sidecar / 本地脚本执行
              |
              ↓
        文件系统操作（工作区目录）
              |
              ↓
        版本快照自动保存 → SQLite记录
              |
              ↓
        UI更新（工作流展示 + 文件树刷新）
```

---

## 3. 功能需求

### 3.1 LLM服务配置

#### 3.1.1 多格式API支持

支持以下LLM API格式，用户可同时配置多个Provider：

| Provider | API格式 | 说明 |
|----------|--------|------|
| OpenAI | OpenAI Chat Completions | 兼容所有OpenAI格式服务（中转站、vLLM、Ollama等） |
| Anthropic | Claude Messages API | 官方Claude API |
| Google | Gemini API | 官方Gemini API |

#### 3.1.2 配置项

每个LLM Provider配置包含：

- **Provider类型**：OpenAI / Claude / Gemini
- **显示名称**：用户自定义的名称（如"我的GPT-4o"）
- **API Base URL**：服务端点地址
- **API Key**：密钥，加密存储
- **模型名称**：如 gpt-4o、claude-3-5-sonnet、gemini-1.5-pro
- **是否为默认模型**：标记当前激活的模型
- **高级参数**（可选）：temperature、max_tokens、top_p 等

#### 3.1.3 Fallback机制

- 支持配置备用模型，当主模型请求失败时自动切换
- 用户可配置Fallback顺序
- 切换时在UI中显示提示信息

#### 3.1.4 连接测试

- 配置完成后提供"测试连接"按钮
- 发送简单请求验证API可用性
- 显示延迟和模型信息

### 3.2 工作区管理

#### 3.2.1 多工作区支持

- 用户可创建多个工作区，每个工作区绑定一个本地文件夹路径
- 工作区列表持久化保存，支持快速切换
- 每个工作区独立维护：会话历史、版本快照

#### 3.2.2 工作区配置

- **名称**：用户自定义工作区名称
- **路径**：本地文件夹绝对路径
- **创建时间**：自动记录

#### 3.2.3 文件树浏览

- 在右侧栏或独立面板中展示当前工作区的文件目录树
- 支持展开/折叠目录
- 文件图标根据类型区分（Word/Excel/PPT/PDF/MD等）
- 右键菜单支持：预览、删除、重命名、在资源管理器中打开
- Agent操作文件后自动刷新文件树
- 高亮显示最近被Agent修改的文件

#### 3.2.4 文档内容搜索

- 工作区内全文搜索功能
- 支持按文件类型过滤搜索范围
- 搜索结果高亮显示匹配内容
- Agent可通过Handler调用搜索功能，基于搜索结果进行文档操作

### 3.3 Agent系统

#### 3.3.1 Agent架构

采用 LLM Tool Calling 模式：

1. 用户输入自然语言指令
2. LLM分析意图，决定是否调用Tool
3. 若需调用Tool，LLM返回Tool Calling请求
4. Agent调度器执行对应Handler
5. 将Handler执行结果返回LLM
6. LLM根据结果继续推理或生成最终回复
7. 支持多轮Tool Calling循环，直到任务完成

#### 3.3.2 工作流展示

主界面区上方展示Agent的完整工作流，采用时间线/流程图形式：

- **用户指令节点**：显示用户输入的指令文本
- **LLM思考节点**：展示LLM的推理过程（思考链），流式输出
- **Tool调用节点**：显示调用的Handler名称、参数、执行状态
- **执行结果节点**：显示Handler执行的结果摘要
- **最终回复节点**：Agent的最终输出

每个节点可展开查看详细信息，折叠时显示摘要。

#### 3.3.3 流式输出

- LLM的思考过程和回复实时流式显示
- Tool Calling的中间状态实时更新
- 流式输出过程中用户可随时中断

#### 3.3.4 内置Handlers

| Handler名称 | 功能 | 输入 | 输出 |
|-----------|------|------|------|
| generate_document | 生成文档 | 文档类型、内容描述、文件名 | 文件路径 |
| modify_document | 修改文档 | 文件路径、修改指令 | 修改结果 |
| delete_document | 删除文档 | 文件路径 | 删除确认 |
| convert_format | 格式转换 | 源文件路径、目标格式 | 转换后文件路径 |
| read_document | 读取文档内容 | 文件路径 | 文档内容文本 |
| search_documents | 搜索文档 | 搜索关键词、文件类型过滤 | 匹配文件列表 |
| analyze_document | 分析文档 | 文件路径、分析维度 | 分析结果 |
| list_workspace | 列出工作区文件 | 目录路径（可选） | 文件列表 |
| batch_process | 批量处理 | 文件列表、操作指令 | 处理结果列表 |

#### 3.3.5 自定义Handlers

- 用户可编写自定义Handler脚本（支持TypeScript/Python）
- 自定义Handler通过标准接口与Agent交互
- Handler声明文件定义：名称、描述（供LLM理解）、参数Schema、执行入口
- Handler脚本存储在应用数据目录（非用户工作区）
- 用户可在设置中管理（添加、编辑、删除、启用/禁用）自定义Handler

自定义Handler声明文件示例：

```json
{
  "name": "generate_changelog",
  "description": "根据Git提交记录生成CHANGELOG.md文档",
  "parameters": {
    "type": "object",
    "properties": {
      "repo_path": { "type": "string", "description": "Git仓库路径" },
      "since": { "type": "string", "description": "起始日期" }
    },
    "required": ["repo_path"]
  },
  "entry": "handlers/generate_changelog.py"
}
```

#### 3.3.6 操作确认机制

用户可在设置中配置确认级别：

| 级别 | 行为 |
|------|------|
| 全部自动确认 | Agent所有操作自动执行，无需用户确认 |
| 仅破坏性操作确认 | 删除、覆盖等破坏性操作需用户确认，其他自动执行（默认） |
| 全部需确认 | Agent所有文件操作均需用户确认 |

确认弹窗显示：操作类型、目标文件、操作描述，提供"确认"/"取消"按钮。

### 3.4 文档处理

#### 3.4.1 支持的文档格式

| 格式 | 扩展名 | 生成 | 修改 | 读取 | 转换 |
|------|--------|------|------|------|------|
| Word | .docx | 支持 | 支持 | 支持 | 支持转PDF/MD/HTML |
| Excel | .xlsx | 支持 | 支持 | 支持 | 支持转CSV/PDF/HTML |
| PowerPoint | .pptx | 支持 | 支持 | 支持 | 支持转PDF |
| PDF | .pdf | 支持 | 有限支持 | 支持（含OCR） | 支持转Word/MD |
| Markdown | .md | 支持 | 支持 | 支持 | 支持转Word/PDF/HTML/PPT |
| CSV | .csv | 支持 | 支持 | 支持 | 支持转Excel |
| HTML | .html | 支持 | 支持 | 支持 | 支持转PDF/MD/Word |

#### 3.4.2 文档生成

- Agent根据用户描述生成指定格式的文档
- 自动填充可配置的元数据（作者名、创建时间等）
- 生成后自动在工作区文件树中显示
- 可选择生成后自动打开预览

#### 3.4.3 文档修改

- Agent读取文档内容后，根据用户指令进行修改
- 修改前自动创建版本快照
- 修改完成后显示变更摘要
- 支持差异对比预览

#### 3.4.4 格式转换

- 支持上述格式之间的互转
- 转换时尽量保留原始格式和样式
- 转换结果保存在源文件同目录（可配置）
- 转换失败时提供详细错误信息

#### 3.4.5 文档处理代码隔离

- 所有文档处理的Python脚本和临时文件存储在应用数据目录中
- 不在用户工作区目录中生成任何操作代码或临时文件
- 临时文件在操作完成后自动清理

### 3.5 文档预览

#### 3.5.1 应用内预览

- 点击文件树中的文件或在Agent工作流中点击文件链接，打开预览面板
- 预览面板可覆盖主界面区或以侧边形式展示
- 各格式预览方式：

| 格式 | 预览方式 |
|------|---------|
| Markdown | 实时渲染，支持代码高亮、表格、LaTeX公式 |
| Word | 渲染为HTML预览 |
| Excel | 表格形式预览，支持Sheet切换 |
| PPT | 幻灯片缩略图 + 点击放大 |
| PDF | pdfjs-dist渲染，支持翻页和缩放 |
| CSV | 表格形式预览 |
| HTML | 内嵌渲染 |

#### 3.5.2 差异对比预览

- 当Agent修改文档后，可查看修改前后的差异
- 文本类文档：行级diff对比，新增/删除/修改高亮显示
- 二进制文档：提供修改前后的描述性对比
- 支持从差异对比界面一键回滚到修改前版本

#### 3.5.3 Markdown实时渲染

- Markdown文件支持实时渲染预览
- 支持的渲染特性：
  - 标题层级
  - 代码块（语法高亮）
  - 表格
  - 列表（有序/无序/任务列表）
  - 图片（本地路径和URL）
  - LaTeX数学公式（KaTeX）
  - Mermaid流程图
  - 脚注

### 3.6 版本管理

#### 3.6.1 自动版本快照

- Agent每次修改文档前，自动创建该文档的版本快照
- 快照存储在应用数据目录（非用户工作区）
- 快照内容：文档完整副本 + 元数据（时间戳、操作描述、关联会话ID）

#### 3.6.2 版本历史

- 每个文档的版本历史列表，按时间倒序排列
- 显示每个版本的：时间、操作描述、触发来源（Agent/用户）
- 可预览任意历史版本的内容
- 支持一键回滚到指定版本

#### 3.6.3 快照清理

- 可配置快照保留策略：按数量（保留最近N个）或按时间（保留最近N天）
- 自动清理过期快照
- 手动清理入口

### 3.7 会话管理

#### 3.7.1 会话持久化

- 所有会话数据持久化存储到SQLite数据库
- 会话数据包含：消息记录、Tool调用记录、关联工作区

#### 3.7.2 历史会话列表

- 侧边栏或独立面板展示历史会话列表
- 每个会话显示：标题（自动生成或用户编辑）、时间、关联工作区
- 支持搜索历史会话
- 支持删除历史会话

#### 3.7.3 会话恢复

- 点击历史会话可恢复完整的工作流展示
- 可在历史会话基础上继续对话

### 3.9 Prompt模板系统

#### 3.9.1 内置模板

预置常用文档操作Prompt模板：

| 模板名称 | 描述 |
|---------|------|
| 生成周报 | 根据要点生成Word格式周报 |
| Markdown转PPT | 将Markdown内容转为PPT演示文稿 |
| Excel数据摘要 | 读取Excel文件并生成数据摘要报告 |
| 论文排版 | 将Markdown论文转为符合格式的Word文档 |
| API文档生成 | 根据代码注释生成API文档 |
| 会议纪要 | 根据要点生成会议纪要文档 |
| 数据报表 | 根据Excel数据生成可视化报表文档 |

#### 3.9.2 自定义模板

- 用户可创建、编辑、删除自定义Prompt模板
- 模板支持变量占位符，格式：`{{变量名}}`
- 使用模板时弹出变量填写表单
- 模板可设置关联的文档类型

自定义模板示例：

```
请根据以下要点生成一份{{文档类型}}格式的{{文档名称}}：

要点：
{{要点内容}}

要求：
- 作者：{{作者名}}
- 风格：{{文档风格}}
- 语言：{{文档语言}}
```

### 3.10 作者元数据

- 右侧栏显示当前配置的作者名
- 作者名可点击编辑，修改后即时生效
- Agent生成文档时自动使用该作者名填充文档的Author元数据字段
- 不同工作区可配置不同的作者名（全局默认 + 工作区覆盖）

---

## 4. 界面设计

### 4.1 整体布局

```
+------------------------------------------------------------------+
|    当前工作区名称 | 工作区切换                 | 设置 | 最小化/关闭  |
+------------------------------------------------------------------+
|                                    |                              |
|         主界面区（约3/4）            |       右侧栏（约1/4）         |
|                                    |                              |
|  +------------------------------+  |  +------------------------+  |
|  |                              |  |  |  工作区文件树           |  |
|  |    Agent工作流展示区          |  |  |  - 目录结构             |  |
|  |                              |  |  |  - 文件图标             |  |
|  |  [用户指令] ──────────────    |  |  |  - 右键菜单             |  |
|  |  [LLM思考中...] ──────────   |  |  +------------------------+  |
|  |  [Tool: generate_document] ─  |  |  |  Agent信息             |  |
|  |  [执行结果] ──────────────    |  |  |  - LLM名称             |  |
|  |  [用户指令] ──────────────    |  |  |  - 作者名（可编辑）     |  |
|  |  [LLM思考中...] ──────────   |  |  +------------------------+  |
|  |                              |  |  |  Todo列表               |  |
|  |                              |  |  |  - 任务1 ✓              |  |
|  |                              |  |  |  - 任务2 ○              |  |
|  |                              |  |  |  - 任务3 ○              |  |
|  |                              |  |  +------------------------+  |
|  +------------------------------+  |                              |
+------------------------------------+------------------------------+
```

### 4.2 设计规范

#### 4.2.1 色彩体系

| 用途 | 色值 | 说明 |
|------|------|------|
| 主背景 | #FFFFFF | 白色 |
| 次背景 | #F7F8FA | 浅灰，用于侧栏、卡片背景 |
| 主文字 | #1F2329 | 深灰黑 |
| 次文字 | #646A73 | 中灰 |
| 辅助文字 | #8F959E | 浅灰 |
| 主强调色 | #3370FF | 蓝色，用于按钮、链接、活跃状态 |
| 成功色 | #34C724 | 绿色，用于完成状态 |
| 警告色 | #FF9F18 | 橙色，用于警告提示 |
| 错误色 | #F54A45 | 红色，用于错误、删除 |
| 边框色 | #E5E6EB | 分割线、边框 |

#### 4.2.2 字体

- 中文：系统默认（微软雅黑 / PingFang SC）
- 英文/代码：JetBrains Mono / SF Mono
- 标题：16-20px，加粗
- 正文：14px，常规
- 辅助文字：12px

#### 4.2.3 间距

- 基础间距单位：4px
- 组件内间距：8px / 12px
- 组件间间距：16px / 24px
- 区域间间距：24px / 32px

#### 4.2.4 圆角

- 按钮圆角：6px
- 卡片圆角：8px
- 输入框圆角：6px
- 弹窗圆角：12px

### 4.3 核心界面

#### 4.3.1 Agent工作流展示区

- 采用垂直时间线布局
- 每个工作流节点包含：
  - 节点类型图标（用户/思考/工具/结果）
  - 时间戳
  - 内容摘要（默认折叠）
  - 展开查看详情
- 用户指令节点：浅蓝背景，显示指令文本
- LLM思考节点：白色背景，流式显示思考内容，支持Markdown渲染
- Tool调用节点：浅灰背景，显示Handler名称和参数
- 执行结果节点：浅绿/浅红背景，显示成功/失败结果
- 节点之间用连接线表示执行顺序

#### 4.3.2 输入框

- 位于主界面区底部
- 多行文本输入，支持Shift+Enter换行，Enter发送
- 左侧附件按钮（可选择工作区文件作为上下文）
- 右侧发送按钮
- 输入框上方可显示当前关联的Prompt模板

#### 4.3.3 右侧栏

- 可折叠/展开
- 各区域（文件树、Agent信息、Todo列表）可独立折叠
- 文件树区域支持搜索过滤
- Todo列表实时更新Agent的任务进度

#### 4.3.4 文档预览面板

- 从主界面区右侧滑出或覆盖主界面区
- 顶部工具栏：关闭按钮、文件名、差异对比切换、在资源管理器中打开
- 差异对比模式：左右分栏，左侧为修改前，右侧为修改后

### 4.4 设置界面

通过顶部栏齿轮图标进入，以弹窗或独立页面形式展示：

#### 4.4.1 LLM配置

- Provider列表（增删改）
- 每个Provider的详细配置项
- 连接测试按钮
- Fallback顺序配置
- 默认模型选择

#### 4.4.2 工作区管理

- 工作区列表（增删改）
- 每个工作区的配置
- 默认工作区设置

#### 4.4.3 Handlers管理

- 内置Handlers列表（启用/禁用）
- 自定义Handlers管理（添加、编辑、删除）
- Handler编辑器（代码编辑 + 声明文件编辑）

#### 4.4.4 Prompt模板管理

- 内置模板列表（查看、使用）
- 自定义模板管理（增删改）
- 模板编辑器（支持变量占位符高亮）

#### 4.4.5 通用设置

- 操作确认级别
- 作者名（全局默认）
- 版本快照保留策略
- 主题设置（预留暗色模式接口）
- 快捷键配置
- 应用数据目录路径
- 语言设置

---

## 5. 数据模型

### 5.1 SQLite数据表

#### sessions（会话表）

| 字段 | 类型 | 说明 |
|------|------|------|
| id | TEXT | 会话唯一ID（UUID） |
| workspace_id | TEXT | 关联工作区ID |
| title | TEXT | 会话标题 |
| created_at | DATETIME | 创建时间 |
| updated_at | DATETIME | 更新时间 |
| llm_provider | TEXT | 使用的LLM Provider |
| llm_model | TEXT | 使用的模型名称 |

#### session_messages（会话消息表）

| 字段 | 类型 | 说明 |
|------|------|------|
| id | TEXT | 消息唯一ID |
| session_id | TEXT | 关联会话ID |
| role | TEXT | 角色：user/assistant/tool |
| content | TEXT | 消息内容 |
| tool_name | TEXT | 调用的Tool名称（role=tool时） |
| tool_args | TEXT | Tool参数JSON |
| tool_result | TEXT | Tool执行结果 |
| thinking_content | TEXT | LLM思考链内容 |
| created_at | DATETIME | 创建时间 |

#### version_snapshots（版本快照表）

| 字段 | 类型 | 说明 |
|------|------|------|
| id | TEXT | 快照唯一ID |
| workspace_id | TEXT | 关联工作区ID |
| session_id | TEXT | 关联会话ID |
| file_path | TEXT | 文件相对路径 |
| snapshot_path | TEXT | 快照文件存储路径 |
| operation | TEXT | 触发操作描述 |
| created_at | DATETIME | 创建时间 |

### 5.2 JSON配置文件

#### llm_config.json

```json
{
  "providers": [
    {
      "id": "provider-uuid-1",
      "type": "openai",
      "name": "我的GPT-4o",
      "api_base_url": "https://api.openai.com/v1",
      "api_key": "encrypted:...",
      "model": "gpt-4o",
      "is_default": true,
      "advanced": {
        "temperature": 0.7,
        "max_tokens": 4096,
        "top_p": 1
      }
    }
  ],
  "fallback_order": ["provider-uuid-1", "provider-uuid-2"]
}
```

#### app_settings.json

```json
{
  "general": {
    "author_name": "用户名",
    "confirmation_level": "destructive_only",
    "language": "zh-CN"
  },
  "version_snapshot": {
    "retention_policy": "by_count",
    "max_count": 50,
    "max_days": 30
  },
  "workspace": {
    "default_workspace_id": "ws-uuid-1"
  },
  "shortcuts": {
    "new_session": "Ctrl+N",
    "send_message": "Enter",
    "toggle_sidebar": "Ctrl+B"
  }
}
```

#### workspaces.json

```json
{
  "workspaces": [
    {
      "id": "ws-uuid-1",
      "name": "项目文档",
      "path": "D:/Documents/ProjectDocs",
      "author_name_override": null,
      "created_at": "2026-05-14T10:00:00Z"
    }
  ]
}
```

---

## 6. Handler系统详细设计

### 6.1 Handler接口规范

每个Handler（内置和自定义）必须实现以下接口：

```typescript
interface Handler {
  name: string;
  description: string;
  parameters: JSONSchema;
  execute(params: Record<string, any>): Promise<HandlerResult>;
}

interface HandlerResult {
  success: boolean;
  data?: any;
  error?: string;
  display: {
    summary: string;
    details?: string;
  };
}
```

### 6.2 内置Handler详细设计

#### generate_document

- 输入：文档类型、内容描述、文件名（可选，自动生成默认名）
- 处理：调用Python Sidecar生成文档
- 输出：生成的文件路径、文件大小
- 特殊逻辑：自动填充作者元数据

#### modify_document

- 输入：文件路径、修改指令
- 处理：读取文档 → 根据指令修改 → 保存前创建版本快照 → 写入修改
- 输出：修改结果摘要、变更的文件路径
- 特殊逻辑：修改前自动快照；根据确认级别可能需要用户确认

#### convert_format

- 输入：源文件路径、目标格式
- 处理：读取源文件 → 格式转换 → 保存目标文件
- 输出：转换后文件路径
- 特殊逻辑：保留原始格式和样式；转换失败提供详细错误

#### read_document

- 输入：文件路径
- 处理：读取文档内容，提取文本
- 输出：文档内容文本、文档元数据
- 特殊逻辑：大文件分段读取；PDF支持OCR

#### search_documents

- 输入：搜索关键词、文件类型过滤（可选）、目录范围（可选）
- 处理：遍历工作区文件，全文搜索
- 输出：匹配文件列表、匹配内容片段

#### analyze_document

- 输入：文件路径、分析维度（摘要/结构/数据统计等）
- 处理：读取文档 → 按维度分析
- 输出：分析结果文本

### 6.3 自定义Handler规范

- 存储位置：`{应用数据目录}/handlers/custom/`
- 每个Handler一个目录，包含：
  - `handler.json`：Handler声明文件
  - `main.py` 或 `main.ts`：执行入口
- Handler声明文件格式：

```json
{
  "name": "handler_name",
  "version": "1.0.0",
  "description": "Handler功能描述，供LLM理解",
  "parameters": {
    "type": "object",
    "properties": {
      "param1": {
        "type": "string",
        "description": "参数描述"
      }
    },
    "required": ["param1"]
  },
  "runtime": "python",
  "entry": "main.py",
  "enabled": true
}
```

---

## 7. LLM适配层设计

### 7.1 统一接口

```typescript
interface LLMProvider {
  chat(params: ChatParams): AsyncIterable<ChatChunk>;
  getToolCallSupport(): boolean;
}

interface ChatParams {
  messages: Message[];
  tools?: ToolDefinition[];
  temperature?: number;
  max_tokens?: number;
  stream?: boolean;
}

interface ChatChunk {
  type: 'thinking' | 'content' | 'tool_call' | 'tool_result' | 'done';
  content?: string;
  tool_call?: {
    id: string;
    name: string;
    arguments: Record<string, any>;
  };
  usage?: {
    input_tokens: number;
    output_tokens: number;
  };
}
```

### 7.2 适配器实现

- **OpenAIAdapter**：兼容OpenAI Chat Completions API格式，覆盖所有兼容服务
- **ClaudeAdapter**：适配Anthropic Messages API格式，处理Claude特殊的Tool Calling格式
- **GeminiAdapter**：适配Google Gemini API格式

### 7.3 Fallback策略

1. 主模型请求失败（网络错误、API错误、超时）
2. 自动尝试Fallback列表中的下一个模型
3. 每次Fallback在UI中显示提示
4. 所有Fallback均失败时，向用户报告错误

---

## 8. 非功能需求

### 8.1 性能

- 应用冷启动时间 < 3秒
- 文档预览加载时间 < 2秒（10MB以内文件）
- 流式输出首字延迟 < 1秒（取决于LLM API响应）
- 工作区文件树加载时间 < 1秒（1000个文件以内）

### 8.2 安全

- API Key加密存储（使用系统密钥链或AES加密）
- 所有数据本地存储，不向第三方发送用户数据
- LLM API通信使用HTTPS
- 自定义Handler执行无沙箱，但在UI中明确提示风险

### 8.3 可靠性

- LLM API请求失败时自动重试（最多3次，指数退避）
- 文档操作失败时自动回滚（恢复版本快照）
- 应用异常退出时，会话数据不丢失（实时持久化）
- 大文件操作支持取消

### 8.4 兼容性

- 操作系统：Windows 10/11（首要支持），macOS（后续支持），Linux（后续支持）
- 最低硬件：4GB RAM，500MB磁盘空间
- Python运行时：3.12+（Tauri Sidecar打包）

### 8.5 可扩展性

- Handler系统支持用户自定义扩展
- LLM适配层支持新增Provider
- Prompt模板支持用户自定义
- 预留插件系统接口（未来迭代）

---

## 9. 未来迭代项

以下功能在当前版本中不实现，记录为未来迭代候选：

| 功能 | 优先级 | 说明 |
|------|--------|------|
| 暗色模式 | 中 | 提供深色主题切换 |
| 多语言UI | 中 | 界面支持中/英文切换 |
| 协作功能 | 低 | 多人共享工作区（需联网） |
| 插件市场 | 低 | 社区共享自定义Handlers和模板 |
| OCR增强 | 中 | 扫描件PDF的文字识别与提取 |
| 图表生成 | 中 | Agent根据数据生成图表并插入文档 |
| 语音输入 | 低 | 支持语音转文字输入指令 |
| 文档对比 | 中 | 两个文档的独立对比功能 |
| Git集成 | 中 | 工作区Git版本控制集成 |
| 自动更新 | 高 | Tauri内置更新机制 |
| 系统托盘 | 低 | 最小化到托盘，后台运行 |
| 配置导入导出 | 中 | 一键迁移应用配置 |
| 拖拽文件 | 中 | 拖拽文件到应用窗口 |
| 快捷键体系 | 中 | 可自定义的键盘快捷键 |

---

## 10. 里程碑规划

### Phase 1 - MVP（核心可用）

- Tauri + React项目搭建
- LLM配置与OpenAI适配
- 基础Agent工作流（Tool Calling）
- 内置Handler：generate_document / modify_document / read_document
- Word/Markdown文档支持
- 基础UI布局（主界面区 + 右侧栏）
- 会话持久化
- 版本快照

### Phase 2 - 格式扩展

- Claude/Gemini适配
- Excel/PPT/PDF文档支持
- 格式转换Handler
- 文档预览（应用内预览 + Markdown渲染）
- 差异对比预览
- 工作区文件树
- Fallback机制

### Phase 3 - 增强体验

- 自定义Handler系统
- Prompt模板系统
- 文档内容搜索
- 操作确认机制（可配置级别）
- 多工作区切换
- 作者元数据管理
- 流式输出优化

### Phase 4 - 打磨发布

- 性能优化
- 错误处理与恢复完善
- UI细节打磨
- 自动更新机制
- 应用打包与分发
- 用户文档

---

## 附录A：术语表

| 术语 | 说明 |
|------|------|
| Agent | 基于LLM的自主决策系统，通过Tool Calling完成用户指令 |
| Handler | Agent可调用的工具，封装了特定的文档操作能力 |
| Tool Calling | LLM通过函数调用机制与外部工具交互的能力 |
| 工作区 | 用户指定的本地文件夹，作为Agent操作的根目录 |
| 版本快照 | 文档修改前的完整备份，用于回滚 |
| Sidecar | Tauri中打包的独立进程，用于运行Python脚本 |
| Fallback | 主模型失败时的备用模型切换机制 |

## 附录B：参考资源

- Tauri 2 文档：https://v2.tauri.app
- OpenAI Chat Completions API：https://platform.openai.com/docs/api-reference/chat
- Anthropic Messages API：https://docs.anthropic.com/en/api/messages
- Google Gemini API：https://ai.google.dev/api
- python-docx：https://python-docx.readthedocs.io
- openpyxl：https://openpyxl.readthedocs.io
- python-pptx：https://python-pptx.readthedocs.io
