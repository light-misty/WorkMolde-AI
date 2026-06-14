# DocAgent 上下文工程开发计划

> **注意**: 本文档中提到的 "Skill" 已重命名为 "Handler"，相关工具名如 `docx_skill` 已更改为 `docx_handler`。

**文档版本**: v1.1
**创建日期**: 2026-05-28
**理论基础**: 菜鸟教程《Agent 上下文工程》(https://www.runoob.com/ai-agent/agent-context-engineering.html)
**项目仓库**: d:\DeskTop\DocAgent

---

## 一、项目现状评估

### 1.1 已具备的上下文工程能力

经过对项目代码的全面研读，DocAgent 在上下文工程方面已具备以下基础能力：

#### 1.1.1 分层系统提示词架构（已实现）

项目在 [context.rs](file:///d:/DeskTop/DocAgent/src-tauri/src/services/agent/context.rs) 中实现了7层系统提示词分层架构：

| 层级 | 名称 | 文件位置 | 功能说明 |
|------|------|----------|----------|
| Layer 0 | 身份层 | `context.rs:341-361` | 定义 DocAgent 角色、专业领域、行为方式和核心立场 |
| Layer 1 | 规则层 | `context.rs:364-387` | 正面约束（7条必须遵守）+ 负面约束（8条禁止行为） |
| Layer 2 | 上下文层 | `context.rs:390-395` | 工作区路径、工具/处理器数量等运行时信息 |
| Layer 3 | 策略层 | `context.rs:398-428` | 工具选择策略指导（读取/写入/搜索/转换/分析） |
| Layer 4 | 防幻觉层 | `context.rs:431-442` | 信息诚实规则，防止编造内容 |
| Layer 5 | 错误处理层 | `context.rs:445-474` | 工具失败处理策略和确认机制说明 |
| Layer 6 | 规范层 | `context.rs:477-496` | 按任务类型注入文档设计规范（docx/xlsx/pptx/pdf） |
| Layer 7 | 示例层 | `context.rs:499-564` | 按任务类型注入 few-shot 示例 |

**评估**: 分层架构设计合理，已实现按任务类型动态注入规范层和示例层，这是"渐进式披露"思想的初步体现。

#### 1.1.2 任务类型识别与动态提示词构建（已实现）

在 [task_type.rs](file:///d:/DeskTop/DocAgent/src-tauri/src/services/agent/prompts/task_type.rs) 中实现了：
- 基于关键词的用户消息任务类型识别（12种任务类型枚举）
- 基于已调用工具的任务类型修正
- 基于 `generate_document` 的 `format` 参数的精确类型推断
- 根据任务类型决定注入哪些设计规范和示例

**评估**: 任务类型识别机制已建立，但仅基于简单关键词匹配，缺乏语义理解能力，对模糊或复杂意图的识别准确度有限。

#### 1.1.3 Token 预算管理器（已实现）

在 [token_budget.rs](file:///d:/DeskTop/DocAgent/src-tauri/src/services/agent/prompts/token_budget.rs) 中实现了：
- 基于上下文窗口大小的预算分配（默认128K）
- 预算分配比例：系统提示词15%、工具定义10%、对话历史50%、LLM响应25%
- 对话历史超预算检测
- 滑动窗口大小动态计算

**评估**: 预算框架已搭建，但存在以下不足：
- Token 估算采用 `chars().count()` 作为近似值，对中文内容严重低估（中文约1.5 token/字符）
- 未考虑不同模型上下文窗口差异，统一使用128K默认值
- 预算分配比例固定，无法根据任务类型动态调整
- 缺少实际 Token 使用量的运行时追踪

#### 1.1.4 对话历史压缩（已实现）

在 [context.rs](file:///d:/DeskTop/DocAgent/src-tauri/src/services/agent/context.rs) 的 `compress_history_if_needed` 方法中实现了：
- 滑动窗口策略：保留最近N轮完整消息（默认2轮）
- 保护第一条用户消息（原始意图）
- 被跳过消息的摘要占位符 `[系统摘要: 已省略 X 条早期对话消息]`
- reasoning_content 的早期压缩（超过500字符截取前200字符）

**评估**: 压缩策略属于最基础的滑动窗口，与上下文工程理论中的"分层摘要"和"关键轮标记"策略存在显著差距：
- 摘要占位符仅告知省略了消息数量，不包含任何实质内容摘要
- 无法保留被省略消息中的关键决策和执行结果
- 没有区分"关键轮次"和"普通轮次"

#### 1.1.5 迭代上下文注入（已实现）

在 [context.rs](file:///d:/DeskTop/DocAgent/src-tauri/src/services/agent/context.rs) 的 `build_iteration_context` 方法中实现了：
- 迭代轮次进度展示（当前轮/最大轮）
- 已完成步骤列表
- 当前正在执行的步骤
- "不要重复已完成的步骤"提示

**评估**: 迭代上下文是"上下文水印"思想的雏形，但信息量有限，缺少上下文策略版本、Token消耗量等调试信息。

#### 1.1.6 提示词外部化加载（已实现）

在 [prompt_loader.rs](file:///d:/DeskTop/DocAgent/src-tauri/src/services/agent/prompts/prompt_loader.rs) 中实现了：
- 从 TOML 文件加载提示词各层内容
- 文件不存在时回退到硬编码默认值
- 版本信息管理
- 设计规范和示例的外部化加载

**评估**: 外部化机制已建立，支持运行时修改提示词而无需重新编译，但当前实际未使用外部文件，全部依赖硬编码默认值。

### 1.2 上下文工程能力差距分析

对照菜鸟教程《Agent 上下文工程》的理论框架，项目存在以下关键差距：

| 上下文工程实践 | 理论要求 | 项目现状 | 差距等级 |
|----------------|----------|----------|----------|
| **上下文记忆** | 工作记忆（会话内）、情景记忆（跨会话）、语义记忆（长期知识） | 仅有会话内工作记忆，Agent每次启动创建空AgentContext | 严重 |
| **上下文预算管理** | 精确Token计数、动态预算分配、实时使用量追踪 | 粗略字符估算、固定比例分配、无运行时追踪 | 严重 |
| **工具定义优化** | 按任务阶段动态注册工具集、描述精炼、参数约束明确 | 全量发送14个工具定义、无动态过滤 | 严重 |
| **历史记录压缩** | 分层摘要（近期完整+远期结构化摘要）、关键轮标记 | 仅滑动窗口+占位符、无实质摘要 | 严重 |
| **检索上下文** | RAG、查询改写、相关性过滤、来源标注 | 完全缺失 | 中等 |
| **渐进式披露** | 按执行阶段逐步注入工具和规则 | 仅按任务类型注入规范层和示例层 | 中等 |
| **上下文水印** | 策略版本、Token消耗、来源标注等调试信息 | 仅迭代轮次和步骤进度 | 轻微 |
| **上下文压缩链** | 链式调用拆分大任务、中间结果精炼 | 完全缺失 | 中等 |
| **评估与迭代** | 任务完成率、上下文效率、工具调用质量、响应一致性 | 完全缺失 | 严重 |
| **模型适配** | 根据不同Provider/模型调整上下文策略 | 统一策略，仅reasoning_in_content适配 | 中等 |
| **用户输入优化** | 查询改写、意图澄清、输入分解 | 完全缺失 | 轻微 |

---

## 二、开发方案设计

### 2.1 设计原则

1. **渐进增强**: 在现有架构基础上逐步增强，不破坏已有功能
2. **数据驱动**: 所有优化策略基于实际运行数据，而非主观判断
3. **可观测性**: 上下文工程的每个决策点都应有日志和指标支撑
4. **模型无关**: 上下文策略应适配不同LLM Provider的能力差异
5. **向后兼容**: 新增功能通过配置开关控制，默认行为与当前一致

### 2.2 技术选型

| 领域 | 选型 | 理由 |
|------|------|------|
| Token 计数 | tiktoken-rs (Rust crate) | OpenAI 官方分词器的 Rust 移植，支持 cl100k_base/o200k_base 等编码，精确度高 |
| 向量检索 | SQLite vec 扩展 / 本地 FAISS | 项目已使用 SQLite，vec 扩展可复用现有基础设施；FAISS 适合大规模向量检索 |
| 文本摘要 | LLM 自身能力 | 利用 Agent 已有的 LLM 连接，对历史消息进行结构化摘要，无需额外模型 |
| 上下文指标 | Prometheus 风格的内存指标 | 轻量级运行时指标收集，不依赖外部服务 |
| 配置管理 | 现有 JSON 配置体系 | 复用 ConfigManager，新增上下文工程配置段 |

### 2.3 架构设计

```
src-tauri/src/services/agent/
  context.rs              (已有，增强)
  executor.rs             (已有，增强)
  prompts/
    mod.rs                (已有)
    task_type.rs          (已有，增强)
    token_budget.rs       (已有，重写)
    document_design.rs    (已有)
    prompt_loader.rs      (已有，增强)
    + context_strategy.rs (新增: 上下文策略引擎)
    + context_metrics.rs  (新增: 上下文指标收集)
    + tool_selector.rs    (新增: 动态工具选择器)
    + history_compressor.rs (新增: 高级历史压缩器)
    + context_watermark.rs  (新增: 上下文水印)
    + memory/             (新增: 上下文记忆系统)
      + mod.rs
      + session_memory.rs   (会话记忆: 历史消息加载与恢复)
      + episodic_memory.rs  (情景记忆: 跨会话摘要持久化)
      + semantic_memory.rs  (语义记忆: 用户偏好与知识积累)
src-tauri/src/db/
  + session_summary_repo.rs (新增: 会话摘要数据访问层)
  + user_preference_repo.rs (新增: 用户偏好数据访问层)
src-tauri/src/models/
  + session_summary.rs    (新增: 会话摘要数据模型)
  + user_preference.rs    (新增: 用户偏好数据模型)
```

---

## 三、详细实施步骤

### 阶段零：上下文记忆系统（优先级：P0-Critical，紧急修复）

> **背景**: 用户在已有历史会话中继续提问时，Agent 回复"这是一次新对话，没有历史会话"，但页面上能看到之前的对话记录。根因是后端 `run_agent` 每次创建全新的 `AgentContext`，从不加载历史消息。数据库层已具备 `list_messages` 查询能力但未被调用。这是当前最影响用户体验的问题，必须最优先修复。

#### 任务 0.1：会话内历史消息加载（紧急 Bug 修复）

**目标**: Agent 启动时从数据库加载当前 session_id 的历史消息，注入 AgentContext，使 Agent 能感知之前的对话内容

**问题根因链路**:
1. 前端 `handleSwitchSession` 正确调用了 `getSession` 并通过 `loadFromMessages` 展示历史消息
2. 但后端 [agent.rs:274](file:///d:/DeskTop/DocAgent/src-tauri/src/commands/agent.rs#L274) 中 `AgentContext::new(session_id, system_prompt)` 创建的 `messages` 为空 `Vec`
3. [agent.rs:295](file:///d:/DeskTop/DocAgent/src-tauri/src/commands/agent.rs#L295) 仅添加当前用户消息 `ctx.add_user_message(prompt)`
4. 数据库 [message_repo.rs:41-154](file:///d:/DeskTop/DocAgent/src-tauri/src/db/message_repo.rs#L41-L154) 的 `list_messages` 已实现但从未被 `run_agent` 调用

**修改文件**:
- `src-tauri/src/commands/agent.rs`: 在 `run_agent` 中添加历史消息加载逻辑
- `src-tauri/src/services/agent/context.rs`: 新增 `load_history_messages` 方法

**实施步骤**:

1. 在 `AgentContext` 中新增历史消息加载方法：
```rust
/// 从数据库加载历史消息并注入上下文
pub fn load_history_messages(&mut self, messages: Vec<ChatMessage>) {
    for msg in messages {
        self.messages.push(msg);
    }
    // 加载历史后更新任务类型识别
    self.update_task_type_from_history();
}
```

2. 在 `run_agent` 函数中，`ctx.add_user_message(prompt)` 之前插入历史加载：
```rust
// 从数据库加载该会话的历史消息
let history_messages = {
    let db = &state.db;
    match db.list_messages(&session_id) {
        Ok(msgs) => {
            // 将数据库 Message 模型转换为 LLM ChatMessage 模型
            msgs.into_iter()
                .filter_map(|m| m.to_chat_message())
                .collect::<Vec<ChatMessage>>()
        }
        Err(e) => {
            log::warn!("加载历史消息失败: {}, 将以空上下文启动", e);
            Vec::new()
        }
    }
};

// 注入历史消息到上下文（在添加当前用户消息之前）
if !history_messages.is_empty() {
    ctx.load_history_messages(history_messages);
}

// 添加当前用户消息
ctx.add_user_message(prompt);
```

3. 在 `Message` 模型中添加 `to_chat_message` 转换方法：
```rust
impl Message {
    /// 将数据库消息模型转换为 LLM ChatMessage
    pub fn to_chat_message(&self) -> Option<ChatMessage> {
        match self.role.as_str() {
            "user" => Some(ChatMessage::user(&self.content)),
            "assistant" => Some(ChatMessage::assistant(&self.content)),
            "system" => Some(ChatMessage::system(&self.content)),
            _ => None,
        }
    }
}
```

4. 处理 tool_call 和 tool_result 消息的转换：
- 数据库中 `tool_calls` 字段（JSON）需要反序列化为 `ToolCall` 结构
- `tool_call_id` 字段需要正确映射
- 无效或损坏的历史消息应跳过而非阻塞

5. 历史消息的 Token 预算控制：
- 加载历史后调用 `compress_history_if_needed` 进行压缩
- 确保历史消息 + 当前消息不超过 Token 预算
- 保留第一条用户消息（原始意图）和最近N轮完整消息

**验收指标**:
- 用户在已有会话中继续提问时，Agent 能正确引用之前的对话内容
- 历史消息加载后经压缩不超出 Token 预算
- 损坏的历史消息不阻塞 Agent 启动
- 新会话（无历史消息）行为不变

#### 任务 0.2：会话摘要持久化（情景记忆）

**目标**: Agent 执行结束时自动生成会话摘要并持久化，新会话启动时可检索同工作区的历史摘要注入上下文

**新增文件**:
- `src-tauri/src/models/session_summary.rs`: 会话摘要数据模型
- `src-tauri/src/db/session_summary_repo.rs`: 会话摘要数据访问层
- `src-tauri/src/services/agent/memory/session_memory.rs`: 会话记忆管理器

**修改文件**:
- `src-tauri/src/db/init.rs`: 新增 `session_summaries` 表
- `src-tauri/src/commands/agent.rs`: Agent 完成时生成摘要
- `src-tauri/src/services/agent/context.rs`: 新会话启动时注入历史摘要

**实施步骤**:

1. 定义会话摘要数据模型：
```rust
pub struct SessionSummary {
    pub id: String,
    pub session_id: String,
    pub workspace_id: String,
    /// 用户原始目标
    pub user_goal: String,
    /// Agent 执行结果摘要
    pub result_summary: String,
    /// 涉及的文件列表
    pub files_involved: Vec<String>,
    /// 使用的工具列表
    pub tools_used: Vec<String>,
    /// 遇到的错误及解决方案
    pub errors_resolved: Vec<String>,
    /// 创建时间
    pub created_at: String,
}
```

2. 数据库迁移（新增表）：
```sql
CREATE TABLE IF NOT EXISTS session_summaries (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    workspace_id TEXT NOT NULL,
    user_goal TEXT NOT NULL,
    result_summary TEXT NOT NULL,
    files_involved TEXT NOT NULL DEFAULT '[]',
    tools_used TEXT NOT NULL DEFAULT '[]',
    errors_resolved TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES sessions(id),
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id)
);
CREATE INDEX IF NOT EXISTS idx_summaries_workspace ON session_summaries(workspace_id, created_at DESC);
```

3. 在 Agent 完成时生成摘要（纯规则，无额外 LLM 调用）：
```rust
/// 从 AgentContext 的执行记录中提取结构化摘要
fn generate_session_summary(ctx: &AgentContext, workspace_id: &str) -> SessionSummary {
    // 从 completed_steps 提取用户目标
    // 从 tool_call 消息中提取涉及文件和使用的工具
    // 从 error 消息中提取错误和解决方案
    // 从最后的 assistant 消息中提取结果摘要
}
```

4. 新会话启动时注入历史摘要：
```rust
/// 在系统提示词的上下文层中注入历史摘要
fn inject_historical_summaries(
    &mut self,
    summaries: &[SessionSummary],
) {
    if summaries.is_empty() {
        return;
    }
    // 最多注入最近3条摘要，避免占用过多上下文
    let recent_summaries: Vec<&SessionSummary> = summaries
        .iter()
        .take(3)
        .collect();

    let summary_text = recent_summaries.iter()
        .map(|s| format!("- 用户目标: {} | 结果: {} | 涉及文件: {:?}",
            s.user_goal, s.result_summary, s.files_involved))
        .collect::<Vec<_>>()
        .join("\n");

    // 注入到上下文层
    self.context_layer_history = Some(summary_text);
}
```

**验收指标**:
- Agent 完成时自动生成并持久化会话摘要
- 新会话启动时能检索同工作区的历史摘要
- 历史摘要注入后 Agent 能参考之前的交互结果
- 摘要 Token 占用不超过上下文预算的 5%

#### 任务 0.3：用户偏好记忆（语义记忆）

**目标**: 从历史交互中提取并持久化用户偏好，在后续会话中自动应用

**新增文件**:
- `src-tauri/src/models/user_preference.rs`: 用户偏好数据模型
- `src-tauri/src/db/user_preference_repo.rs`: 用户偏好数据访问层
- `src-tauri/src/services/agent/memory/semantic_memory.rs`: 语义记忆管理器

**修改文件**:
- `src-tauri/src/db/init.rs`: 新增 `user_preferences` 表
- `src-tauri/src/services/agent/context.rs`: 在上下文层注入用户偏好

**实施步骤**:

1. 定义用户偏好数据模型：
```rust
pub struct UserPreference {
    pub id: String,
    /// 偏好类别（format/style/language/naming 等）
    pub category: String,
    /// 偏好键
    pub key: String,
    /// 偏好值
    pub value: String,
    /// 置信度（0.0-1.0，基于观察次数）
    pub confidence: f64,
    /// 观察次数
    pub observation_count: u32,
    /// 最后观察时间
    pub last_observed_at: String,
}
```

2. 数据库迁移：
```sql
CREATE TABLE IF NOT EXISTS user_preferences (
    id TEXT PRIMARY KEY,
    category TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    confidence REAL NOT NULL DEFAULT 0.5,
    observation_count INTEGER NOT NULL DEFAULT 1,
    last_observed_at TEXT NOT NULL,
    UNIQUE(category, key)
);
```

3. 偏好提取规则（纯规则，从工具调用参数中提取）：
- `generate_document` 的 `format` 参数 → 偏好文档格式
- `generate_document` 的内容语言 → 偏好输出语言
- 文件命名模式 → 偏好命名风格
- 反复使用的工具 → 偏好工作流模式

4. 在系统提示词上下文层注入用户偏好：
```rust
fn inject_user_preferences(&mut self, preferences: &[UserPreference]) {
    // 仅注入置信度 > 0.7 的偏好
    let high_confidence: Vec<&UserPreference> = preferences
        .iter()
        .filter(|p| p.confidence > 0.7)
        .collect();

    if high_confidence.is_empty() {
        return;
    }

    let pref_text = high_confidence.iter()
        .map(|p| format!("- {}: {}", p.key, p.value))
        .collect::<Vec<_>>()
        .join("\n");

    self.context_layer_preferences = Some(pref_text);
}
```

**验收指标**:
- 用户重复使用相同文档格式时，偏好被自动记录
- 新会话中 Agent 能参考用户偏好做出推荐
- 偏好可被用户手动清除或修改
- 偏好注入不影响 Token 预算（控制在 500 token 以内）

---

### 阶段一：Token 精确计数与预算管理增强（优先级：P0）

#### 任务 1.1：引入 tiktoken-rs 实现精确 Token 计数

**目标**: 替换当前的 `chars().count()` 估算为精确的 Token 计数

**修改文件**:
- `src-tauri/Cargo.toml`: 添加 `tiktoken-rs` 依赖
- `src-tauri/src/services/agent/prompts/token_budget.rs`: 重写 `estimate_tokens` 方法

**实施步骤**:

1. 在 `Cargo.toml` 中添加依赖：
```toml
[dependencies]
tiktoken-rs = "0.6"
```

2. 重写 `TokenBudgetManager::estimate_tokens` 方法：
```rust
pub fn estimate_tokens(text: &str) -> usize {
    // 使用 tiktoken-rs 的 cl100k_base 编码（GPT-4/4o 系列）
    // 对中文内容：约 1.5 token/字符，远比 chars().count() 精确
    use tiktoken_rs::cl100k_base;
    if let Ok(bpe) = cl100k_base() {
        return bpe.encode_with_special_tokens(text).len();
    }
    // 回退到字符数估算
    text.chars().count()
}
```

3. 新增按模型选择编码的方法：
```rust
pub fn estimate_tokens_for_model(text: &str, model: &str) -> usize {
    let bpe = if model.starts_with("gpt-4") || model.starts_with("gpt-3.5") {
        tiktoken_rs::cl100k_base().ok()
    } else if model.starts_with("gpt-4o") || model.starts_with("o1") {
        tiktoken_rs::o200k_base().ok()
    } else {
        // 非 OpenAI 模型使用 cl100k_base 作为通用估算
        tiktoken_rs::cl100k_base().ok()
    };
    match bpe {
        Some(bpe) => bpe.encode_with_special_tokens(text).len(),
        None => text.chars().count(),
    }
}
```

**验收指标**:
- Token 计数误差 < 5%（与 OpenAI API 返回的 usage 对比）
- 对中文内容的计数精度显著优于 `chars().count()`
- 现有单元测试全部通过

#### 任务 1.2：动态上下文窗口适配

**目标**: 根据当前使用的 LLM 模型自动设置上下文窗口大小

**修改文件**:
- `src-tauri/src/services/agent/prompts/token_budget.rs`: 新增模型上下文窗口映射
- `src-tauri/src/commands/agent.rs`: 传递模型信息到 AgentContext

**实施步骤**:

1. 在 `token_budget.rs` 中新增模型窗口映射：
```rust
pub fn context_window_for_model(model: &str) -> usize {
    match model {
        // OpenAI 系列
        m if m.starts_with("gpt-4o") => 128_000,
        m if m.starts_with("gpt-4-turbo") => 128_000,
        m if m.starts_with("gpt-4-32k") => 32_768,
        m if m.starts_with("gpt-4") => 8_192,
        m if m.starts_with("gpt-3.5-turbo-16k") => 16_384,
        m if m.starts_with("gpt-3.5") => 4_096,
        // Anthropic 系列
        m if m.contains("claude-3-5") => 200_000,
        m if m.contains("claude-3") => 200_000,
        // DeepSeek 系列
        m if m.contains("deepseek") => 128_000,
        // Gemini 系列
        m if m.contains("gemini-1.5") => 1_000_000,
        m if m.contains("gemini") => 32_000,
        // Ollama 本地模型
        m if m.contains("llama-3") => 8_192,
        m if m.contains("qwen") => 32_000,
        // 默认值
        _ => 128_000,
    }
}
```

2. 在 `AgentContext::new` 中接受模型信息并初始化对应的预算管理器

3. 在 `run_agent` 函数中从 LlmRouter 获取当前模型的上下文窗口大小

**验收指标**:
- 不同模型自动使用正确的上下文窗口大小
- 预算分配根据窗口大小动态调整
- 小窗口模型（如8K）的预算分配合理

#### 任务 1.3：运行时 Token 使用量追踪

**目标**: 实时追踪上下文各部分的 Token 消耗，为后续优化提供数据支撑

**新增文件**:
- `src-tauri/src/services/agent/prompts/context_metrics.rs`

**实施步骤**:

1. 定义上下文指标结构体：
```rust
pub struct ContextMetrics {
    /// 系统提示词 Token 数
    pub system_prompt_tokens: usize,
    /// 工具定义 Token 数
    pub tool_definitions_tokens: usize,
    /// 对话历史 Token 数
    pub conversation_tokens: usize,
    /// 用户输入 Token 数
    pub user_input_tokens: usize,
    /// 检索上下文 Token 数（预留）
    pub retrieved_context_tokens: usize,
    /// 总输入 Token 数
    pub total_input_tokens: usize,
    /// LLM 响应 Token 数
    pub response_tokens: usize,
    /// 上下文窗口大小
    pub context_window: usize,
    /// 上下文利用率
    pub utilization_rate: f64,
}
```

2. 在 `AgentContext` 中集成指标收集：
- 每次 `get_messages_for_iteration` 调用时计算各部分 Token 数
- 记录 LLM API 返回的 `usage` 字段（如可用）
- 计算上下文利用率 = 实际使用 / 上下文窗口

3. 在日志中输出 Token 使用报告：
```
[上下文指标] 系统提示=3200 工具定义=5800 对话历史=42000 用户输入=150 总输入=51150/128000 利用率=39.9%
```

4. 通过 Tauri 事件向前端发送上下文指标（可选，用于UI展示）

**验收指标**:
- 每次迭代输出 Token 使用报告
- 上下文利用率计算准确
- 指标数据可用于后续优化决策

---

### 阶段二：动态工具选择与优化（优先级：P0）

#### 任务 2.1：基于任务类型的工具集动态过滤

**目标**: 根据当前任务类型和执行阶段，仅向 LLM 发送相关的工具定义，减少无效 Token 消耗

**新增文件**:
- `src-tauri/src/services/agent/prompts/tool_selector.rs`

**实施步骤**:

1. 定义工具与任务类型的关联矩阵：
```rust
pub struct ToolSelector {
    /// 工具-任务类型关联映射
    tool_task_matrix: HashMap<String, HashSet<TaskType>>,
}

impl ToolSelector {
    /// 根据任务类型选择相关工具
    pub fn select_tools(
        &self,
        task_type: &TaskType,
        tool_registry: &ToolRegistry,
        handler_registry: &HandlerRegistry,
    ) -> Vec<serde_json::Value> {
        // 基础工具始终包含：list_directory, search_files, read_file, file_info, file_exists
        // 按任务类型选择性包含高级工具
        match task_type {
            TaskType::GenerateDocx | TaskType::GenerateXlsx | ... => {
                // 生成类：generate_document + 基础工具
            }
            TaskType::ReadDocument => {
                // 读取类：read_document + 基础工具
            }
            TaskType::ModifyDocument => {
                // 修改类：modify_document + read_document + 基础工具
            }
            TaskType::ConvertFormat => {
                // 转换类：convert_format + 基础工具
            }
            TaskType::Unknown => {
                // 未知类型：发送全部工具（安全回退）
            }
        }
    }
}
```

2. 在 `AgentExecutor::execute` 中替换全量工具定义获取逻辑：
```rust
// 当前代码（全量发送）:
let tool_defs_json = {
    let tool_defs = self.tool_registry.tool_definitions();
    let handler_defs = { ... };
    [tool_defs, handler_defs].concat()
};

// 优化后（按需发送）:
let tool_defs_json = self.tool_selector.select_tools(
    ctx.task_type(),
    &self.tool_registry,
    &self.handler_registry,
);
```

3. 支持执行阶段动态调整工具集：
- 初始阶段：仅基础工具 + 任务相关工具
- 执行阶段：根据已调用工具动态补充可能需要的工具
- 完成阶段：移除不再需要的工具

**验收指标**:
- 工具定义 Token 消耗减少 30%-50%
- 任务完成率不下降（回归测试）
- LLM 工具调用准确率不下降

#### 任务 2.2：工具描述精炼与参数约束增强

**目标**: 优化每个工具的描述文本，使其更简洁且信息密度更高

**修改文件**:
- 各 Tool/Handler 实现文件中的 `description()` 和 `parameters()` 方法

**实施步骤**:

1. 审查所有14个工具的描述，遵循以下原则：
   - 每个工具描述不超过2句话：第1句说明用途，第2句说明关键约束
   - 参数描述中明确使用场景和限制条件
   - 移除冗余信息和重复说明

2. 优化参数 JSON Schema：
   - 添加 `enum` 约束减少无效参数值
   - 添加 `examples` 字段提供典型用法
   - 必填/选填标记清晰

3. 示例对比：
```
优化前:
"generate_document": "生成文档。支持生成 Word、Excel、PPT、PDF、Markdown 格式的文档。可以根据用户需求生成各种类型的文档，包括报告、合同、方案等。"

优化后:
"generate_document": "生成 docx/xlsx/pptx/pdf/md 格式文档。format 参数必填，content 需包含结构化的文档内容定义。"
```

**验收指标**:
- 工具描述总 Token 数减少 20%-30%
- LLM 工具调用参数错误率降低
- 工具选择准确率不下降

---

### 阶段三：高级历史压缩策略（优先级：P1）

#### 任务 3.1：实现分层摘要压缩

**目标**: 替换当前的简单滑动窗口+占位符策略，实现"近期完整+远期结构化摘要"的分层压缩

**新增文件**:
- `src-tauri/src/services/agent/prompts/history_compressor.rs`

**实施步骤**:

1. 定义结构化摘要格式：
```rust
pub struct ConversationSummary {
    /// 用户原始目标
    pub user_goal: String,
    /// Agent 已完成的关键操作列表
    pub completed_actions: Vec<ActionSummary>,
    /// 关键决策和结果
    pub key_decisions: Vec<String>,
    /// 遇到的错误和解决方案
    pub errors_encountered: Vec<ErrorSummary>,
    /// 当前执行状态
    pub current_status: String,
}

pub struct ActionSummary {
    /// 工具名称
    pub tool: String,
    /// 操作目标（文件路径等）
    pub target: String,
    /// 操作结果（成功/失败 + 关键输出）
    pub result: String,
}

pub struct ErrorSummary {
    /// 错误类型
    pub error_type: String,
    /// 错误信息
    pub message: String,
    /// 解决方案
    pub resolution: String,
}
```

2. 实现分层压缩逻辑：
```rust
pub fn compress_with_summary(
    messages: &[ChatMessage],
    keep_recent_rounds: usize,
    budget: usize,
) -> Vec<ChatMessage> {
    // 1. 保留第一条用户消息（原始意图）
    // 2. 对超出保留窗口的早期消息，生成结构化摘要
    // 3. 将摘要作为一条 assistant 消息插入
    // 4. 保留最近 N 轮完整消息
}
```

3. 利用 LLM 生成结构化摘要（可选方案）：
- 当早期消息超过阈值时，调用 LLM 对历史消息进行摘要
- 摘要 prompt 要求输出结构化 JSON 格式
- 摘要结果缓存，避免重复生成

4. 纯规则摘要（推荐方案，无额外 LLM 调用）：
- 从 tool_result 消息中提取关键信息
- 从 assistant 消息中提取决策描述
- 从 error 消息中提取错误和解决方案
- 拼接为结构化摘要文本

**验收指标**:
- 压缩后对话历史 Token 数减少 40%-60%
- 摘要包含足够信息使 Agent 能正确继续任务
- 长对话场景（10+轮）的任务完成率不下降

#### 任务 3.2：关键轮次标记与保护

**目标**: 识别并保护对话中的关键轮次，避免重要信息被压缩丢失

**修改文件**:
- `src-tauri/src/services/agent/context.rs`: 扩展 `ChatMessage` 或 `AgentContext`

**实施步骤**:

1. 定义关键轮次判定规则：
- 包含用户明确指令变更的消息
- 包含工具调用失败和重试成功的消息
- 包含用户确认/拒绝操作的消息
- 包含文档生成/修改最终结果的消息

2. 在 `AgentContext` 中添加关键轮次标记：
```rust
/// 标记指定消息索引为关键轮次
fn mark_critical_round(&mut self, message_index: usize, reason: String)
```

3. 在压缩逻辑中保护关键轮次：
- 关键轮次的消息不被压缩，始终保持完整
- 非关键轮次可被摘要替换

**验收指标**:
- 关键决策信息在压缩后仍可追溯
- 用户确认/拒绝记录在长对话中不丢失
- 压缩效率不受显著影响

---

### 阶段四：上下文策略引擎（优先级：P1）

#### 任务 4.1：上下文策略引擎核心实现

**目标**: 建立统一的上下文策略决策框架，协调预算分配、工具选择、历史压缩等策略

**新增文件**:
- `src-tauri/src/services/agent/prompts/context_strategy.rs`

**实施步骤**:

1. 定义上下文策略接口：
```rust
pub trait ContextStrategy: Send + Sync {
    /// 根据当前状态决定上下文构建策略
    fn decide(&self, state: &ContextState) -> ContextDecision;
}

pub struct ContextState {
    /// 当前任务类型
    pub task_type: TaskType,
    /// 当前迭代轮次
    pub iteration: u32,
    /// 已使用 Token 数
    pub tokens_used: usize,
    /// 上下文窗口大小
    pub context_window: usize,
    /// 对话轮数
    pub conversation_rounds: usize,
    /// 已调用工具列表
    pub tools_called: Vec<String>,
    /// 是否存在错误
    pub has_errors: bool,
}

pub struct ContextDecision {
    /// 应发送的工具列表
    pub tools_to_send: Vec<String>,
    /// 历史压缩策略
    pub compression: CompressionDecision,
    /// 是否注入规范层
    pub inject_guides: bool,
    /// 是否注入示例层
    pub inject_examples: bool,
    /// 迭代上下文内容
    pub iteration_context: Option<String>,
}
```

2. 实现默认策略：
```rust
pub struct DefaultContextStrategy;

impl ContextStrategy for DefaultContextStrategy {
    fn decide(&self, state: &ContextState) -> ContextDecision {
        // 基于任务类型选择工具
        // 基于Token使用量决定压缩策略
        // 基于迭代轮次决定注入内容
        // 基于错误状态调整策略
    }
}
```

3. 在 `AgentExecutor` 中集成策略引擎：
- 每次迭代前调用策略引擎获取决策
- 根据决策构建上下文

**验收指标**:
- 策略引擎能根据运行状态动态调整上下文构建
- 不同场景下策略决策合理
- 策略可配置、可扩展

#### 任务 4.2：上下文水印实现

**目标**: 在上下文中注入调试信息，帮助监控和诊断 Agent 行为

**新增文件**:
- `src-tauri/src/services/agent/prompts/context_watermark.rs`

**实施步骤**:

1. 定义水印格式：
```rust
pub struct ContextWatermark {
    /// 上下文策略版本
    pub strategy_version: String,
    /// 当前 Token 使用情况
    pub token_usage: String,
    /// 工具选择策略
    pub tool_selection: String,
    /// 历史压缩策略
    pub compression_strategy: String,
    /// 检索上下文来源（预留）
    pub retrieved_sources: Vec<String>,
}
```

2. 在系统提示词末尾注入水印（以 XML 注释形式，不影响 LLM 行为）：
```xml
<!-- context_watermark
  strategy_version: v2.3
  token_usage: 51150/128000 (39.9%)
  tool_selection: task_based(generate_docx)
  compression: sliding_window(keep=2)
  retrieved_sources: none
-->
```

3. 在日志中记录水印信息，用于调试和优化

**验收指标**:
- 水印信息准确反映当前上下文策略
- 水印不影响 LLM 输出质量
- 调试时可快速定位上下文构建问题

---

### 阶段五：模型适配与 Provider 感知（优先级：P2）

#### 任务 5.1：Provider 感知的上下文构建

**目标**: 根据当前 LLM Provider 的能力差异调整上下文策略

**修改文件**:
- `src-tauri/src/services/agent/context.rs`: 增强 `build_system_prompt_with_task`
- `src-tauri/src/services/agent/executor.rs`: 传递 Provider 信息

**实施步骤**:

1. 定义 Provider 能力描述：
```rust
pub struct ProviderCapabilities {
    /// 上下文窗口大小
    pub context_window: usize,
    /// 是否支持 tool_call
    pub supports_tool_call: bool,
    /// 是否支持 reasoning_content
    pub supports_reasoning: bool,
    /// 最大输出 Token 数
    pub max_output_tokens: usize,
    /// 是否支持系统提示词
    pub supports_system_prompt: bool,
    /// 推荐的系统提示词格式
    pub preferred_prompt_format: PromptFormat,
}

pub enum PromptFormat {
    /// XML 标签格式（当前使用）
    XmlTags,
    /// Markdown 格式
    Markdown,
    /// 纯文本格式
    PlainText,
}
```

2. 为不同 Provider 预设能力描述：
- OpenAI: 128K窗口，支持tool_call，cl100k_base编码
- Anthropic: 200K窗口，支持tool_call，XML标签格式偏好
- Gemini: 1M窗口，支持tool_call
- Ollama: 视模型而定，可能不支持tool_call

3. 在上下文构建时根据 Provider 能力调整：
- 不支持 tool_call 的 Provider：将工具描述嵌入系统提示词
- 小窗口 Provider：更激进地压缩历史和精简工具
- 不同格式偏好的 Provider：调整系统提示词格式

**验收指标**:
- 不同 Provider 的上下文策略自动适配
- Ollama 等不支持 tool_call 的 Provider 能正常工作
- 小窗口模型的上下文利用率合理

#### 任务 5.2：动态预算分配

**目标**: 根据任务类型和执行阶段动态调整各部分 Token 预算

**修改文件**:
- `src-tauri/src/services/agent/prompts/token_budget.rs`: 新增动态预算计算

**实施步骤**:

1. 定义任务类型与预算分配的映射：
```rust
pub fn budget_allocation_for_task(task_type: &TaskType) -> BudgetAllocation {
    match task_type {
        // 代码生成类：检索上下文比例调高
        TaskType::GenerateDocx | TaskType::GenerateXlsx | ... => {
            BudgetAllocation {
                system_prompt: 0.10,  // 10%
                tool_definitions: 0.10, // 10%
                conversation: 0.35,  // 35%
                retrieved_context: 0.30, // 30%（预留）
                response: 0.15,     // 15%
            }
        }
        // 多轮对话类：历史记录预算优先
        TaskType::ReadDocument | TaskType::ModifyDocument => {
            BudgetAllocation {
                system_prompt: 0.08,
                tool_definitions: 0.12,
                conversation: 0.50,
                retrieved_context: 0.10,
                response: 0.20,
            }
        }
        _ => BudgetAllocation::default(),
    }
}
```

2. 在 `TokenBudgetManager` 中支持动态预算分配

**验收指标**:
- 不同任务类型使用不同的预算分配策略
- 上下文利用率提升 10%-20%
- 无预算溢出导致的 API 错误

---

### 阶段六：评估与迭代体系（优先级：P2）

#### 任务 6.1：上下文工程评估指标体系

**目标**: 建立上下文工程的评估指标，为持续优化提供数据支撑

**新增文件**:
- `src-tauri/src/services/agent/prompts/context_metrics.rs`（扩展）

**实施步骤**:

1. 定义核心评估指标：
```rust
pub struct ContextEngineeringMetrics {
    // 任务完成指标
    /// 任务完成率
    pub task_completion_rate: f64,
    /// 平均迭代次数
    pub avg_iterations: f64,
    /// 失败原因分布
    pub failure_distribution: HashMap<String, u32>,

    // 上下文效率指标
    /// 平均 Token 消耗
    pub avg_token_consumption: usize,
    /// 上下文利用率
    pub avg_utilization_rate: f64,
    /// 系统提示词占比
    pub system_prompt_ratio: f64,
    /// 工具定义占比
    pub tool_definitions_ratio: f64,
    /// 对话历史占比
    pub conversation_ratio: f64,

    // 工具调用质量指标
    /// 工具调用准确率
    pub tool_call_accuracy: f64,
    /// 无效调用占比
    pub invalid_tool_call_rate: f64,
    /// 参数错误率
    pub parameter_error_rate: f64,
    /// 平均重试次数
    pub avg_retries: f64,

    // 响应一致性指标
    /// 相同输入输出稳定性（需多次测试）
    pub response_stability: f64,
    /// 格式合规率
    pub format_compliance_rate: f64,
}
```

2. 在 Agent 执行流程中收集指标数据：
- 在 `AgentExecutor::execute` 的每次迭代中记录 Token 使用
- 在工具调用结果中统计成功/失败/重试
- 在 Agent 完成时汇总指标

3. 将指标持久化到数据库（新增 metrics 表）

4. 提供指标查询 API（Tauri 命令）

**验收指标**:
- 每次 Agent 执行自动收集完整指标
- 指标数据可查询、可聚合
- 指标数据可用于优化决策

#### 任务 6.2：A/B 测试框架（可选）

**目标**: 支持对不同上下文策略进行 A/B 测试，数据驱动优化

**实施步骤**:

1. 定义策略变体配置：
```rust
pub struct StrategyVariant {
    /// 变体名称
    pub name: String,
    /// 上下文策略配置
    pub config: ContextStrategyConfig,
    /// 流量比例（0.0-1.0）
    pub traffic_ratio: f64,
}
```

2. 在 Agent 启动时根据配置选择策略变体

3. 收集各变体的指标数据并对比

**验收指标**:
- 支持同时运行不同策略变体
- 指标数据可按变体分组对比
- 统计显著性可计算

---

### 阶段七：检索上下文增强（优先级：P3，远期规划）

#### 任务 7.1：工作区文件索引与语义检索

**目标**: 对工作区文件建立索引，支持按语义相关性检索文件内容注入上下文

**实施步骤**:

1. 文件索引构建：
- 监听工作区文件变更事件
- 对文本文件进行分块和向量化
- 存储向量索引到本地

2. 查询改写：
- 在 Agent 接收到用户消息后，先对查询进行改写
- 提取关键实体和意图
- 生成检索查询

3. 相关性检索：
- 根据改写后的查询检索相关文件片段
- 按相关性排序
- 截取最相关的片段注入上下文

4. 来源标注：
- 每个检索结果附带文件路径和更新时间
- 在上下文中标注来源信息

**验收指标**:
- 检索结果与用户意图相关性 > 80%
- 检索延迟 < 500ms
- 上下文中检索结果 Token 占比 < 25%

#### 任务 7.2：上下文压缩链

**目标**: 对大量数据处理任务实现链式调用，每步只保留关键中间结果

**实施步骤**:

1. 识别需要压缩链的场景：
- 批量文件分析
- 大文件内容提取
- 多步骤文档生成

2. 实现链式执行框架：
- 每步执行后对结果进行摘要
- 摘要作为下一步的输入
- 原始数据不进入后续迭代的上下文

**验收指标**:
- 大数据处理任务的上下文消耗可控
- 链式执行的信息损失可接受
- 任务完成率不下降

---

## 四、质量标准

### 4.1 代码质量标准

| 维度 | 标准 |
|------|------|
| 单元测试覆盖率 | 新增代码覆盖率 >= 80% |
| 集成测试 | 每个阶段完成后执行端到端测试 |
| 代码审查 | 所有变更需通过代码审查 |
| 向后兼容 | 新功能默认关闭，通过配置开关启用 |
| 日志规范 | 关键决策点必须有 INFO 级别日志 |
| 错误处理 | 所有新增错误纳入统一错误码体系 |

### 4.2 性能标准

| 指标 | 目标值 |
|------|--------|
| Token 计数延迟 | < 10ms / 1000字符 |
| 工具选择延迟 | < 5ms |
| 历史压缩延迟 | < 50ms |
| 上下文构建总延迟 | < 100ms |
| 内存增量 | < 50MB（索引和缓存） |

### 4.3 功能验收标准

| 验收项 | 标准 |
|--------|------|
| 会话内历史记忆 | 用户在已有会话中继续提问时，Agent 能正确引用之前的对话内容 |
| 跨会话情景记忆 | 新会话启动时能检索同工作区历史摘要，Agent 能参考之前的交互结果 |
| 用户偏好记忆 | 重复使用相同格式时偏好被自动记录，新会话中 Agent 能参考偏好 |
| Token 计数精度 | 与 API 返回 usage 误差 < 5% |
| 工具定义 Token 减少 | 相比全量发送减少 30%-50% |
| 历史压缩 Token 减少 | 相比原始消息减少 40%-60% |
| 任务完成率 | 优化前后完成率不下降 |
| 工具调用准确率 | 优化前后准确率不下降 |
| 上下文利用率 | 优化后提升 10%-20% |

---

## 五、实施路线图

```
阶段零（P0-Critical）: 上下文记忆系统（紧急修复）
  任务0.1: 会话内历史消息加载              [预计2天]
  任务0.2: 会话摘要持久化（情景记忆）       [预计3天]
  任务0.3: 用户偏好记忆（语义记忆）         [预计2天]

阶段一（P0）: Token精确计数与预算管理增强
  任务1.1: tiktoken-rs集成               [预计3天]
  任务1.2: 动态上下文窗口适配             [预计2天]
  任务1.3: 运行时Token使用量追踪          [预计2天]

阶段二（P0）: 动态工具选择与优化
  任务2.1: 基于任务类型的工具集动态过滤    [预计3天]
  任务2.2: 工具描述精炼与参数约束增强      [预计2天]

阶段三（P1）: 高级历史压缩策略
  任务3.1: 分层摘要压缩实现               [预计4天]
  任务3.2: 关键轮次标记与保护             [预计2天]

阶段四（P1）: 上下文策略引擎
  任务4.1: 上下文策略引擎核心实现          [预计3天]
  任务4.2: 上下文水印实现                 [预计1天]

阶段五（P2）: 模型适配与Provider感知
  任务5.1: Provider感知的上下文构建        [预计3天]
  任务5.2: 动态预算分配                   [预计2天]

阶段六（P2）: 评估与迭代体系
  任务6.1: 上下文工程评估指标体系          [预计3天]
  任务6.2: A/B测试框架（可选）            [预计3天]

阶段七（P3）: 检索上下文增强
  任务7.1: 工作区文件索引与语义检索        [预计5天]
  任务7.2: 上下文压缩链                   [预计3天]
```

---

## 六、风险与缓解措施

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 历史消息格式转换不完整 | Agent 行为异常 | 损坏消息跳过而非阻塞；添加格式校验 |
| 历史消息过长超出预算 | API 调用失败 | 加载后立即执行压缩；设置最大加载条数 |
| 会话摘要质量不足 | 跨会话信息丢失 | 纯规则摘要 + 可选 LLM 摘要增强；关键信息优先保留 |
| 用户偏好提取误判 | Agent 行为偏差 | 置信度阈值过滤；用户可手动清除偏好 |
| tiktoken-rs 编译问题 | 阻塞阶段一 | 预先验证编译，准备纯 Rust 实现的回退方案 |
| 工具过滤导致任务失败 | 任务完成率下降 | 未知任务类型回退到全量工具；提供配置开关 |
| 历史摘要丢失关键信息 | Agent 行为异常 | 关键轮次保护机制；摘要质量评估 |
| LLM 调用摘要增加成本 | Token 消耗增加 | 优先使用规则摘要，LLM摘要仅作为可选方案 |
| 不同 Provider 适配复杂 | 开发周期延长 | 优先适配 OpenAI/Anthropic，其他逐步支持 |
| 上下文策略引擎过度设计 | 维护成本增加 | 保持策略接口简洁，默认策略覆盖80%场景 |

---

## 七、总结

本开发计划基于菜鸟教程《Agent 上下文工程》的理论框架，结合 DocAgent 项目的实际代码现状，制定了8个阶段、16个任务的上下文工程优化方案。

**核心发现**:
- 项目已具备良好的分层系统提示词架构和基础 Token 预算管理
- **最严重缺陷**：Agent 每次启动创建空 AgentContext，不加载历史消息，导致用户在已有会话中继续提问时 Agent 无法感知之前的对话
- 关键差距集中在：上下文记忆缺失、Token 精确计数、动态工具选择、高级历史压缩、运行时指标追踪
- 现有架构扩展性良好，新增功能可在不破坏现有逻辑的前提下实现

**优先级建议**:
- P0-Critical（立即修复）：阶段零，修复历史消息不加载的 Bug 并建立三层记忆体系
- P0（立即实施）：阶段一和阶段二，解决 Token 计数不准和工具定义浪费的根本问题
- P1（短期实施）：阶段三和阶段四，提升长对话场景的上下文管理质量
- P2（中期实施）：阶段五和阶段六，实现模型适配和评估体系
- P3（远期规划）：阶段七，检索增强和压缩链需要较大工程投入

**预期收益**:
- 修复用户最痛点的"对话无记忆"问题
- 上下文 Token 消耗减少 30%-50%
- 长对话场景任务完成率提升
- 上下文策略可观测、可调试、可优化
- 为后续 RAG 等高级功能奠定基础
