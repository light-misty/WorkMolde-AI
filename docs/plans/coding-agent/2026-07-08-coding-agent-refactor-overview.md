# WorkMolde AI 编程 Agent 改造总体计划

> 文档版本:v1.2(2026-07-09 修订:LSP 单工具架构,新增 apply_patch/question 工具,工具清单对齐 OpenCode 13 个,grep/glob 基于 ignore crate,websearch 改为 MCP 协议,read_lines 合并到 read)
> v1.1(2026-07-08 修订:保留文档 Handler,新增 Document 模式)
> 创建日期:2026-07-08
> 改造目标:将 WorkMolde AI 从文档处理智能体改造为通用编程 Agent,参照 OpenCode 的功能实现,同时保留文档处理能力(按 Document 模式启用)
> 改造原则:全面改造,分阶段进行,保证质量,不改变现有 UI 设计

---

## 一、改造背景与目标

### 1.1 背景

WorkMolde AI 起初定位为 AI 文档处理桌面应用,基于 Tauri 2.x (Rust + React/TypeScript) 构建,通过 4 个文档 Handler (docx/xlsx/pptx/pdf,经 Python Sidecar 执行)完成文档的生成、读取、修改、格式转换。

随着产品定位调整,需将其改造为通用编程 Agent 应用(类似 Claude Code、OpenCode、Codex),让智能体通过编写代码处理任何事情。**但文档处理能力并不废弃**:通过新增 `Document` Agent 模式(与 Plan/Build 同级),用户切换到 Document 模式后,4 个文档 Handler 才被动态启用(出现在 LLM 可见的工具列表中);在 Plan/Build 模式下,这些 Handler 不会出现在工具列表中,LLM 完全感知不到它们的存在。

> **重要说明**:`Document` 模式为 WorkMolde AI 原创设计,OpenCode 实际只有 `build`/`plan` 两个 primary 模式(外加 `general`/`explore` 等 subagent)。WorkMolde AI 新增 Document 模式的目的是保留原有的文档处理能力,使应用在获得编程 Agent 能力的同时不丢失文档处理特色。

### 1.2 改造目标

参照开源编程 Agent [OpenCode](https://github.com/sst/opencode) (sst/opencode, branch 2.0) 的功能实现,对 WorkMolde AI 进行大型改造:

1. **系统提示词架构**:从"文档处理专家"重构为"编程 Agent",引入 AGENTS.md 机制、Agent 类型特定 prompt
2. **内置工具链**:保留文档 Handler(按模式动态启用),新增编程核心工具(edit/glob/grep/todowrite/task/webfetch/lsp 等)
3. **Agent 模式**:实现 Plan(只读规划)/Build(执行修改)/Document(文档处理)三态模式切换,文档 Handler 仅在 Document 模式下出现在工具列表中
4. **权限系统**:从简单 ConfirmationLevel 升级为三态权限(allow/deny/ask) + 可持久化规则
5. **Skill 系统**:实现 SKILL.md 加载机制,按需注入领域能力
6. **子 Agent**:实现 task 工具,支持 Agent 嵌套调用
7. **LSP 集成**:实现 LSP 客户端,支持跳转定义、查找引用、诊断反馈
8. **上下文管理**:实现 SessionCompaction(上下文压缩)、Doom loop 检测

### 1.3 改造原则

- **不改变 UI 设计**:前端组件基本不动,仅在必要时增加模式切换、权限对话框等交互元素
- **保留良好基础**:复用 WorkMolde AI 现有的 LlmRouter、流式事件机制、确认通道、增量持久化、Scratchpad 等已验证的设计
- **分阶段交付**:按依赖关系分 5 个阶段,每阶段独立可交付、可测试
- **质量优先**:每阶段配套测试用例,遵循 TDD(测试驱动开发)

---

## 二、OpenCode vs WorkMolde AI 架构对比

### 2.1 整体架构对比

| 维度 | OpenCode | WorkMolde AI 现状 | 改造方向 |
|------|----------|---------------|----------|
| **定位** | 终端编程 Agent | 文档处理 Agent | 桌面编程 Agent |
| **语言栈** | TypeScript (Bun) | Rust + React/TypeScript | 保持 Rust + React |
| **Agent 类型** | build/plan/general/explore/scout/compaction/title/summary (8个, scout 为外部文档/依赖研究的只读子代理) | 单一 Agent | 引入 build/plan/explore/general |
| **工具数量** | 13 个核心工具(官方对齐) | 16 个 Tool + 4 个 Handler | 保留 Handler(按模式启用),对齐 OpenCode 13 个核心工具 + WorkMolde AI 扩展 |
| **权限系统** | 三态(allow/deny/ask) + 持久化规则 | ConfirmationLevel(Always/EditOnly/Never) | 升级为三态权限 |
| **Skill 系统** | .opencode/skill/*/SKILL.md | 无 | 实现 Skill 加载 |
| **LSP 集成** | 有(单一 lsp 工具,operation 参数路由) | 无 | 实现 LSP 客户端(实验性) |
| **子 Agent** | 有(task 工具) | 无 | 实现 Agent 嵌套 |
| **上下文压缩** | SessionCompaction | 无 | 实现压缩机制 |
| **规则文件** | AGENTS.md/CLAUDE.md/CONTEXT.md | 无 | 实现 AGENTS.md |
| **Agent 模式** | 有(build/plan agent) | 无 | 实现 Plan/Build/Document 三态切换(前端按钮) |

### 2.2 系统提示词架构对比

OpenCode 系统提示词架构(多段式):

> **说明**:经源码分析,OpenCode 实际采用多段式架构(非严格的 3 段),组装顺序为:基础 prompt + 环境信息 + AGENTS.md + Agent 特定 prompt + Skill 清单 + MCP 指令。

```
System Prompt
├── 基础 prompt (系统内置核心提示词,OpenCode 原实现按 Provider 分类为 default.txt / anthropic / gpt / gemini 等,WorkMolde AI 改造为统一基础 prompt,不按 Provider 区分)
├── 环境信息 (工作目录、workspace root、Git 状态、平台、日期、模型 ID)
├── 自定义规则 (AGENTS.md: 全局 ~/.config/opencode/AGENTS.md + 项目级向上查找)
├── Agent 特定 prompt (build/plan/compaction/title/summary 等)
├── Skill 清单 (XML 格式 <available_skills>)
└── MCP 指令 (XML 格式 <mcp_instructions>)
```

**WorkMolde AI 现有系统提示词(分层架构)**:

```
System Prompt
├── Layer 0: 身份层 (WorkMolde AI 文档处理专家)
├── Layer 1: 规则层
├── Layer 2: 上下文层 (workspace_path, env_info, author_info)
├── Layer 3: 策略层 (tool_strategy)
├── Layer 3.5: 工程方法论层
├── Layer 3.6: 脚本执行最佳实践层
├── Layer 4: 防幻觉层
├── Layer 5: 错误处理层
├── Layer 6: 规范层 (按需注入文档设计规范)
└── Layer 7: 示例层 (按需注入)
```

**改造方向**:采用 OpenCode 的多段式系统提示词架构(基础 prompt + 环境信息 + AGENTS.md + Agent 特定 prompt + Skill 清单),删除现有分层架构,内容彻底重写为编程 Agent。基础 prompt 保留为系统内置的统一核心提示词,不再按 Provider 区分加载。

### 2.3 工具链对比

| 工具类别 | OpenCode | WorkMolde AI 现状 | 改造方向 |
|---------|----------|---------------|----------|
| **文件读取** | read(行号、行范围、二进制保护) | read, read_file_lines | 改造 read 增加行号,合并 read_lines(通过 start_line/end_line 参数实现) |
| **文件编辑** | edit(oldString/newString 精确替换,FileTime 锁) | write_text_file(整体覆盖) | **新增 edit 工具** |
| **补丁应用** | apply_patch(补丁文件应用) | 无 | **新增 apply_patch 工具**(edit 权限类别) |
| **文件写入** | write | write_text_file | 保留 |
| **文件搜索** | glob(模式匹配) | list_directory, search_files | **新增 glob 工具**(基于 ignore crate) |
| **内容搜索** | grep(ripgrep,正则) | search_files(简单匹配) | **新增 grep 工具**(基于 ignore crate,支持 .gitignore) |
| **命令执行** | bash(AST 解析,权限扫描) | run_command(Git Bash) | 改造 bash 增强权限 |
| **任务管理** | todowrite | update_notes(草稿本) | **新增 todowrite 工具** |
| **子 Agent** | task | 无 | **新增 task 工具** |
| **网页抓取** | webfetch | 无 | **新增 webfetch 工具** |
| **网络搜索** | websearch | 无 | **新增 websearch 工具**(MCP 协议,Exa AI) |
| **用户提问** | question | 无 | **新增 question 工具**(独立权限类别) |
| **LSP** | lsp(单一工具,operation 路由 8 种操作) | 无 | **新增 lsp 工具**(阶段5,实验性) |
| **代码搜索** | sourcecode, codesearch | 无 | **新增 sourcecode 工具** |
| **脚本执行** | 无(用 bash) | write_script, run_command | 保留(WorkMolde AI 特色) |
| **文档处理** | 无 | 4 个 Handler | **保留,按 Document 模式动态启用** |

---

## 三、改造范围与阶段划分

### 3.1 改造范围总览

```
改造范围
├── 后端 Rust (src-tauri/src/)
│   ├── services/agent/         系统提示词重构、Agent 模式(Plan/Build/Document)、执行循环增强
│   ├── services/tool/          工具链重构(新增 edit/glob/grep/todowrite/task/webfetch/apply_patch/question 等)
│   ├── services/permission/    [新] 权限系统(三态决策 + 持久化规则)
│   ├── services/skill/         [新] Skill 加载系统
│   ├── services/lsp/           [新] LSP 客户端
│   ├── services/subagent/      [新] 子 Agent 执行器
│   ├── services/handler/       [保留] 文档 Handler(按 Document 模式动态启用)
│   ├── services/document/      [保留] Python Sidecar 管理(Document 模式下使用)
│   ├── commands/               命令层调整
│   ├── models/                 数据模型调整
│   └── lib.rs                  AppState 调整
│
├── 前端 TypeScript (src/)
│   ├── hooks/useAgent.ts       增加 mode 参数、权限事件处理
│   ├── stores/                 增加 permissionStore、modeStore
│   ├── services/tauri.ts       增加新命令封装
│   ├── services/event.ts       增加权限事件类型
│   ├── types/                  增加新类型定义
│   └── components/
│       ├── layout/InputArea.tsx    增加 Plan/Build/Document 三态模式切换按钮
│       ├── workflow/ToolNode.tsx   适配新工具展示
│       └── settings/               权限规则管理 UI
│
├── Python Sidecar (sidecar/)
│   └── [保留]                 Document 模式下提供文档处理能力
│
└── 配置文件
    ├── Cargo.toml              新增 lsp-types 等,保留 Sidecar 相关依赖
    ├── package.json            保留 sidecar 构建脚本
    └── tauri.conf.json         保留 sidecar 相关配置
```

### 3.2 阶段划分

改造分为 5 个阶段,按依赖关系排序:

#### 阶段 1:核心架构与工具链基础(基础阶段,必须先完成)

**目标**:建立编程 Agent 的核心能力,保留文档处理能力(后续按模式启用)

**主要任务**:
1. 保留文档 Handler (docx/xlsx/pptx/pdf) 和 Python Sidecar,为后续 Document 模式做准备
2. 重构系统提示词架构(采用 OpenCode 3 段架构:环境信息 + AGENTS.md + Agent 特定 prompt)
3. 实现 AGENTS.md 加载机制(项目级 + 全局级规则文件)
4. 新增核心编程工具:
   - `edit`:基于 oldString/newString 的精确文件编辑(带 FileTime 锁)
   - `glob`:文件模式匹配工具(基于 ignore crate,支持 .gitignore)
   - `grep`:内容搜索工具(基于 ignore crate + regex,支持 .gitignore)
   - `apply_patch`:应用补丁文件修改代码(edit 权限类别)
   - `question`:向用户提问,收集偏好或澄清需求(独立权限类别)
5. 改造现有工具:
   - `read`:增加行号显示、二进制文件保护,合并 read_lines(通过 start_line/end_line 参数实现)
   - `bash`:增强命令解析和权限控制
6. 在 executor 中预留"按 Agent 模式动态过滤工具列表"的钩子(实际过滤逻辑在阶段 2 实现)
7. 更新 Cargo.toml 和 package.json 依赖(新增 edit/glob/grep/apply_patch 所需,保留 Sidecar 依赖)

**交付物**:可运行的编程 Agent 原型,具备文件读写、编辑、搜索、命令执行能力;文档 Handler 保留但默认不启用(等待阶段 2 的模式过滤机制)

**详细文档**:[2026-07-08-coding-agent-refactor-phase1-core.md](./2026-07-08-coding-agent-refactor-phase1-core.md)

---

#### 阶段 2:权限系统与 Agent 模式

**目标**:实现 OpenCode 风格的三态权限系统和 Plan/Build/Document 三态模式切换

**主要任务**:
1. 实现三态权限系统(allow/deny/ask):
   - 权限类型:edit, read(WorkMolde AI 扩展,OpenCode 原生不管控 read 权限), bash, webfetch, task, external_directory, doom_loop, document 等
    - 用户回复:once / reject
   - 权限规则持久化(数据库 permission 表)
2. 实现 Plan/Build/Document 三态模式切换(仅前端按钮,不提供 LLM 工具):
   - Plan 模式:禁止 edit/bash 等修改类操作,只允许 read/glob/grep/list
   - Build 模式:默认允许所有编程操作(受权限规则约束),文档 Handler 不出现在工具列表
   - Document 模式:Build 模式超集 + 4 个文档 Handler 动态加入工具列表
   - 模式切换由用户通过前端 InputArea 按钮主动完成,LLM 无法自主切换
3. 实现工具列表动态过滤(基于 AgentMode):
   - executor 构建 tool_definitions 时,若 mode != Document,过滤掉 4 个文档 Handler
   - Document 模式下,4 个文档 Handler 出现在工具列表中
4. 前端增加三态模式切换按钮和权限对话框
5. 实现 Doom loop 检测(连续 3 次相同工具调用无错误)
6. 改造现有 ConfirmationLevel 为新权限系统

**交付物**:具备权限审批和三态模式切换的编程 Agent,Document 模式下可使用文档 Handler

**详细文档**:[2026-07-08-coding-agent-refactor-phase2-permission.md](./2026-07-08-coding-agent-refactor-phase2-permission.md)

---

#### 阶段 3:Skill 系统与上下文管理

**目标**:实现 Skill 加载机制和上下文压缩,提升 Agent 的领域能力和长对话保活能力

**主要任务**:
1. 实现 Skill 系统:
   - Skill 加载:从 `.agent/skills/*/SKILL.md` 加载(frontmatter + markdown 内容)
   - Skill 发现:扫描全局目录(`~/.agent/skills/`)、项目目录(`.agent/skills/`)、配置路径
   - Skill 注入:系统提示词中注入可用 Skill 清单,Agent 通过 skill 工具按需加载
   - Skill 权限:按 Agent 模式过滤可见 Skill
2. 实现 TodoWrite 工具:
   - 结构化任务管理(pending/in_progress/completed)
   - 跨迭代任务状态保持
   - 替代/整合现有 Scratchpad 草稿本
3. 实现 SessionCompaction(上下文压缩):
   - 上下文接近溢出时触发压缩
   - 生成"继续工作所需摘要"而非原样保留全部历史
   - 对旧工具输出做 prune(保留必要信息,释放 token)
4. 实现 SourceCode 工具(代码语义搜索,基于 tree-sitter)

**交付物**:具备领域能力注入和长对话保活能力的编程 Agent

**详细文档**:[2026-07-08-coding-agent-refactor-phase3-skill-context.md](./2026-07-08-coding-agent-refactor-phase3-skill-context.md)

---

#### 阶段 4:子 Agent 与高级工具

**目标**:实现 Agent 嵌套调用和网页交互能力

**主要任务**:
1. 实现子 Agent (task 工具):
   - Agent 嵌套调用:主 Agent 通过 task 工具委托复杂任务给子 Agent
   - 子 Agent 类型:explore(代码探索,只读)、general(通用多任务)
   - 子 Agent 独立上下文:不污染主 Agent 的消息历史
   - task_id 支持:可续跑被中断的子任务
2. 实现 WebFetch 工具:
   - 抓取网页内容并转为 markdown
   - 支持 URL 过滤和内容截断
3. 实现 WebSearch 工具(MCP 协议,Exa AI,JSON-RPC 2.0):
   - 网络搜索(基于 MCP 协议接入 Exa AI)
   - 返回搜索结果摘要
4. 改造 AgentExecutor 支持递归调用:
   - 子 Agent 执行器复用主执行器逻辑
   - 子 Agent 事件流隔离(不直接发射到前端)
   - 子 Agent 结果汇总给主 Agent

**交付物**:具备子 Agent 委托和网页交互能力的编程 Agent

**详细文档**:[2026-07-08-coding-agent-refactor-phase4-subagent-tools.md](./2026-07-08-coding-agent-refactor-phase4-subagent-tools.md)

---

#### 阶段 5:LSP 集成

**目标**:实现 LSP 客户端,支持代码导航和诊断

**主要任务**:
1. 实现 LSP 客户端:
   - 基于 lsp-types crate 实现 LSP 协议
   - 支持 LSP 服务器自动启动(按文件类型)
   - 常见语言服务器配置(TypeScript/Python/Rust/Go/Java)
2. 实现 LSP 工具(实验性,参照 OpenCode `OPENCODE_EXPERIMENTAL_LSP_TOOL`):
   - 单一 `lsp` 工具,通过 `operation` 参数路由 8 种操作:
     - `definition`:跳转到定义
     - `references`:查找引用
     - `hover`:悬停信息
     - `diagnostics`:获取诊断信息(错误、警告)
     - `document_symbol`:文档符号列表
     - `workspace_symbol`:工作区符号搜索
     - `implementation`:跳转到实现
     - `call_hierarchy`:调用层次
3. 集成到 edit 工具:
   - 编辑文件后自动触发 LSP 诊断
   - 将诊断错误反馈给 LLM
4. LSP 服务器管理:
   - 按工作区启动/停止 LSP 服务器
   - 服务器健康检查和自动重启
5. 前端 LSP 状态展示(可选):
   - 显示当前激活的 LSP 服务器
   - 诊断信息可视化

**交付物**:具备代码导航和诊断能力的完整编程 Agent

**详细文档**:[2026-07-08-coding-agent-refactor-phase5-lsp.md](./2026-07-08-coding-agent-refactor-phase5-lsp.md)

---

### 3.3 阶段依赖关系

```
阶段 1 (核心架构与工具链)
   │
   ├──> 阶段 2 (权限系统与 Agent 模式)
   │       │
   │       └──> 阶段 3 (Skill 系统与上下文管理)
   │               │
   │               └──> 阶段 4 (子 Agent 与高级工具)
   │                       │
   │                       └──> 阶段 5 (LSP 集成)
   │
   └──> [可并行] 阶段 3 的 Skill 系统部分(不依赖阶段 2)
```

**依赖说明**:
- 阶段 1 是所有后续阶段的基础,必须先完成
- 阶段 2 的权限系统是阶段 3/4/5 的前提(新工具需要权限控制)
- 阶段 3 的上下文压缩是阶段 4 子 Agent 的前提(子 Agent 需要独立上下文)
- 阶段 4 的子 Agent 是阶段 5 LSP 的前提(LSP 工具需要权限和上下文管理)
- 部分阶段可并行:阶段 3 的 Skill 系统不依赖阶段 2,可并行开发

---

## 四、关键技术决策

### 4.1 保留的 WorkMolde AI 设计

以下 WorkMolde AI 现有设计经过验证,将在改造中保留:

1. **LlmRouter 多 Provider 适配**:OpenAI/Anthropic/Gemini/Ollama 适配器 + Fallback + 健康检查
2. **流式事件机制**:agent:thinking/content/tool_call/tool_result/done/error 事件流
3. **确认通道**:oneshot::channel 同步等待用户确认(升级为权限系统)
4. **增量持久化**:每轮迭代后持久化消息,防止崩溃丢失
5. **Scratchpad 草稿本**:按 session_id 隔离的笔记系统(整合进 TodoWrite)
6. **版本快照**:文件修改前自动创建快照(保留,但调整为 edit 工具触发)
7. **缓存优化**:工具定义按字母序排序、工具结果截断、reasoning_content 压缩
8. **截断重试**:max_tokens 不足时翻倍重试
9. **网络监控**:断网暂停 + 恢复重试
10. **Tauri 无边框窗口**:保持现有 UI 框架

### 4.2 新增的技术选型

| 模块 | 技术选型 | 说明 |
|------|----------|------|
| **glob 工具** | `ignore` crate(ripgrep 封装) | 高性能文件模式匹配,支持 .gitignore |
| **grep 工具** | `ignore` crate(ripgrep 封装) + `regex` | 高性能正则搜索,支持 .gitignore |
| **edit 工具** | 自实现 + `similar` crate | 精确字符串替换 + 差异计算 |
| **apply_patch 工具** | 自实现 | 应用补丁文件修改代码(edit 权限类别) |
| **LSP 客户端** | `lsp-types` + `tokio` | LSP 协议实现(单一 lsp 工具,operation 路由) |
| **tree-sitter** | `tree-sitter` crate | 代码语法分析(用于 sourcecode 工具) |
| **网页抓取** | `reqwest` + `scraper` | HTTP 请求 + HTML 解析 |
| **网络搜索** | MCP 协议(Exa AI,JSON-RPC 2.0) | websearch 工具实现 |
| **权限规则存储** | SQLite `permission_rules` 表 | 持久化权限规则 |
| **Skill 加载** | `serde_yaml` + `walkdir` | frontmatter 解析 + 目录扫描 |

### 4.3 保留的依赖(原计划移除,现保留)

> v1.1 修订:文档 Handler 保留,以下依赖项不再移除。

| 保留项 | 原因 |
|--------|------|
| Python Sidecar (sidecar/) | Document 模式下提供文档处理能力 |
| python-docx/openpyxl/python-pptx/PyMuPDF 等 | 文档处理库(Document 模式依赖) |
| `scripts/sync_sidecar_dev.ps1` | Sidecar 开发同步脚本 |
| `scripts/build_sidecar.ps1` | Sidecar 构建脚本(打包时使用) |
| `tauri.conf.json` 中的 sidecar 配置 | 打包 Sidecar |
| `package.json` 中的 sidecar:build 脚本 | 构建 Sidecar |

### 4.4 统一接口定义(跨阶段共用)

> v1.3 新增:为避免各阶段对同一接口的修改互相冲突,本节定义跨阶段共用的统一接口签名。各阶段文档在修改这些接口时,必须以本节为准。

#### 4.4.1 `register_builtin_tools` 统一签名

`register_builtin_tools` 函数随阶段推进逐步增加参数。为避免各阶段签名冲突,采用"渐进式签名扩展"策略:每阶段只增加该阶段引入的参数,不提前引入后续阶段的参数类型(避免类型不存在的编译错误)。最终签名(Phase 5 完成后)如下所示。

**最终签名(Phase 5 完成后)**:
```rust
pub fn register_builtin_tools(
    registry: &mut ToolRegistry,
    git_bash_path: String,
    // Phase 2 新增:Agent 模式管理器(用于模式相关工具的行为控制)
    agent_mode_manager: Arc<AgentModeManager>,
    // Phase 3 新增:数据库连接(用于 TodoWriteTool 持久化任务列表)
    db: Arc<Database>,
    // Phase 4 新增:子 Agent 执行器和网络搜索配置
    sub_executor: Arc<SubAgentExecutor>,
    web_search_config: WebSearchConfig,
    // Phase 5 新增:LSP 相关组件
    lsp_manager: Arc<LspServerManager>,
    lsp_router: Arc<LanguageRouter>,
    lsp_cache: Arc<LspResultCache>,
) -> SharedScratchpadStates
```

**各阶段调用方式**:

各阶段采用"渐进式签名扩展"原则,每阶段只增加该阶段引入的参数,不提前引入后续阶段的参数类型(避免类型不存在的编译错误):

- **Phase 1**:签名保持原始形态 `(registry, git_bash_path) -> SharedScratchpadStates`
- **Phase 2**:签名扩展为 `(registry, git_bash_path, agent_mode_manager) -> SharedScratchpadStates`
- **Phase 3**:签名扩展为 `(registry, git_bash_path, agent_mode_manager, db) -> SharedScratchpadStates`
- **Phase 4**:签名扩展为 `(registry, git_bash_path, agent_mode_manager, db, sub_executor, web_search_config) -> SharedScratchpadStates`
- **Phase 5**:签名扩展为最终形态(上述完整签名)

**注意**:
- `workspace_root` 不作为 `register_builtin_tools` 的参数(工作区路径由 executor 在运行时注入,不在工具注册时传递)
- 返回值 `SharedScratchpadStates` 必须被调用方接收,用于与 `ScratchpadTool` 共享状态
- 各阶段实施时只需修改签名为该阶段的形态,不需要提前引入后续阶段的参数

#### 4.4.2 `build_system_prompt_with_task` 统一签名

**最终签名(Phase 2 完成后)**:
```rust
pub fn build_system_prompt_with_task(
    workspace_path: &str,
    task_type: &TaskType,
    tool_count: usize,
    handler_count: usize,
    token_budget: &TokenBudgetManager,
    author_info: Option<&AuthorInfo>,       // 保留(传 None 即可,Document 模式下使用)
    env_info: &EnvironmentInfo,
    agents_md_content: Option<&str>,        // Phase 1 新增:AGENTS.md 内容
    agent_mode: &AgentMode,                 // Phase 2 新增:Agent 模式
) -> String
```

**各阶段实施要求**:
- **Phase 1 T1.06**:实现基础签名 + `agents_md_content` 参数,`agent_mode` 参数暂用默认值 `AgentMode::Build`
- **Phase 2 T2.14**:增加 `agent_mode` 参数,必须保留 `author_info` 参数(传 `None`),不得移除
- **Phase 3 及以后**:不再修改此签名,仅修改方法内部的 prompt 组装逻辑

#### 4.4.3 `layer_context` 统一签名

**最终签名(保持与现有代码一致)**:
```rust
fn layer_context(
    workspace_path: &str,
    tool_count: usize,
    handler_count: usize,
    author_info: Option<&AuthorInfo>,  // 保留参数,编程 Agent 场景传 None
    env_info: &EnvironmentInfo,
) -> String
```

**要求**:所有阶段调用 `layer_context` 时必须传递完整的 5 个参数,`author_info` 在非 Document 模式下传 `None`。

#### 4.4.4 `build_system_prompt` 方法形态

**Phase 3 重构说明**:
- Phase 1/2:`build_system_prompt` 为 `AgentContext` 的关联函数(静态方法),签名 `pub fn build_system_prompt(workspace_path: &str) -> String`
- Phase 3:改为实例方法 `pub fn build_system_prompt(&self, session_id: &str, mode: &str) -> String`,需要给 `AgentContext` 添加 `db: Option<Arc<Database>>` 字段以读取 TodoList
- Phase 3 实施时必须在 `AgentContext` 结构体中新增 `db` 字段,并在 `new` 方法中初始化

---

## 五、风险与应对

### 5.1 技术风险

| 风险 | 影响 | 应对措施 |
|------|------|----------|
| **LSP 集成复杂度高** | 阶段 5 可能延期 | LSP 作为最后阶段,不影响核心功能;可降级为仅支持 TypeScript/Python |
| **子 Agent 递归调用导致栈溢出** | 阶段 4 稳定性问题 | 限制子 Agent 最大嵌套深度(默认 3 层);子 Agent 独立 tokio task |
| **权限规则过多影响性能** | 阶段 2 性能问题 | 权限规则按 glob 模式索引;热规则缓存在内存 |
| **上下文压缩丢失关键信息** | 阶段 3 任务失败 | 压缩前生成摘要;保留最近 N 轮完整历史;压缩可回滚 |
| **edit 工具误操作覆盖文件** | 阶段 1 数据丢失 | FileTime 锁断言;编辑前自动创建版本快照;oldString 必须唯一匹配 |
| **Document 模式下工具列表动态过滤失效** | 阶段 2 模式隔离失效 | 单元测试覆盖各模式下的工具列表;executor 构建工具定义后断言 Handler 存在/不存在 |
| **Sidecar 在非 Document 模式下仍被启动** | 资源浪费 | Sidecar 保持现有懒启动机制;仅 Document 模式下触发文档处理请求时启动 |

### 5.2 工程风险

| 风险 | 影响 | 应对措施 |
|------|------|----------|
| **改造范围过大导致周期失控** | 整体延期 | 严格分阶段交付;每阶段独立可测试;允许阶段性降级 |
| **前端 UI 调整影响用户体验** | 用户抵触 | 遵循"不改变 UI 设计"原则;新功能以非侵入方式增加 |
| **数据库迁移导致数据丢失** | 用户数据丢失 | 提供 migration 脚本;旧数据兼容;备份机制 |
| **测试覆盖不足引入隐性 bug** | 质量问题 | 每阶段配套单元测试 + 集成测试;关键路径 E2E 测试 |

---

## 六、测试策略

### 6.1 测试分层

```
测试金字塔
├── E2E 测试(少量)
│   ├── 端到端 Agent 任务执行(创建文件、编辑代码、运行测试)
│   └── 权限审批流程
│
├── 集成测试(适量)
│   ├── 工具链集成测试(edit + read + grep 组合)
│   ├── 权限系统集成测试(规则匹配 + 持久化)
│   ├── LSP 集成测试(启动服务器 + 跳转定义)
│   └── 子 Agent 集成测试(嵌套调用 + 结果汇总)
│
└── 单元测试(大量)
    ├── 每个工具的 execute 方法
    ├── 权限规则匹配逻辑
    ├── AGENTS.md 解析
    ├── SKILL.md 解析
    ├── 系统提示词构建
    └── 上下文压缩算法
```

### 6.2 关键测试用例

每个阶段的详细文档中会列出具体的测试用例,以下为关键场景:

1. **edit 工具**:oldString 唯一匹配、多匹配报错、FileTime 锁冲突、编辑后自动格式化
2. **glob 工具`**:`**/*.ts` 递归匹配、`{a,b}/*.rs` 花括号扩展、排除模式
3. **grep 工具**:正则匹配、多文件搜索、上下文行显示、二进制文件跳过
4. **权限系统**:allow 规则自动放行、deny 规则立即拒绝、ask 规则触发对话框
5. **Plan/Build/Document 模式切换**:Plan 模式下 edit 被拒绝;Build 模式下文档 Handler 不在工具列表;Document 模式下文档 Handler 出现且可调用
6. **Skill 加载**:SKILL.md frontmatter 解析、按权限过滤、按需加载内容
7. **子 Agent**:task 工具委托、独立上下文、结果汇总、嵌套深度限制
8. **LSP**:服务器自动启动、跳转定义、诊断反馈

### 6.3 验证标准

每个阶段交付前必须满足:
- 所有单元测试通过(`cargo test`)
- 所有集成测试通过
- 无编译警告(`cargo clippy`)
- 代码格式规范(`cargo fmt --check`)
- 手动 E2E 测试通过(至少 3 个典型场景)

---

## 七、文档索引

| 阶段 | 文档 | 状态 |
|------|------|------|
| 总纲 | [本文档](./2026-07-08-coding-agent-refactor-overview.md) | 已完成 |
| 阶段 1 | [核心架构与工具链基础](./2026-07-08-coding-agent-refactor-phase1-core.md) | 已完成 |
| 阶段 2 | [权限系统与 Agent 模式](./2026-07-08-coding-agent-refactor-phase2-permission.md) | 已完成 |
| 阶段 3 | [Skill 系统与上下文管理](./2026-07-08-coding-agent-refactor-phase3-skill-context.md) | 已完成 |
| 阶段 4 | [子 Agent 与高级工具](./2026-07-08-coding-agent-refactor-phase4-subagent-tools.md) | 已完成 |
| 阶段 5 | [LSP 集成](./2026-07-08-coding-agent-refactor-phase5-lsp.md) | 已完成 |

---

## 八、参考资源

### 8.1 OpenCode 相关

- **GitHub 仓库**:https://github.com/sst/opencode (branch 2.0)
- **官方文档**:https://opencode.ai/docs
- **中文文档**:https://opencode.doczh.com/docs
- **源码解析**:packages/opencode/src/{agent,tool,skill,session,permission}/

### 8.2 WorkMolde AI 现有文档

- [技术架构](../tech_architecture.md)
- [Tauri 命令规范](../tauri_commands.md)
- [Handler 开发指南](../handler_development.md)
- [数据库设计](../database_design.md)
- [组件设计](../component_design.md)
- [上下文窗口设计](./2026-05-28-context-window-design.md)
- [LLM 缓存优化](./2026-06-14-llm-cache-optimization-design.md)

### 8.3 技术规范参考

- **Anthropic Effective Context Engineering**:Structured Note-taking 模式(Scratchpad 设计依据)
- **Claude Code 实践**:重试策略、流式看门狗、提示词缓存
- **LSP 协议规范**:https://microsoft.github.io/language-server-protocol/

---

## 九、改造里程碑

| 里程碑 | 内容 | 验收标准 |
|--------|------|----------|
| M1: 核心能力 | 阶段 1 完成 | Agent 能读写、编辑、搜索代码文件,执行命令 |
| M2: 权限与模式 | 阶段 2 完成 | Plan/Build 模式切换可用,权限审批正常 |
| M3: Skill 与压缩 | 阶段 3 完成 | Skill 加载正常,长对话不爆上下文 |
| M4: 子 Agent | 阶段 4 完成 | task 工具委托正常,网页抓取可用 |
| M5: LSP 集成 | 阶段 5 完成 | 跳转定义、查找引用、诊断反馈可用 |
| **最终验收** | 全部完成 | WorkMolde AI 可作为通用编程 Agent 使用,通过 E2E 测试 |

---

## 十、附录:WorkMolde AI 现有架构关键信息

### 10.1 AppState 定义

```rust
pub struct AppState {
    pub db: Arc<Database>,                            // [保留]
    pub config: Arc<Mutex<ConfigManager>>,            // [保留]
    pub active_agents: Arc<Mutex<HashMap<String, bool>>>, // [保留]
    pub confirm_channels: Arc<Mutex<HashMap<String, oneshot::Sender<ConfirmDecision>>>>, // [保留] 阶段2升级为权限系统
    pub doc_service: Arc<DocumentService>,           // [保留] v1.1: Document 模式下使用
    pub llm_router: Arc<RwLock<Arc<LlmRouter>>>,     // [保留]
    pub tool_registry: Arc<ToolRegistry>,            // [保留]
    pub handler_registry: Arc<Mutex<HandlerRegistry>>, // [保留] v1.1: Document 模式下使用(阶段1保留)
    pub fs_watcher: Arc<FsWatcherService>,           // [保留]
    pub network_monitor: Arc<NetworkMonitor>,        // [保留]
    pub scratchpad_states: SharedScratchpadStates,   // [保留] 阶段3整合进 TodoWrite
    // [新增] 阶段2: permission_registry: Arc<PermissionRegistry>
    // [新增] 阶段3: skill_service: Arc<SkillService>
    // [新增] 阶段5: lsp_manager: Arc<LspManager>
}
```

### 10.2 目标工具清单(对齐 OpenCode 13 个核心工具)

| 工具名 | 权限类别 | 用途 |
|--------|---------|------|
| bash | bash | 执行 shell 命令 |
| edit | edit | 精确字符串替换修改文件 |
| write | edit | 创建新文件或覆盖现有文件 |
| read | read | 读取文件内容(支持行号范围 start_line/end_line) |
| grep | grep | 基于 ignore crate 的正则搜索(支持 .gitignore) |
| glob | glob | 基于 ignore crate 的文件模式匹配(支持 .gitignore) |
| lsp | lsp | LSP 代码智能(单一工具,operation 参数路由 8 种操作,实验性) |
| apply_patch | edit | 应用补丁文件修改代码 |
| skill | skill | 加载 Skill |
| todowrite | todowrite | 管理待办列表 |
| webfetch | webfetch | 获取网页内容 |
| websearch | websearch | 网络搜索(MCP 协议,Exa AI,JSON-RPC 2.0) |
| question | question | 向用户提问(收集偏好或澄清需求) |

> **WorkMolde AI 扩展工具**(不在 OpenCode 13 个核心工具内,WorkMolde AI 特色保留):
> - `write_script`:将智能体生成的脚本写入系统临时目录(WorkMolde AI 特色)
> - `task`:子 Agent 委托工具(阶段4 实现)
> - `sourcecode`:代码语义搜索(阶段3 实现,基于 tree-sitter)
> - 文档 Handler(`docx`/`xlsx`/`pptx`/`pdf`):仅 Document 模式下动态启用

> **命名规则说明**：
> - WorkMolde AI 原有工具：复合词保留下划线（如 `file_info`/`read_lines`/`remove_dir`/`write_script`），单词无下划线（如 `list`/`read`/`bash`）
> - 从 OpenCode 引入的工具：沿用 OpenCode 原名，不适用下划线规则（如 `todowrite`/`webfetch`/`websearch`/`apply_patch`/`question`），因为这些工具名在 OpenCode 生态中已约定俗成。原 docx_handler 改名为 docx ，其他 handler 同理。

### 工具重命名映射表

以下工具在改造过程中进行重命名、合并或新增（当前代码使用旧名，改造后使用新名）：

**A. WorkMolde AI 原有工具重命名**

| 当前代码名(旧) | 目标新名 | 类型 | 说明 |
|---------------|---------|------|------|
| list_directory | list | 重命名 | 单词化 |
| search_files | search | 重命名 | 单词化 |
| read_file | read | 重命名 | 单词化（注：当前代码已是 read，无需改） |
| file_exists | exists | 重命名 | 单词化 |
| delete_file | remove | 重命名 | 单词化 |
| create_directory | mkdir | 重命名 | 单词化 |
| write_text_file | write | 重命名 | 单词化 |
| rename_file | rename | 重命名 | 单词化 |
| copy_file | copy | 重命名 | 单词化 |
| delete_directory | remove_dir | 重命名 | 复合词保留下划线 |
| get_file_hash | hash | 重命名 | 单词化 |
| update_notes | scratchpad | 重命名 | 统一为 scratchpad |
| run_command | bash | 重命名 | 单词化（增强权限） |

**B. 工具合并**

| 当前代码名(旧) | 目标新名 | 类型 | 说明 |
|---------------|---------|------|------|
| read_file_lines | read | 合并 | 合并到 read,通过 start_line/end_line 参数实现行号范围读取 |
| lsp_definition / lsp_references / lsp_diagnostics / lsp_hover | lsp | 合并 | 合并为单一 lsp 工具,通过 operation 参数路由 8 种操作(实验性) |

**C. 新增工具(OpenCode 引入,沿用原名)**

| 工具名 | 权限类别 | 类型 | 说明 |
|--------|---------|------|------|
| edit | edit | 新增 | 基于 oldString/newString 的精确文件编辑(带 FileTime 锁) |
| glob | glob | 新增 | 基于 ignore crate 的文件模式匹配(支持 .gitignore) |
| grep | grep | 新增 | 基于 ignore crate 的正则搜索(支持 .gitignore) |
| apply_patch | edit | 新增 | 应用补丁文件修改代码 |
| skill | skill | 新增 | 加载 Skill |
| todowrite | todowrite | 新增 | 管理待办列表(替代 Scratchpad) |
| webfetch | webfetch | 新增 | 获取网页内容 |
| websearch | websearch | 新增 | 网络搜索(MCP 协议,Exa AI) |
| question | question | 新增 | 向用户提问(独立权限类别) |
| task | task | 新增 | 子 Agent 委托(阶段4) |

> **不变的工具**：`read`、`file_info`、`write_script`、`bash`（注：bash 是 run_command 的新名）、`scratchpad`（是 update_notes 的新名,阶段3整合进 TodoWrite）

### 10.3 现有 Handler 清单(4 个,保留并按 Document 模式启用)

| Handler 名 | 功能 | 改造处置 |
|------------|------|----------|
| docx | Word 文档处理 | **保留**,Document 模式下动态加入工具列表 |
| xlsx | Excel 文档处理 | **保留**,Document 模式下动态加入工具列表 |
| pptx | PPT 文档处理 | **保留**,Document 模式下动态加入工具列表 |
| pdf | PDF 文档处理 | **保留**,Document 模式下动态加入工具列表 |

### 10.4 现有命令清单(40+)

详见 [tauri_commands.md](../tauri_commands.md),改造中需调整的命令:
- `list_handlers`:**保留**(Document 模式下用于查看可用文档 Handler)
- `list_tools`:保留,返回值按当前 Agent 模式过滤
- `start_agent`:增加 mode 参数(阶段2,支持 plan/build/document)
- `confirm_operation`:升级为权限审批(阶段2)
- 新增命令:`list_skills`, `load_skill`, `lsp` 等
