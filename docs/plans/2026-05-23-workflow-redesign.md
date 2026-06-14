# 工作流区域全面重新设计 - 开发计划

> **注意**: 本文档中提到的 "Skill" 已重命名为 "Handler"，相关工具名如 `docx_skill` 已更改为 `docx_handler`。

**目标**: 将工作流区域从卡片式布局重新设计为扁平化自然布局，支持大模型深度思考链展示，移除冗余节点类型，打造类似 Claude Code / Codex CLI 的沉浸式交互体验。

**架构方向**: 前端组件重构 + 后端事件扩展（支持深度思考链），保持数据流不变，仅改变展示层和事件粒度。

**技术栈**: React 19 + TypeScript 5 + Tailwind CSS 4 + Zustand 5（前端）；Rust + Tokio + Tauri 事件系统（后端）

---

## 一、设计参考与需求分析

### 1.1 参考应用分析

| 应用 | 思考链展示 | 工具调用展示 | 整体风格 |
|------|-----------|-------------|---------|
| **Claude Code** | Extended Thinking 以可折叠块展示，标签显示 "Thought for Xs"，内容为浅色斜体 | 工具调用以单行显示工具名，不展示完整 JSON | 终端风格，扁平化，无卡片 |
| **OpenAI Codex CLI** | 思维摘要（thinking summary）以简短文字展示 | 工具调用以命令行风格展示 | 终端风格，极简 |
| **Claude.ai Web** | 思考块可折叠，标签 "Thinking..."，内容为小字浅色 | 工具调用以徽章+名称展示 | 卡片式但思考部分扁平 |

### 1.2 当前实现问题

1. **卡片式布局过重**: 所有节点都使用 `.wf-node-card` 卡片样式（边框+背景+圆角），视觉噪音大
2. **硬编码思考文字**: `agent:thinking` 事件发射的是 "正在分析用户请求并规划操作步骤(第x轮)"，不是 LLM 的真实思考
3. **冗余节点类型**: `result`（执行成功）和 `reply`（回复）节点增加了视觉复杂度但信息价值低
4. **工具调用展示冗长**: JSON.stringify 展示完整参数，可读性差
5. **不支持深度思考链**: 后端未解析 LLM 的 `reasoning_content` / `thinking` 块，前端无法区分深度思考和普通输出

### 1.3 核心需求确认

| 需求 | 决策 |
|------|------|
| 用户输入 | 保留卡片样式 |
| 其他所有内容 | 去掉卡片，平铺在页面底色上 |
| 执行成功(result)节点 | 完全移除 |
| 回复(reply)节点 | 移除，最后一段内容即为回复 |
| 标签文字 | 移除"用户指令、思考中、执行成功、工具调用、回复"等标签 |
| 工具调用展示 | 仅显示工具名+简要描述，不显示 JSON |
| 确认节点 | 去掉卡片，平铺显示，保留按钮 |
| 错误节点 | 去掉卡片，用红色标记突出 |
| 深度思考链 | 可折叠，标签 "Thinking..."，内容为浅色小字斜体 |
| 左侧图标 | 保留，每个图标对应右侧工作流内容 |
| 工作流间距 | 每个工作流之间有一小块空白区域 |

---

## 二、新设计方案

### 2.1 新的节点类型体系

**当前类型**: `user | thinking | tool | result | reply | confirm | error`

**新类型**: `user | thinking | content | tool | confirm | error`

| 类型 | 用途 | 展示方式 | 左侧图标 |
|------|------|---------|----------|
| `user` | 用户输入 | 卡片样式（保留） | user 图标，accent 色 |
| `thinking` | 深度思考链（Extended Thinking） | 可折叠 "Thinking..." 块，浅色小字斜体 | thinking 图标，purple 色 |
| `content` | LLM 正常文本输出 | 直接平铺，正常字号 | 无图标（或小型圆点） |
| `tool` | 工具调用 | 工具名+简要描述，单行 | tool 图标，secondary 色 |
| `confirm` | 操作确认 | 平铺，保留确认/取消按钮 | warning 图标，warning 色 |
| `error` | 错误信息 | 平铺，红色左边框标记 | error 图标，error 色 |

**移除的类型**:
- `result`: 工具执行结果不再单独显示，成功时静默，失败时更新 tool 节点状态
- `reply`: 最后一段 `content` 节点即为回复

### 2.2 页面布局示意

```
┌─────────────────────────────────────────────────┐
│  ●  ┌──────────────────────────────────────┐    │  ← 用户输入（卡片）
│  │  │ 帮我生成一份季度销售报告              │    │
│  │  └──────────────────────────────────────┘    │
│  │                                              │
│  ●  ▼ Thinking...                              │  ← 深度思考（可折叠）
│  │    用户需要一份季度销售报告，我需要先       │
│  │    确定报告的格式和内容结构...              │    ← 浅色小字斜体
│  │                                              │
│  ●  我来帮你生成季度销售报告。首先需要         │  ← 正常文本输出（平铺）
│  │  确认一些细节：报告的格式是 Word 还是       │
│  │  PDF？需要包含哪些数据维度？                │
│  │                                              │
│  ●  ▼ Thinking...                              │  ← 第二轮深度思考
│  │    用户指定了 Word 格式，我需要调用          │
│  │    generate_document 工具...                │
│  │                                              │
│  ◆  generate_document · 生成 季度销售报告.docx │  ← 工具调用（单行）
│  │                                              │
│  ●  报告已生成完毕！文件保存在工作区的          │  ← 最终回复（平铺）
│     季度销售报告.docx 中，你可以打开预览。     │
│                                                │
└─────────────────────────────────────────────────┘

图例: ● = 圆形图标   ◆ = 菱形/方形图标   │ = 时间线竖线
```

### 2.3 深度思考折叠组件设计

```
折叠状态:
  ●  ▶ Thinking...                          ← 点击可展开

展开状态:
  ●  ▼ Thinking...                          ← 点击可折叠
       用户需要一份季度销售报告，我需要先
       确定报告的格式和内容结构。根据常见的
       季度报告模板，应该包含...
                                         ← 浅色(text-tertiary)、小字(12px)、斜体
```

- 折叠标签文字: "Thinking..."
- 折叠/展开图标: chevron-right / chevron-down，小尺寸
- 内容样式: `font-size: 12px; font-style: italic; color: var(--color-text-tertiary); line-height: 1.7;`
- 默认状态: 折叠（思考完成后自动折叠，思考中时展开）
- 流式输出时: 展开状态，带闪烁光标

### 2.4 工具调用展示设计

```
当前（要移除）:
  ┌─────────────────────────────┐
  │ 工具调用          12:30:45 ▼│
  │ ┌─────────────────────────┐ │
  │ │ Handler  generate_document│ │
  │ │ {                       │ │
  │ │   "file_name": "报告",  │ │
  │ │   "format": "docx",     │ │
  │ │   ...                   │ │
  │ │ }                       │ │
  │ └─────────────────────────┘ │
  └─────────────────────────────┘

新设计:
  ◆  generate_document · 生成 报告.docx
```

- 工具名: 等宽字体，`font-family: var(--font-mono); font-size: 13px;`
- 分隔符: ` · `（中点）
- 简要描述: 从参数中提取关键信息（文件名、操作类型等）
- 执行状态: 成功时无额外标记；失败时文字变红，追加错误信息

### 2.5 确认节点设计

```
  ⚠  操作确认
     即将删除文件 报告.docx，此操作不可撤销。
     [确认执行]  [取消操作]
```

- 无卡片边框和背景
- 标题行: warning 色，加粗
- 描述: 正常文字，secondary 色
- 按钮: 保持现有样式

### 2.6 错误节点设计

```
  ✕  文件不存在: 报告.docx
     错误码: E3001 · 模块: document
     [重试]
```

- 无卡片边框
- 左侧红色竖线标记: `border-left: 3px solid var(--color-error);`
- 错误消息: error 色
- 错误详情: 可折叠，tertiary 色
- 重试按钮: 保持现有样式

---

## 三、后端改造

### 3.1 LLM 模型扩展 - 支持深度思考链

**文件**: `src-tauri/src/models/llm.rs`

当前 `ChatMessage` 结构体缺少 `reasoning_content` 字段。需要扩展以支持深度思考模型的输出。

```rust
// 新增字段
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub tool_calls: Option<Vec<LlmToolCall>>,
    pub tool_call_id: Option<String>,
    // 新增: 深度思考链内容（Claude extended thinking / DeepSeek reasoning_content）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
}
```

**流式响应类型扩展**:

```rust
// 流式 delta 中新增 reasoning_content 字段
pub struct StreamDelta {
    pub content: Option<String>,
    pub reasoning_content: Option<String>,  // 新增
    pub tool_calls: Option<Vec<ToolCallDelta>>,
}
```

### 3.2 LLM 适配器改造

**涉及文件**:
- `src-tauri/src/services/llm/openai_adapter.rs`
- `src-tauri/src/services/llm/anthropic_adapter.rs`
- `src-tauri/src/services/llm/gemini_adapter.rs`

**改造要点**:

1. **OpenAI 兼容适配器**: 解析 `reasoning_content` 字段（DeepSeek 等兼容模型使用）
2. **Anthropic 适配器**: 解析 `thinking` 块（Claude extended thinking），将其映射为 `reasoning_content`
3. **Gemini 适配器**: 解析 `thought` 块（Gemini thinking），将其映射为 `reasoning_content`

**流式输出改造**: 在流式回调中，当检测到 `reasoning_content` 增量时，发射 `agent:deep_thinking` 事件而非 `agent:content` 事件。

### 3.3 事件类型扩展

**文件**: `src-tauri/src/events/types.rs`

新增深度思考事件:

```rust
pub const AGENT_DEEP_THINKING: &str = "agent:deep_thinking";

/// 深度思考链增量
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeepThinkingPayload {
    pub session_id: String,
    pub step: u32,
    pub thought: String,
    /// 是否为流式输出的中间片段
    pub is_streaming: bool,
}
```

### 3.4 Agent Executor 改造

**文件**: `src-tauri/src/services/agent/executor.rs`

**改造要点**:

1. 移除硬编码的 "正在分析用户请求并规划操作步骤(第x轮)" 思考事件发射
2. 在流式回调中，当收到 `reasoning_content` 时发射 `agent:deep_thinking` 事件
3. 当收到普通 `content` 时发射 `agent:content` 事件（保持不变）
4. 工具调用结果不再发射 `agent:tool_result` 事件中的成功结果（仅失败时发射错误）

### 3.5 数据库消息模型扩展

**文件**: `src-tauri/src/db/` 相关文件

消息表需要新增 `reasoning_content` 列，用于持久化深度思考链内容:

```sql
ALTER TABLE messages ADD COLUMN reasoning_content TEXT;
```

---

## 四、前端改造

### 4.1 类型定义更新

**文件**: `src/types/workflow.ts`

```typescript
// 节点类型精简
export type WorkflowNodeType = "user" | "thinking" | "content" | "tool" | "confirm" | "error";

// 思考节点数据 - 现在承载真实深度思考内容
export interface ThinkingNodeData {
  content: string;       // 深度思考链内容
  duration: number;      // 思考耗时（秒）
  isStreaming?: boolean;  // 是否正在流式输出
}

// 新增: 内容节点数据（替代原 reply 节点）
export interface ContentNodeData {
  content: string;
  isStreaming?: boolean;
}

// 工具节点数据 - 简化
export interface ToolNodeData {
  toolName: string;
  toolBadge?: string;
  briefDescription: string;  // 新增: 简要描述
  input: Record<string, unknown>;  // 保留原始参数用于详情展开
  success?: boolean;         // 新增: 执行结果状态
  error?: string;            // 新增: 执行错误信息
}

// 移除: ResultNodeData, ReplyNodeData

// NodeDataMap 更新
export interface NodeDataMap {
  user: UserNodeData;
  thinking: ThinkingNodeData;
  content: ContentNodeData;
  tool: ToolNodeData;
  confirm: ConfirmNodeData;
  error: ErrorNodeData;
}
```

### 4.2 事件类型更新

**文件**: `src/services/event.ts`

```typescript
// 新增: 深度思考链增量事件
export interface DeepThinkingPayload {
  sessionId: string;
  step: number;
  thought: string;
  isStreaming: boolean;
}

// 新增监听函数
export function onAgentDeepThinking(
  handler: (payload: DeepThinkingPayload) => void,
): Promise<UnlistenFn> {
  return listen<DeepThinkingPayload>("agent:deep_thinking", (event) => {
    handler(event.payload);
  });
}
```

### 4.3 组件重构

#### 4.3.1 WorkflowNode.tsx - 节点路由更新

移除 `ResultNode` 和 `ReplyNode` 的引用，新增 `ContentNode`:

```typescript
// 移除: ResultNode, ReplyNode
// 新增: ContentNode
import { ContentNode } from "./ContentNode";

switch (nt) {
  case "user": return <UserNode ... />;
  case "thinking": return <ThinkingNode ... />;
  case "content": return <ContentNode ... />;
  case "tool": return <ToolNode ... />;
  case "confirm": return <ConfirmNode ... />;
  case "error": return <ErrorNode ... />;
  default: return null;
}
```

#### 4.3.2 UserNode.tsx - 简化

- 保留卡片样式
- 移除 "用户指令" 标签
- 移除时间戳和折叠按钮
- 仅显示用户输入文本

#### 4.3.3 ThinkingNode.tsx - 重新设计

- 移除卡片样式，改为扁平布局
- 移除 "思考中" 标签
- 改为可折叠 "Thinking..." 标签
- 内容使用浅色小字斜体
- 流式输出时自动展开，完成后自动折叠
- 保留左侧 thinking 图标

#### 4.3.4 ContentNode.tsx - 新建

- 扁平布局，无卡片
- 正常字号显示 LLM 文本输出
- 支持简单 Markdown 渲染（加粗、代码、链接）
- 流式输出时带闪烁光标
- 无左侧图标（或使用小型圆点）

#### 4.3.5 ToolNode.tsx - 简化

- 移除卡片样式，改为扁平布局
- 移除 "工具调用" 标签
- 移除 JSON 参数展示
- 仅显示: 工具名 + ` · ` + 简要描述
- 执行失败时: 文字变红，追加错误信息
- 保留左侧 tool 图标

#### 4.3.6 ConfirmNode.tsx - 去卡片化

- 移除卡片样式，改为扁平布局
- 保留确认/取消按钮
- 保留 warning 色标记

#### 4.3.7 ErrorNode.tsx - 去卡片化

- 移除卡片样式，改为扁平布局
- 添加左侧红色竖线标记
- 保留重试按钮
- 保留错误详情折叠

#### 4.3.8 删除的组件

- `ResultNode.tsx` - 删除
- `ReplyNode.tsx` - 删除

### 4.4 CSS 样式改造

**文件**: `src/styles/globals.css`

**移除的样式**:
- `.wf-node-card` 及相关卡片样式
- `.wf-node-header` 卡片头部样式
- `.wf-node-label` 标签样式
- `.wf-node-toggle` 折叠按钮样式
- `.wf-tool-args` JSON 参数展示样式
- `.wf-result-text` / `.wf-result-file` 结果节点样式
- `.wf-reply-text` 回复节点样式

**新增的样式**:

```css
/* 思考折叠块 */
.wf-thinking-toggle { ... }    /* "Thinking..." 折叠标签 */
.wf-thinking-content { ... }   /* 思考内容：浅色小字斜体 */

/* 内容输出 */
.wf-content-text { ... }       /* 正常文本输出 */

/* 工具调用简化 */
.wf-tool-brief { ... }         /* 工具名+简要描述 */
.wf-tool-error { ... }         /* 工具执行失败标记 */

/* 确认节点扁平化 */
.wf-confirm-flat { ... }       /* 扁平确认区域 */

/* 错误节点扁平化 */
.wf-error-flat { ... }         /* 扁平错误区域，红色左边框 */
```

### 4.5 Store 更新

**文件**: `src/stores/useWorkflowStore.ts`

1. 更新 `loadFromMessages` 方法:
   - 助手消息的 `reasoning_content` → `thinking` 节点
   - 助手消息的 `content` → `content` 节点
   - 助手消息的 `tool_calls` → `tool` 节点
   - 工具消息不再生成 `result` 节点

2. 新增 `appendThinkingContent` 方法: 用于流式追加深度思考内容到现有 thinking 节点

### 4.6 App.tsx 事件处理更新

**文件**: `src/App.tsx`

1. 注册 `agent:deep_thinking` 事件监听
2. 更新节点映射逻辑:
   - `deep_thinking` → `thinking` 节点（流式追加）
   - `content` → `content` 节点（流式追加）
   - `tool_call` → `tool` 节点（含简要描述生成）
   - `tool_result` → 更新对应 `tool` 节点的成功/失败状态
   - 移除 `result` 节点创建
   - 移除 `reply` 节点创建
3. 简要描述生成逻辑: 从工具参数中提取关键信息（如文件名、操作类型）

### 4.7 工具简要描述生成

新增工具函数，从工具参数中提取可读的简要描述:

```typescript
function generateToolBrief(toolName: string, input: Record<string, unknown>): string {
  switch (toolName) {
    case "generate_document":
      return `生成 ${input.file_name || "文档"}`;
    case "read_document":
      return `读取 ${input.file_name || "文档"}`;
    case "modify_document":
      return `修改 ${input.file_name || "文档"}`;
    case "delete_document":
      return `删除 ${input.file_name || "文件"}`;
    case "convert_format":
      return `转换 ${input.file_name || "文档"} 格式`;
    case "search_documents":
      return `搜索 ${input.query ? `"${input.query}"` : "文件"}`;
    case "analyze_document":
      return `分析 ${input.file_name || "文档"}`;
    case "list_workspace":
      return "列出工作区目录";
    case "batch_process":
      return `批量处理 ${input.operation || "文档"}`;
    default:
      return toolName;
  }
}
```

---

## 五、实施任务分解

### 阶段一: 后端深度思考链支持

| 任务 | 文件 | 说明 |
|------|------|------|
| 1.1 扩展 ChatMessage 模型 | `models/llm.rs` | 新增 `reasoning_content` 字段 |
| 1.2 扩展流式响应类型 | `models/llm.rs` | StreamDelta 新增 `reasoning_content` |
| 1.3 新增 DeepThinkingPayload | `events/types.rs` | 深度思考事件类型定义 |
| 1.4 新增事件发射方法 | `events/emitter.rs` | `emit_deep_thinking()` 方法 |
| 1.5 改造 OpenAI 适配器 | `llm/openai_adapter.rs` | 解析 `reasoning_content` 字段 |
| 1.6 改造 Anthropic 适配器 | `llm/anthropic_adapter.rs` | 解析 `thinking` 块 |
| 1.7 改造 Gemini 适配器 | `llm/gemini_adapter.rs` | 解析 `thought` 块 |
| 1.8 改造 Executor | `agent/executor.rs` | 移除硬编码思考，发射深度思考事件 |
| 1.9 数据库迁移 | `db/init.rs` | messages 表新增 `reasoning_content` 列 |
| 1.10 消息持久化更新 | `db/message_repo.rs` | 保存和读取 `reasoning_content` |

### 阶段二: 前端类型与事件更新

| 任务 | 文件 | 说明 |
|------|------|------|
| 2.1 更新 WorkflowNodeType | `types/workflow.ts` | 移除 result/reply，新增 content |
| 2.2 更新 NodeDataMap | `types/workflow.ts` | 新增 ContentNodeData，简化 ToolNodeData |
| 2.3 新增 DeepThinkingPayload | `services/event.ts` | 深度思考事件类型和监听函数 |
| 2.4 更新 useAgent hook | `hooks/useAgent.ts` | 新增 deepThinking 状态 |

### 阶段三: 前端组件重构

| 任务 | 文件 | 说明 |
|------|------|------|
| 3.1 重构 UserNode | `workflow/UserNode.tsx` | 移除标签，简化为纯文本卡片 |
| 3.2 重构 ThinkingNode | `workflow/ThinkingNode.tsx` | 可折叠 "Thinking..." 设计 |
| 3.3 新建 ContentNode | `workflow/ContentNode.tsx` | 扁平文本输出组件 |
| 3.4 重构 ToolNode | `workflow/ToolNode.tsx` | 简化为工具名+简要描述 |
| 3.5 重构 ConfirmNode | `workflow/ConfirmNode.tsx` | 去卡片化，扁平布局 |
| 3.6 重构 ErrorNode | `workflow/ErrorNode.tsx` | 去卡片化，红色标记 |
| 3.7 删除 ResultNode | `workflow/ResultNode.tsx` | 删除文件 |
| 3.8 删除 ReplyNode | `workflow/ReplyNode.tsx` | 删除文件 |
| 3.9 更新 WorkflowNode | `workflow/WorkflowNode.tsx` | 更新节点路由 |
| 3.10 更新 WorkflowTimeline | `workflow/WorkflowTimeline.tsx` | 调整虚拟滚动高度估算 |

### 阶段四: CSS 样式改造

| 任务 | 文件 | 说明 |
|------|------|------|
| 4.1 移除卡片相关样式 | `styles/globals.css` | 删除 .wf-node-card 等 |
| 4.2 新增扁平布局样式 | `styles/globals.css` | 思考折叠、内容输出、工具简要等 |
| 4.3 更新暗色模式样式 | `styles/globals.css` | 适配新的扁平布局 |

### 阶段五: 事件处理与 Store 更新

| 任务 | 文件 | 说明 |
|------|------|------|
| 5.1 更新 useWorkflowStore | `stores/useWorkflowStore.ts` | 更新 loadFromMessages，新增 appendThinkingContent |
| 5.2 更新 App.tsx 事件映射 | `App.tsx` | 注册深度思考事件，更新节点创建逻辑 |
| 5.3 新增工具简要描述生成 | `utils/toolBrief.ts` | 从参数提取可读描述 |

### 阶段六: 测试与验证

| 任务 | 说明 |
|------|------|
| 6.1 TypeScript 类型检查 | `npx tsc -b` 确保无类型错误 |
| 6.2 构建验证 | `npm run build` 确保构建通过 |
| 6.3 开发模式运行 | `npm run tauri:dev` 手动验证 UI |
| 6.4 Rust 编译检查 | `cargo build -p docagent_lib` |
| 6.5 Clippy 检查 | `cargo clippy` |

---

## 六、风险与注意事项

### 6.1 后端兼容性

- **非深度思考模型**: 对于不支持 extended thinking 的模型（如 GPT-4o），不会发射 `agent:deep_thinking` 事件，前端需要优雅处理（不显示思考折叠块，直接显示内容输出）
- **历史会话兼容**: 旧的消息记录中没有 `reasoning_content` 字段，`loadFromMessages` 需要兼容旧数据格式
- **数据库迁移**: 新增列需要考虑向后兼容，使用 `ALTER TABLE ADD COLUMN` 而非重建表

### 6.2 前端兼容性

- **虚拟滚动高度估算**: 扁平布局后节点高度变化，需要更新 `estimateSize` 函数
- **流式输出性能**: 深度思考链可能非常长（数千字），频繁更新 DOM 需要注意性能
- **Markdown 渲染**: ContentNode 中的文本可能包含 Markdown 格式，需要轻量级渲染

### 6.3 用户体验

- **思考折叠默认状态**: 思考中时展开（用户可看到实时推理），完成后自动折叠（减少视觉噪音）
- **工具调用失败反馈**: 移除 result 节点后，工具调用失败需要通过更新 tool 节点来反馈
- **确认节点醒目性**: 去掉卡片后，确认节点需要通过其他方式（颜色、间距）确保用户不会忽略

---

## 七、实施优先级建议

1. **先前端后后端**: 先完成前端组件重构（阶段三、四），使用现有事件数据验证 UI 效果
2. **后端深度思考支持**: 再完成后端改造（阶段一），实现深度思考链的完整支持
3. **最后集成测试**: 完成所有改造后进行端到端验证（阶段六）

建议按以下顺序执行:
1. 阶段二（前端类型更新）→ 阶段三（组件重构）→ 阶段四（CSS 改造）→ 阶段五（事件处理）→ 阶段一（后端改造）→ 阶段六（测试验证）
